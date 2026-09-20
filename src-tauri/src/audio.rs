use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Holds an in-progress or finished system-audio capture.
#[derive(Default)]
pub struct CaptureState {
    inner: Mutex<Option<Recording>>,
    /// WAV bytes of the most recently finished capture.
    pub last: Mutex<Option<Vec<u8>>>,
}

struct Recording {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<Result<Vec<u8>, String>>,
}

impl CaptureState {
    /// Start capturing the system output (loopback) audio.
    pub fn start(&self) -> Result<(), String> {
        let mut guard = self.inner.lock().unwrap();
        if guard.is_some() {
            return Err("A capture is already running".into());
        }

        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

        let thread = std::thread::spawn(move || capture_loop(stop_thread, ready_tx));

        // Wait until the stream is actually running (or failed to start).
        match ready_rx.recv().map_err(|e| e.to_string())? {
            Ok(()) => {}
            Err(e) => {
                let _ = thread.join();
                return Err(e);
            }
        }

        *guard = Some(Recording { stop, thread });
        Ok(())
    }

    /// Stop the running capture and store the resulting WAV bytes.
    pub fn stop(&self) -> Result<Vec<u8>, String> {
        let recording = self.inner.lock().unwrap().take();
        let recording = recording.ok_or("No capture is running")?;
        recording.stop.store(true, Ordering::Relaxed);
        let bytes = recording
            .thread
            .join()
            .map_err(|_| "Capture thread panicked".to_string())??;
        *self.last.lock().unwrap() = Some(bytes.clone());
        Ok(bytes)
    }

    pub fn is_recording(&self) -> bool {
        self.inner.lock().unwrap().is_some()
    }
}

fn capture_loop(
    stop: Arc<AtomicBool>,
    ready_tx: mpsc::Sender<Result<(), String>>,
) -> Result<Vec<u8>, String> {
    let host = cpal::default_host();
    // On Windows, building an input stream on the default *output* device
    // performs WASAPI loopback capture of the system audio.
    let device = match host.default_output_device() {
        Some(d) => d,
        None => {
            let _ = ready_tx.send(Err("No default output device found".into()));
            return Err("No default output device found".into());
        }
    };

    let default_config = match device.default_output_config() {
        Ok(c) => c,
        Err(e) => {
            let _ = ready_tx.send(Err(e.to_string()));
            return Err(e.to_string());
        }
    };

    let sample_format = default_config.sample_format();
    let config: cpal::StreamConfig = default_config.into();
    let channels = config.channels as usize;
    let sample_rate = config.sample_rate.0;

    // Mono f32 samples accumulated from the audio callback.
    let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
    let sink = samples.clone();

    let err_fn = |err| eprintln!("audio stream error: {err}");

    let downmix = move |frame: &[f32], sink: &Arc<Mutex<Vec<f32>>>| {
        let mut buf = sink.lock().unwrap();
        for chunk in frame.chunks(channels) {
            let sum: f32 = chunk.iter().copied().sum();
            buf.push(sum / channels as f32);
        }
    };

    let build = |device: &cpal::Device| -> Result<cpal::Stream, cpal::BuildStreamError> {
        match sample_format {
            cpal::SampleFormat::F32 => {
                let sink = sink.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _| downmix(data, &sink),
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let sink = sink.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _| {
                        let f: Vec<f32> =
                            data.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                        downmix(&f, &sink);
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let sink = sink.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _| {
                        let f: Vec<f32> = data
                            .iter()
                            .map(|s| (*s as f32 - 32768.0) / 32768.0)
                            .collect();
                        downmix(&f, &sink);
                    },
                    err_fn,
                    None,
                )
            }
            other => {
                eprintln!("unsupported sample format: {other:?}");
                Err(cpal::BuildStreamError::StreamConfigNotSupported)
            }
        }
    };

    let stream = match build(&device) {
        Ok(s) => s,
        Err(e) => {
            let _ = ready_tx.send(Err(e.to_string()));
            return Err(e.to_string());
        }
    };

    if let Err(e) = stream.play() {
        let _ = ready_tx.send(Err(e.to_string()));
        return Err(e.to_string());
    }

    let _ = ready_tx.send(Ok(()));

    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    drop(stream);

    let buf = samples.lock().unwrap();
    encode_wav(&buf, sample_rate)
}

/// Encode mono f32 samples as a 16-bit PCM WAV file in memory.
fn encode_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
    {
        let mut writer =
            hound::WavWriter::new(&mut cursor, spec).map_err(|e| e.to_string())?;
        for &sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            let value = (clamped * i16::MAX as f32) as i16;
            writer.write_sample(value).map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }
    Ok(cursor.into_inner())
}
