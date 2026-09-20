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

        let (recognizer, vad) = engine.as_ref().expect("engine loaded above");
        if let Err(e) = session(&app, recognizer, vad, &recording) {
            let _ = app.emit("stt-error", e);
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
    let default_config = device.default_output_config().map_err(|e| e.to_string())?;
    let sample_format = default_config.sample_format();
    let config: cpal::StreamConfig = default_config.into();
    let channels = config.channels as usize;
    let device_rate = config.sample_rate.0 as i32;

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

    let mut buffer: Vec<f32> = Vec::new();
    let mut offset = 0usize;
    let mut speech_started = false;
    let mut last_partial = Instant::now();

    while recording.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(samples) => match &resampler {
                Some(r) => buffer.extend_from_slice(&r.resample(&samples, false)),
                None => buffer.extend_from_slice(&samples),
            },
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        // Feed the VAD in fixed 512-sample windows.
        while offset + WINDOW <= buffer.len() {
            vad.accept_waveform(&buffer[offset..offset + WINDOW]);
            if !speech_started && vad.detected() {
                speech_started = true;
            }
            offset += WINDOW;
        }

        // Drop stale leading silence so the buffer doesn't grow unbounded.
        if !speech_started && buffer.len() > 10 * WINDOW {
            buffer = buffer[buffer.len() - 10 * WINDOW..].to_vec();
            offset = 0;
        }

        // Interim transcript while the current utterance is still ongoing.
        if speech_started && last_partial.elapsed().as_secs_f32() > 0.5 {
            emit_decode(app, recognizer, &buffer, "stt-partial");
            last_partial = Instant::now();
        }

        // Emit finished utterances detected by the VAD.
        while !vad.is_empty() {
            if let Some(segment) = vad.front() {
                emit_decode(app, recognizer, segment.samples(), "stt-final");
            }
            vad.pop();
            buffer.clear();
            offset = 0;
            speech_started = false;
        }
    }

    // Flush any trailing speech that was buffered when the user stopped.
    vad.flush();
    while !vad.is_empty() {
        if let Some(segment) = vad.front() {
            emit_decode(app, recognizer, segment.samples(), "stt-final");
        }
        vad.pop();
    }

    drop(stream);
    Ok(())
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
