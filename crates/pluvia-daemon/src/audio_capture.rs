use crate::runtime::SkinRuntime;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Low-latency system audio capture streamer using PipeWire (pw-record) or PulseAudio (parec).
pub struct AudioCaptureWorker;

impl AudioCaptureWorker {
    /// Spawns the asynchronous audio capture loop.
    pub fn start(
        runtime: Arc<RwLock<SkinRuntime>>,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut float_buffer = vec![0f32; 1024];
            let mut process: Option<Child> = None;

            loop {
                // Check shutdown signal
                if *shutdown_rx.borrow() {
                    if let Some(mut child) = process {
                        let _ = child.kill();
                    }
                    break;
                }

                // Check if any loaded skins exist
                let has_skins = {
                    let rt = runtime.read().await;
                    !rt.list_skins().is_empty()
                };

                if !has_skins {
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    continue;
                }

                if process.is_none() {
                    // Try pw-record first, fallback to parec
                    let spawn_res = Command::new("pw-record")
                        .args(["--channels=2", "--rate=44100", "--format=s16", "-"])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .spawn()
                        .or_else(|_| {
                            Command::new("parec")
                                .args(["--channels=2", "--rate=44100", "--format=s16le"])
                                .stdout(Stdio::piped())
                                .stderr(Stdio::null())
                                .spawn()
                        });

                    if let Ok(child) = spawn_res {
                        process = Some(child);
                    } else {
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        continue;
                    }
                }

                if let Some(ref mut child) = process {
                    if let Some(ref mut stdout) = child.stdout {
                        let mut byte_buf = [0u8; 2048];
                        match stdout.read(&mut byte_buf) {
                            Ok(0) => {
                                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                            }
                            Ok(bytes_read) => {
                                let samples_read = bytes_read / 2;
                                if samples_read > 0 {
                                    float_buffer.clear();
                                    for i in 0..samples_read {
                                        let sample = i16::from_le_bytes([byte_buf[i * 2], byte_buf[i * 2 + 1]]);
                                        float_buffer.push(sample as f32 / 32768.0);
                                    }

                                    let mut rt = runtime.write().await;
                                    rt.feed_audio_samples(&float_buffer);
                                }
                            }
                            Err(_) => {
                                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                            }
                        }
                    }
                }

                tokio::task::yield_now().await;
            }
        })
    }
}
