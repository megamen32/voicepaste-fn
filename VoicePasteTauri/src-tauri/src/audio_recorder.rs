use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use hound::{WavSpec, WavWriter};
use parking_lot::Mutex;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::Arc;

/// Wrapper around cpal::Stream to make it Send + Sync.
/// Safety: cpal::Stream is !Send on some platforms, but we only access it
/// while holding the AudioRecorder mutex, so no concurrent access occurs.
#[allow(dead_code)]
struct SendStream(Stream);
unsafe impl Send for SendStream {}
unsafe impl Sync for SendStream {}

/// Records audio from the default microphone to a WAV file.
pub struct AudioRecorder {
    stream: Option<SendStream>,
    current_path: Option<PathBuf>,
    writer: Arc<Mutex<Option<WavWriter<BufWriter<std::fs::File>>>>>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            current_path: None,
            writer: Arc::new(Mutex::new(None)),
        }
    }

    /// Current recording file path (set after start, cleared after stop).
    pub fn current_path(&self) -> Option<&PathBuf> {
        self.current_path.as_ref()
    }

    /// Start recording from the default input device.
    pub fn start(&mut self) -> Result<(), String> {
        self.stop_internal(false);

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get input config: {}", e))?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as u16;

        // Create temp file
        let dir = std::env::temp_dir().join("voicepaste-recordings");
        std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create dir: {}", e))?;
        let path = dir.join(format!("{}.wav", uuid_simple()));

        // Set up WAV writer writing directly to file
        let spec = WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let file = std::fs::File::create(&path)
            .map_err(|e| format!("Cannot create file: {}", e))?;
        let buf_writer = BufWriter::new(file);
        let wav_writer = WavWriter::new(buf_writer, spec)
            .map_err(|e| format!("Cannot create WAV writer: {}", e))?;

        let writer = Arc::new(Mutex::new(Some(wav_writer)));
        let writer_clone = writer.clone();

        let stream_config = cpal::StreamConfig {
            channels: channels.into(),
            sample_rate: config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = match config.sample_format() {
            SampleFormat::I16 => device.build_input_stream(
                &stream_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let mut guard = writer_clone.lock();
                    if let Some(w) = guard.as_mut() {
                        for &sample in data {
                            let _ = w.write_sample(sample);
                        }
                    }
                },
                |err| {
                    log::error!("Audio stream error: {}", err);
                },
                None,
            ),
            SampleFormat::F32 => device.build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let mut guard = writer_clone.lock();
                    if let Some(w) = guard.as_mut() {
                        for &sample in data {
                            let _ = w.write_sample((sample * i16::MAX as f32) as i16);
                        }
                    }
                },
                |err| {
                    log::error!("Audio stream error: {}", err);
                },
                None,
            ),
            _ => return Err("Unsupported sample format".to_string()),
        };

        let stream = stream.map_err(|e| format!("Failed to build input stream: {}", e))?;
        stream.play().map_err(|e| format!("Failed to start stream: {}", e))?;

        self.stream = Some(SendStream(stream));
        self.writer = writer;
        self.current_path = Some(path);
        Ok(())
    }

    /// Stop recording and return the path to the WAV file.
    pub fn stop(&mut self) -> Option<PathBuf> {
        self.stop_internal(true)
    }

    /// Stop recording without returning the file (discards it).
    pub fn stop_and_discard(&mut self) {
        self.stop_internal(false);
    }

    fn stop_internal(&mut self, keep_file: bool) -> Option<PathBuf> {
        // Drop the stream first to stop capturing
        self.stream = None;

        // Finalize the WAV writer
        let path = self.current_path.take();
        let writer_guard = self.writer.lock().take();

        if let (Some(path), Some(writer)) = (path, writer_guard) {
            if keep_file {
                // Finalize the WAV file (writes header with correct sizes)
                if writer.finalize().is_ok() {
                    return Some(path);
                }
            }
            // Discard
            let _ = std::fs::remove_file(&path);
        }

        None
    }
}

fn uuid_simple() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:016x}{:016x}", rng.gen::<u64>(), rng.gen::<u64>())
}
