//! Local, embedded speech-to-text.
//!
//! Captures the system output (WASAPI loopback) audio, runs Silero VAD to
//! segment speech, and transcribes each segment with an offline SenseVoice
//! model via `sherpa-onnx`. Everything runs on-device — no network, no API key.
//!
//! A single long-lived worker thread owns the recognizer, the VAD and the
//! audio stream (all of which are cheap to keep but expensive to reload), and
//! streams results back to the UI through Tauri events:
//!   * `stt-status`  — "loading" | "listening" | "stopped"
//!   * `stt-partial` — interim transcript for the current utterance
//!   * `stt-final`   — a finished utterance
//!   * `stt-error`   — a human-readable error string

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use sherpa_onnx::{
    LinearResampler, OfflineRecognizer, OfflineRecognizerConfig, VadModelConfig,
    VoiceActivityDetector,
};
use tauri::{AppHandle, Emitter};

/// Sample rate expected by the VAD and the recognizer.
const SAMPLE_RATE: i32 = 16_000;
/// VAD window size in samples — do not change, the Silero model expects 512.
const WINDOW: usize = 512;

enum Cmd {
    Start,
}

/// Shared handle to the STT worker, stored as Tauri managed state.
pub struct SttState {
    tx: Sender<Cmd>,
    recording: Arc<AtomicBool>,
}

impl SttState {
    /// Spawn the worker thread. Models are loaded lazily on the first `start`.
    pub fn new(app: AppHandle, models_dir: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel::<Cmd>();
        let recording = Arc::new(AtomicBool::new(false));
        let worker_recording = recording.clone();
        std::thread::spawn(move || worker(app, models_dir, rx, worker_recording));
        Self { tx, recording }
    }

    pub fn start(&self) -> Result<(), String> {
        if self.recording.swap(true, Ordering::SeqCst) {
            return Err("Already recording".into());
        }
        self.tx
            .send(Cmd::Start)
            .map_err(|_| "Speech engine is not running".to_string())
    }

    pub fn stop(&self) {
        self.recording.store(false, Ordering::SeqCst);
    }

    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }
}

/// The worker owns all sherpa-onnx objects and the audio stream. It stays
/// parked on the command channel between sessions so the models load only once.
fn worker(app: AppHandle, models_dir: PathBuf, rx: Receiver<Cmd>, recording: Arc<AtomicBool>) {
    let mut engine: Option<(OfflineRecognizer, VoiceActivityDetector)> = None;

    while let Ok(Cmd::Start) = rx.recv() {
        if engine.is_none() {
            let _ = app.emit("stt-status", "loading");
            match load(&models_dir) {
                Ok(loaded) => engine = Some(loaded),
                Err(e) => {
                    recording.store(false, Ordering::SeqCst);
                    let _ = app.emit("stt-error", e);
                    continue;
                }
            }
        }

        // Run the session guarded against panics so a single failure never kills
        // the worker thread (which would leave the UI stuck on "Finishing…").
        let outcome = {
            let (recognizer, vad) = engine.as_ref().expect("engine loaded above");
            catch_unwind(AssertUnwindSafe(|| session(&app, recognizer, vad, &recording)))
        };

        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                eprintln!("[stt] session error: {e}");
                let _ = app.emit("stt-error", e);
            }
            Err(_) => {
                eprintln!("[stt] session panicked; resetting engine");
                // The recognizer/VAD may be in a bad state — force a reload.
                engine = None;
                let _ = app.emit("stt-error", "Speech engine hit an error. Try again.");
            }
        }

        recording.store(false, Ordering::SeqCst);
        let _ = app.emit("stt-status", "stopped");
    }
}

/// Load the Silero VAD and the offline SenseVoice recognizer from `models_dir`.
fn load(models_dir: &Path) -> Result<(OfflineRecognizer, VoiceActivityDetector), String> {
    let sense_model = models_dir.join("sense-voice").join("model.int8.onnx");
    let tokens = models_dir.join("sense-voice").join("tokens.txt");
    let vad_model = models_dir.join("silero_vad.onnx");

    for path in [&sense_model, &tokens, &vad_model] {
        if !path.exists() {
            return Err(format!(
                "Missing speech model file: {}. Run scripts/download-models.ps1 first.",
                path.display()
            ));
        }
    }

    let mut config = OfflineRecognizerConfig::default();
    config.model_config.sense_voice.model = Some(sense_model.to_string_lossy().into_owned());
    config.model_config.sense_voice.language = Some("auto".to_string());
    config.model_config.sense_voice.use_itn = true;
    config.model_config.tokens = Some(tokens.to_string_lossy().into_owned());
    config.model_config.num_threads = 2;
    let recognizer =
        OfflineRecognizer::create(&config).ok_or("Failed to create speech recognizer")?;

    let mut vad_config = VadModelConfig::default();
    vad_config.silero_vad.model = Some(vad_model.to_string_lossy().into_owned());
    vad_config.silero_vad.threshold = 0.5;
    vad_config.silero_vad.min_silence_duration = 0.25;
    vad_config.silero_vad.min_speech_duration = 0.25;
    vad_config.silero_vad.max_speech_duration = 8.0;
    vad_config.silero_vad.window_size = WINDOW as i32;
    vad_config.sample_rate = SAMPLE_RATE;
    let vad = VoiceActivityDetector::create(&vad_config, 30.0)
        .ok_or("Failed to create voice activity detector")?;

    Ok((recognizer, vad))
}

/// Run one capture-and-transcribe session until `recording` is cleared.
fn session(
    app: &AppHandle,
    recognizer: &OfflineRecognizer,
    vad: &VoiceActivityDetector,
    recording: &AtomicBool,
) -> Result<(), String> {
    vad.reset();

    let host = cpal::default_host();
    // On Windows, an input stream on the default *output* device performs
    // WASAPI loopback capture of the system audio.
    let device = host
        .default_output_device()
        .ok_or("No default output device found")?;
    let device_name = device.name().unwrap_or_else(|_| "<unknown>".into());
    let default_config = device.default_output_config().map_err(|e| e.to_string())?;
    let sample_format = default_config.sample_format();
    let config: cpal::StreamConfig = default_config.into();
    let channels = config.channels as usize;
    let device_rate = config.sample_rate.0 as i32;
    eprintln!(
        "[stt] loopback device='{device_name}' rate={device_rate} channels={channels} format={sample_format:?}"
    );

    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let stream = build_stream(&device, &config, sample_format, channels, tx)?;
    stream.play().map_err(|e| e.to_string())?;

    let resampler = if device_rate != SAMPLE_RATE {
        Some(
            LinearResampler::create(device_rate, SAMPLE_RATE)
                .ok_or("Failed to create resampler")?,
        )
    } else {
        None
    };

    let _ = app.emit("stt-status", "listening");

    // `pending` holds 16 kHz mono samples not yet chunked into 512-sample VAD
    // windows. `utterance` accumulates the current speech run so partials can be
    // decoded incrementally without re-decoding old, already-finalised audio.
    let mut pending: Vec<f32> = Vec::new();
    let mut utterance: Vec<f32> = Vec::new();
    let mut in_speech = false;
    let mut last_partial = Instant::now();
    let mut last_level_log = Instant::now();
    let mut peak_level = 0.0f32;
    let mut got_audio = false;

    while recording.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(samples) => {
                got_audio = true;
                match &resampler {
                    Some(r) => pending.extend_from_slice(&r.resample(&samples, false)),
                    None => pending.extend_from_slice(&samples),
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        // Feed the VAD in fixed 512-sample windows.
        let mut consumed = 0usize;
        while consumed + WINDOW <= pending.len() {
            let window = &pending[consumed..consumed + WINDOW];
            for &s in window {
                let a = s.abs();
                if a > peak_level {
                    peak_level = a;
                }
            }
            vad.accept_waveform(window);
            if vad.detected() {
                if !in_speech {
                    in_speech = true;
                    utterance.clear();
                    eprintln!("[stt] speech detected");
                }
            }
            if in_speech {
                utterance.extend_from_slice(window);
            }
            consumed += WINDOW;
        }
        if consumed > 0 {
            pending.drain(0..consumed);
        }

        // Emit finished utterances the VAD has segmented out.
        while !vad.is_empty() {
            if let Some(segment) = vad.front() {
                let samples = segment.samples();
                eprintln!("[stt] final segment: {} samples", samples.len());
                emit_decode(app, recognizer, samples, "stt-final");
            }
            vad.pop();
            in_speech = false;
            utterance.clear();
        }

        // Interim transcript for the still-ongoing utterance (bounded so a long
        // continuous stream never turns decoding into an unbounded cost).
        if in_speech && !utterance.is_empty() && last_partial.elapsed().as_secs_f32() > 0.4 {
            let tail = tail_samples(&utterance, (SAMPLE_RATE as usize) * 12);
            emit_decode(app, recognizer, tail, "stt-partial");
            last_partial = Instant::now();
        }

        if last_level_log.elapsed().as_secs_f32() > 2.0 {
            eprintln!("[stt] peak level over last 2s: {peak_level:.4} (audio flowing: {got_audio})");
            peak_level = 0.0;
            last_level_log = Instant::now();
        }
    }

    // Flush any trailing speech buffered when the user stopped.
    vad.flush();
    let mut flushed_final = false;
    while !vad.is_empty() {
        if let Some(segment) = vad.front() {
            emit_decode(app, recognizer, segment.samples(), "stt-final");
            flushed_final = true;
        }
        vad.pop();
    }
    // If the user stopped mid-sentence the VAD may not have produced a segment;
    // decode whatever speech we accumulated so nothing is silently dropped.
    if !flushed_final && utterance.len() > WINDOW {
        emit_decode(app, recognizer, &utterance, "stt-final");
    }

    drop(stream);
    eprintln!("[stt] session ended (audio flowing: {got_audio})");
    Ok(())
}

/// Return the last `max` samples of `buf` (or all of them if shorter).
fn tail_samples(buf: &[f32], max: usize) -> &[f32] {
    if buf.len() > max {
        &buf[buf.len() - max..]
    } else {
        buf
    }
}

/// Decode `samples` and emit the non-empty transcript under `event`.
fn emit_decode(app: &AppHandle, recognizer: &OfflineRecognizer, samples: &[f32], event: &str) {
    let stream = recognizer.create_stream();
    stream.accept_waveform(SAMPLE_RATE, samples);
    recognizer.decode(&stream);
    if let Some(result) = stream.get_result() {
        let text = result.text.trim().to_string();
        if !text.is_empty() {
            let _ = app.emit(event, text);
        }
    }
}

/// Build a loopback input stream that downmixes to mono f32 and forwards
/// samples over `tx`.
fn build_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    channels: usize,
    tx: Sender<Vec<f32>>,
) -> Result<cpal::Stream, String> {
    let result = match sample_format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _| {
                if !data.is_empty() {
                    let _ = tx.send(downmix(data, channels));
                }
            },
            on_stream_error,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _| {
                if data.is_empty() {
                    return;
                }
                let f: Vec<f32> = data.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                let _ = tx.send(downmix(&f, channels));
            },
            on_stream_error,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            config,
            move |data: &[u16], _| {
                if data.is_empty() {
                    return;
                }
                let f: Vec<f32> = data.iter().map(|s| (*s as f32 - 32768.0) / 32768.0).collect();
                let _ = tx.send(downmix(&f, channels));
            },
            on_stream_error,
            None,
        ),
        other => return Err(format!("Unsupported sample format: {other:?}")),
    };

    result.map_err(|e| e.to_string())
}

fn on_stream_error(err: cpal::StreamError) {
    eprintln!("audio stream error: {err}");
}

/// Average interleaved frames down to a single mono channel.
fn downmix(frames: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return frames.to_vec();
    }
    frames
        .chunks(channels)
        .map(|frame| frame.iter().copied().sum::<f32>() / channels as f32)
        .collect()
}
