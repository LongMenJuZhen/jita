// 语音识别引擎模块
// 基于 Sherpa-onnx 实现离线语音识别（ASR）
// 支持 VAD（语音活动检测）和 Qwen3 ASR 模型

use anyhow::Result;
use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use sherpa_onnx::{
    LinearResampler, OfflineQwen3ASRModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
    VadModelConfig, VoiceActivityDetector,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

// 模型下载相关常量
const VAD_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx";
const ASR_ARCHIVE_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2";
const ASR_DIR_NAME: &str = "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25";

// ASR 管理器 - 负责模型预加载和音频监听
pub struct AsrManager {
    model_dir: PathBuf,
    // 预加载的模型
    vad: Arc<Mutex<Option<VoiceActivityDetector>>>,
    recognizer: Arc<Mutex<Option<OfflineRecognizer>>>,
    resampler: Arc<Mutex<Option<LinearResampler>>>,
    stop_flag: Arc<AtomicBool>,
    listen_thread: Option<JoinHandle<()>>,
}

impl AsrManager {
    pub fn new(model_dir: PathBuf) -> Self {
        Self {
            model_dir,
            vad: Arc::new(Mutex::new(None)),
            recognizer: Arc::new(Mutex::new(None)),
            resampler: Arc::new(Mutex::new(None)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            listen_thread: None,
        }
    }

    /// 预加载模型到内存（启动时调用）
    pub fn preload(&self, on_status: impl Fn(String)) -> Result<()> {
        on_status("正在加载 ASR 模型...".to_string());

        let vad = create_vad(&self.model_dir)?;
        let recognizer = create_recognizer(&self.model_dir)?;

        // 检查采样率并创建重采样器
        let host = cpal::default_host();
        let resampler = if let Some(device) = host.default_input_device() {
            if let Ok(config) = device.default_input_config() {
                let sample_rate = config.sample_rate().0 as i32;
                if sample_rate != 16000 {
                    on_status(format!("需要重采样: {} -> 16000", sample_rate));
                    Some(LinearResampler::create(sample_rate, 16000).expect("创建重采样器失败"))
                } else {
                    on_status("使用原生 16kHz 采样率".to_string());
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        *self.vad.lock().unwrap() = Some(vad);
        *self.recognizer.lock().unwrap() = Some(recognizer);
        *self.resampler.lock().unwrap() = resampler;

        on_status("ASR 模型已就绪".to_string());
        Ok(())
    }

    /// 检查模型是否已加载
    pub fn is_loaded(&self) -> bool {
        self.vad.lock().unwrap().is_some() && self.recognizer.lock().unwrap().is_some()
    }

    /// 检查是否正在监听
    pub fn is_listening(&self) -> bool {
        self.listen_thread.is_some()
    }

    /// 开始监听
    pub fn start_listening(
        &mut self,
        on_status: impl Fn(String) + Send + 'static + 'static,
        on_text: impl Fn(String) + Send + 'static + 'static,
    ) -> Result<()> {
        // 如果已在运行，先停止
        self.stop();

        // 获取模型引用
        let vad = match self.vad.lock().unwrap().take() {
            Some(v) => v,
            None => {
                on_status("错误: ASR 模型未加载".to_string());
                return Err(anyhow::anyhow!("模型未加载"));
            }
        };
        let recognizer = match self.recognizer.lock().unwrap().take() {
            Some(r) => r,
            None => {
                on_status("错误: ASR 模型未加载".to_string());
                return Err(anyhow::anyhow!("模型未加载"));
            }
        };
        let resampler = self.resampler.lock().unwrap().take();

        // 重置停止标志
        self.stop_flag.store(false, Ordering::SeqCst);

        let stop_flag = self.stop_flag.clone();
        let sample_rate = 16000i32;
        let window_size = 512usize;

        let handle = thread::spawn(move || {
            // 获取音频设备
            let host = cpal::default_host();
            let device = match list_input_devices(&host) {
                Ok(d) => d,
                Err(e) => {
                    on_status(format!("设备错误: {}", e));
                    return;
                }
            };

            let supported = match device.default_input_config() {
                Ok(c) => c,
                Err(e) => {
                    on_status(format!("配置错误: {}", e));
                    return;
                }
            };

            // 创建音频流
            let (tx, rx) = mpsc::channel::<Vec<f32>>();
            let audio_stream = match build_input_stream(&device, tx) {
                Ok(s) => s,
                Err(e) => {
                    on_status(format!("音频流错误: {}", e));
                    return;
                }
            };

            if let Err(e) = audio_stream.play() {
                on_status(format!("播放错误: {}", e));
                return;
            }
            on_status("正在监听...".to_string());

            let mut buffer: Vec<f32> = Vec::new();
            let mut offset = 0usize;
            let mut speech_started = false;
            let mut started_time = Instant::now();

            // 主循环
            loop {
                if stop_flag.load(Ordering::SeqCst) {
                    on_status("ASR 已停止".to_string());
                    break;
                }

                // 接收音频数据
                while let Ok(samples) = rx.try_recv() {
                    if let Some(ref resampler) = resampler {
                        let resampled = resampler.resample(&samples, false);
                        buffer.extend_from_slice(&resampled);
                    } else {
                        buffer.extend_from_slice(&samples);
                    }
                }

                // VAD 检测
                while offset + window_size <= buffer.len() {
                    vad.accept_waveform(&buffer[offset..offset + window_size]);
                    if !speech_started && vad.detected() {
                        speech_started = true;
                        started_time = Instant::now();
                        on_status("检测到语音...".to_string());
                    }
                    offset += window_size;
                }

                // 丢弃旧数据
                if !speech_started && buffer.len() > 10 * window_size {
                    let trim_amount = buffer.len() - 10 * window_size;
                    offset = offset.saturating_sub(trim_amount);
                    buffer = buffer[buffer.len() - 10 * window_size..].to_vec();
                }

                // 处理完成的语音片段
                while !vad.is_empty() {
                    if let Some(segment) = vad.front() {
                        vad.pop();
                        let stream = recognizer.create_stream();
                        stream.accept_waveform(sample_rate, segment.samples());
                        recognizer.decode(&stream);
                        if let Some(result) = stream.get_result() {
                            if !result.text.is_empty() {
                                on_text(result.text.clone());
                            }
                        }
                    }
                    // 重置状态
                    buffer.clear();
                    offset = 0;
                    speech_started = false;
                    on_status("正在监听...".to_string());
                }

                thread::sleep(Duration::from_millis(50));
            }
        });

        self.listen_thread = Some(handle);
        Ok(())
    }

    /// 停止监听
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.listen_thread.take() {
            let _ = handle.join();
        }
    }

    /// 重置模型引用（停止后调用）
    pub fn reset_models(&mut self) {
        // 这个方法在 stop 后可以用来重新加载模型引用
        // 但实际上模型已经通过 take() 被消费了，需要重新创建
    }
}

impl Default for AsrManager {
    fn default() -> Self {
        Self::new(PathBuf::from("../../.."))
    }
}

impl Drop for AsrManager {
    fn drop(&mut self) {
        self.stop();
    }
}

// =============================================================================
// 模型管理：检查、下载、解压
// =============================================================================

/// 检查所有必需的 ASR 模型文件是否就绪
pub fn models_ready(model_dir: &Path) -> bool {
    let vad_path = model_dir.join("silero_vad.onnx");
    let asr_dir = model_dir.join(ASR_DIR_NAME);
    let files = ["conv_frontend.onnx", "encoder.int8.onnx", "decoder.int8.onnx", "tokenizer"];
    vad_path.exists() && files.iter().all(|f| asr_dir.join(f).exists())
}

/// 确保模型存在，缺失时自动下载
pub async fn ensure_models(
    model_dir: &Path,
    on_status: impl Fn(String) + Send,
) -> Result<()> {
    if models_ready(model_dir) {
        return Ok(());
    }

    std::fs::create_dir_all(model_dir)?;

    // 下载 VAD 模型
    let vad_path = model_dir.join("silero_vad.onnx");
    if !vad_path.exists() {
        on_status("正在下载 VAD 模型...".to_string());
        download_file(VAD_URL, &vad_path, |downloaded, total| {
            if let Some(t) = total {
                on_status(format!(
                    "下载 VAD 模型: {:.1}/{:.1} MB",
                    downloaded as f64 / 1024.0 / 1024.0,
                    t as f64 / 1024.0 / 1024.0
                ));
            } else {
                on_status(format!(
                    "下载 VAD 模型: {:.1} MB",
                    downloaded as f64 / 1024.0 / 1024.0
                ));
            }
        })
        .await?;
    }

    // 下载并解压 ASR 模型包
    let asr_dir = model_dir.join(ASR_DIR_NAME);
    let asr_files = ["conv_frontend.onnx", "encoder.int8.onnx", "decoder.int8.onnx", "tokenizer"];
    let need_asr = !asr_files.iter().all(|f| asr_dir.join(f).exists());

    if need_asr {
        on_status("正在下载 ASR 模型（约 600MB，请耐心等待）...".to_string());
        let archive_path = model_dir.join("asr_models.tar.bz2");
        download_file(ASR_ARCHIVE_URL, &archive_path, |downloaded, total| {
            if let Some(t) = total {
                on_status(format!(
                    "下载 ASR 模型: {:.1}/{:.1} MB",
                    downloaded as f64 / 1024.0 / 1024.0,
                    t as f64 / 1024.0 / 1024.0
                ));
            } else {
                on_status(format!(
                    "下载 ASR 模型: {:.1} MB",
                    downloaded as f64 / 1024.0 / 1024.0
                ));
            }
        })
        .await?;

        on_status("正在解压 ASR 模型...".to_string());
        extract_tar_bz2(&archive_path, model_dir).await?;
        let _ = std::fs::remove_file(&archive_path);
        on_status("ASR 模型解压完成".to_string());
    }

    Ok(())
}

/// 下载文件，带进度回调
async fn download_file(
    url: &str,
    path: &Path,
    progress: impl Fn(u64, Option<u64>),
) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?;

    let mut response = client.get(url).send().await?;
    let total = response.content_length();

    let mut file = tokio::fs::File::create(path).await?;
    let mut downloaded: u64 = 0;

    while let Some(chunk) = response.chunk().await? {
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
        downloaded += chunk.len() as u64;
        progress(downloaded, total);
    }

    tokio::io::AsyncWriteExt::flush(&mut file).await?;
    Ok(())
}

/// 解压 tar.bz2 归档
async fn extract_tar_bz2(archive: &Path, dest: &Path) -> Result<()> {
    let archive = archive.to_path_buf();
    let dest = dest.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&archive)?;
        let decoder = bzip2::read::BzDecoder::new(file);
        let mut tar = tar::Archive::new(decoder);
        tar.unpack(&dest)?;
        Ok::<(), anyhow::Error>(())
    })
    .await??;

    Ok(())
}

// =============================================================================
// 音频设备与输入流
// =============================================================================

/// 列出可用的输入设备
fn list_input_devices(host: &cpal::Host) -> Result<cpal::Device> {
    let default_input = host.default_input_device();
    let default_name = default_input.as_ref().map(|d| d.name().unwrap_or_default());

    println!("可用的输入设备:");
    for device in host.input_devices()? {
        let name = device.name().unwrap_or("<unknown>".to_string());
        let mark = if Some(&name) == default_name.as_ref() { "*" } else { " " };
        println!("{} {}", mark, name);
    }

    let device = default_input.ok_or_else(|| anyhow::anyhow!("没有默认输入设备"))?;
    println!("使用默认设备: {}", device.name()?);
    Ok(device)
}

/// 构建音频输入流
fn build_input_stream(device: &cpal::Device, tx: mpsc::Sender<Vec<f32>>) -> Result<cpal::Stream> {
    let supported = device.default_input_config()?;
    let config = supported.config();
    let sample_format = supported.sample_format();
    let channels = config.channels as usize;

    let err_fn = |err| eprintln!("音频流错误: {:?}", err);

    println!(
        "输入格式: {:?}, 声道: {}, 采样率: {}",
        sample_format, channels, config.sample_rate.0
    );

    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _| {
                if data.is_empty() { return; }
                let mono: Vec<f32> = data
                    .chunks(channels)
                    .map(|frame| frame.iter().copied().sum::<f32>() / channels as f32)
                    .collect();
                let _ = tx.send(mono);
            },
            err_fn,
            None,
        )?,

        SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _| {
                if data.is_empty() { return; }
                let mono: Vec<f32> = data
                    .chunks(channels)
                    .map(|frame| {
                        frame.iter().map(|&s| s as f32 / i16::MAX as f32).sum::<f32>() / channels as f32
                    })
                    .collect();
                let _ = tx.send(mono);
            },
            err_fn,
            None,
        )?,

        SampleFormat::U16 => device.build_input_stream(
            &config,
            move |data: &[u16], _| {
                if data.is_empty() { return; }
                let mono: Vec<f32> = data
                    .chunks(channels)
                    .map(|frame| {
                        frame.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).sum::<f32>() / channels as f32
                    })
                    .collect();
                let _ = tx.send(mono);
            },
            err_fn,
            None,
        )?,

        other => anyhow::bail!("不支持的采样格式: {:?}", other),
    };

    Ok(stream)
}

/// 创建 VAD
fn create_vad(model_dir: &Path) -> Result<VoiceActivityDetector> {
    let mut config = VadModelConfig::default();
    config.silero_vad.model = Some(model_dir.join("silero_vad.onnx").to_string_lossy().to_string());
    config.silero_vad.threshold = 0.5;
    config.silero_vad.min_silence_duration = 0.3;
    config.silero_vad.min_speech_duration = 0.25;
    config.silero_vad.max_speech_duration = 10.0;
    config.silero_vad.window_size = 512;
    config.sample_rate = 16000;
    config.debug = false;

    VoiceActivityDetector::create(&config, 20.0)
        .ok_or_else(|| anyhow::anyhow!("创建 VAD 失败"))
}

/// 创建识别器
fn create_recognizer(model_dir: &Path) -> Result<OfflineRecognizer> {
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.qwen3_asr = OfflineQwen3ASRModelConfig {
        conv_frontend: Some(model_dir.join(ASR_DIR_NAME).join("conv_frontend.onnx").to_string_lossy().to_string()),
        encoder: Some(model_dir.join(ASR_DIR_NAME).join("encoder.int8.onnx").to_string_lossy().to_string()),
        decoder: Some(model_dir.join(ASR_DIR_NAME).join("decoder.int8.onnx").to_string_lossy().to_string()),
        tokenizer: Some(model_dir.join(ASR_DIR_NAME).join("tokenizer").to_string_lossy().to_string()),
        ..Default::default()
    };
    config.model_config.tokens = Some(String::new());
    config.model_config.num_threads = 4;
    config.model_config.debug = false;

    println!("正在加载 ASR 模型...");
    let recognizer = OfflineRecognizer::create(&config)
        .ok_or_else(|| anyhow::anyhow!("创建识别器失败"))?;
    println!("ASR 模型加载完成");
    Ok(recognizer)
}