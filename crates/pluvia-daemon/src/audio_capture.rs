use crate::runtime::SkinRuntime;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Low-latency system audio capture streamer using PipeWire (pw-record) or PulseAudio (parec).
pub struct AudioCaptureWorker;

impl AudioCaptureWorker {
    /// Spawns the audio capture loop on a dedicated blocking OS thread.
    pub fn start(
        runtime: Arc<RwLock<SkinRuntime>>,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::task::spawn_blocking(move || {
            let mut float_buffer = Vec::with_capacity(1024);
            let mut process: Option<Child> = None;

            while !*shutdown_rx.borrow() {
                // Check if any loaded skins exist
                let has_skins = {
                    if let Ok(rt) = runtime.try_read() {
                        !rt.list_skins().is_empty()
                    } else {
                        true
                    }
                };

                if !has_skins {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    continue;
                }

                if process.is_none() {
                    // Strictly capture desktop audio playback monitor (never the microphone input)
                    let monitor_device = get_default_monitor_sink_name();
                    let spawn_res = Command::new("parec")
                        .args(["-d", &monitor_device, "--channels=2", "--rate=44100", "--format=s16le"])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .spawn()
                        .or_else(|_| {
                            Command::new("parec")
                                .args(["-d", "@DEFAULT_MONITOR@", "--channels=2", "--rate=44100", "--format=s16le"])
                                .stdout(Stdio::piped())
                                .stderr(Stdio::null())
                                .spawn()
                        });

                    if let Ok(child) = spawn_res {
                        process = Some(child);
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        continue;
                    }
                }

                if let Some(ref mut child) = process {
                    if let Ok(Some(_status)) = child.try_wait() {
                        process = None;
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        continue;
                    }

                    if let Some(ref mut stdout) = child.stdout {
                        let mut byte_buf = [0u8; 2048];
                        match stdout.read(&mut byte_buf) {
                            Ok(0) => {
                                std::thread::sleep(std::time::Duration::from_millis(20));
                            }
                            Ok(bytes_read) => {
                                let samples_read = bytes_read / 2;
                                if samples_read > 0 {
                                    float_buffer.clear();
                                    for i in 0..samples_read {
                                        let sample = i16::from_le_bytes([byte_buf[i * 2], byte_buf[i * 2 + 1]]);
                                        float_buffer.push(sample as f32 / 32768.0);
                                    }

                                    if let Ok(mut rt) = runtime.try_write() {
                                        rt.feed_audio_samples(&float_buffer);
                                    }
                                }
                            }
                            Err(_) => {
                                std::thread::sleep(std::time::Duration::from_millis(20));
                            }
                        }
                    }
                }
            }

            if let Some(mut child) = process {
                let _ = child.kill();
            }
        })
    }
}

fn get_default_monitor_sink_name() -> String {
    if let Ok(output) = Command::new("pactl").arg("get-default-sink").output() {
        if output.status.success() {
            let sink = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !sink.is_empty() {
                return format!("{}.monitor", sink);
            }
        }
    }
    "@DEFAULT_MONITOR@".to_string()
}


