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
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

// ASR 引擎句柄
// 封装 Sherpa-onnx 识别器和 VAD，负责音频采集和转写
pub struct AsrEngine {
    model_dir: PathBuf, // 模型文件所在目录
}

// ASR 内部状态
// 用于在音频循环中追踪语音检测和转写进度
struct AsrState {
    buffer: Vec<f32>,        // 音频缓冲区（单声道，16kHz）
    offset: usize,           // 已处理音频偏移量
    speech_started: bool,    // 是否已开始检测到语音
    started_time: Instant,   // 语音开始时间（用于超时检测）
    final_text: String,     // 最终识别文本（累加）
}

impl AsrEngine {
    // 创建 ASR 引擎
    pub fn new(model_dir: PathBuf) -> Self {
        Self { model_dir }
    }

    /// 运行语音识别循环
    /// - on_status: 状态回调（加载状态、设备状态等）
    /// - on_text: 识别结果回调（实时文本更新）
    pub fn run<F>(
        &self,
        mut on_status: F,
        mut on_text: impl FnMut(String) + Send + 'static,
    ) where
        F: FnMut(String) + Send + 'static,
    {
        on_status("正在加载模型...".to_string());

        // 初始化 VAD 和识别器
        let vad = create_vad(&self.model_dir);
        let recognizer = create_recognizer(&self.model_dir);

        // 获取音频主机和输入设备
        let host = cpal::default_host();
        let device = match list_input_devices(&host) {
            Ok(d) => d,
            Err(e) => {
                on_status(format!("设备错误: {}", e));
                return;
            }
        };

        // 获取设备默认配置
        let supported = match device.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                on_status(format!("配置错误: {}", e));
                return;
            }
        };

        let mic_sample_rate = supported.sample_rate().0 as i32;

        // 检查是否需要重采样（麦克风采样率转 16kHz）
        let resampler = if mic_sample_rate != 16000 {
            println!("需要重采样: {} -> 16000", mic_sample_rate);
            Some(LinearResampler::create(mic_sample_rate, 16000).expect("创建重采样器失败"))
        } else {
            println!("不需要重采样");
            None
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

        // 启动音频流
        if let Err(e) = audio_stream.play() {
            on_status(format!("播放错误: {}", e));
            return;
        }
        println!("音频流已启动");
        on_status("音频流已启动，等待说话...".to_string());

        // 初始化状态
        let state = RefCell::new(AsrState {
            buffer: Vec::new(),
            offset: 0,
            speech_started: false,
            started_time: Instant::now(),
            final_text: String::new(),
        });

        let sample_rate = 16000i32;
        let window_size = 512usize;

        // 主循环：从 channel 接收音频数据，进行 VAD 和识别
        loop {
            // 接收麦克风音频数据
            while let Ok(samples) = rx.try_recv() {
                let mut s = state.borrow_mut();
                if let Some(ref resampler) = resampler {
                    // 需要重采样
                    let resampled = resampler.resample(&samples, false);
                    s.buffer.extend_from_slice(&resampled);
                } else {
                    s.buffer.extend_from_slice(&samples);
                }
            }

            // VAD 检测：处理音频窗口
            {
                let mut s = state.borrow_mut();
                while s.offset + window_size <= s.buffer.len() {
                    vad.accept_waveform(&s.buffer[s.offset..s.offset + window_size]);
                    if !s.speech_started && vad.detected() {
                        // 检测到语音开始
                        s.speech_started = true;
                        s.started_time = Instant::now();
                    }
                    s.offset += window_size;
                }

                // 如果没有检测到语音且缓冲区过长，丢弃旧数据
                if !s.speech_started && s.buffer.len() > 10 * window_size {
                    let trim_amount = s.buffer.len() - 10 * window_size;
                    s.offset = s.offset.saturating_sub(trim_amount);
                    let new_buf = s.buffer[s.buffer.len() - 10 * window_size..].to_vec();
                    s.buffer = new_buf;
                }
            }

            // 实时识别：语音开始后定期进行识别
            {
                let mut s = state.borrow_mut();
                let elapsed = s.started_time.elapsed().as_secs_f32();
                if s.speech_started && elapsed > 0.2 {
                    let stream = recognizer.create_stream();
                    stream.accept_waveform(sample_rate, &s.buffer);
                    recognizer.decode(&stream);
                    if let Some(result) = stream.get_result() {
                        let display = format!("{}{}", s.final_text, result.text);
                        on_text(display);
                    }
                    s.started_time = Instant::now();
                }
            }

            // VAD 分段检测：处理已完成的语音片段
            while !vad.is_empty() {
                let mut s = state.borrow_mut();
                if let Some(segment) = vad.front() {
                    vad.pop();
                    let stream = recognizer.create_stream();
                    stream.accept_waveform(sample_rate, segment.samples());
                    recognizer.decode(&stream);
                    if let Some(result) = stream.get_result() {
                        if !result.text.is_empty() {
                            // 添加分隔符并累加文本
                            if !s.final_text.is_empty() {
                                s.final_text.push('。');
                            }
                            s.final_text.push_str(&result.text);
                            let display = s.final_text.clone();
                            on_text(display);
                        }
                    }
                }
                // 重置状态，准备下一段语音
                s.buffer.clear();
                s.offset = 0;
                s.speech_started = false;
            }

            // 休眠避免 CPU 占用过高
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// 列出可用的输入设备并返回默认设备
fn list_input_devices(host: &cpal::Host) -> Result<cpal::Device> {
    let default_input = host.default_input_device();
    let default_name = default_input.as_ref().map(|d| d.name().unwrap_or_default());

    println!("可用的输入设备:");
    for device in host.input_devices()? {
        let name = device.name().unwrap_or("<unknown>".to_string());
        // 标记默认设备
        let mark = if Some(&name) == default_name.as_ref() { "*" } else { " " };
        println!("{} {}", mark, name);
    }

    let device = default_input.ok_or_else(|| anyhow::anyhow!("没有默认输入设备"))?;
    println!("使用默认设备: {}", device.name()?);
    Ok(device)
}

/// 构建音频输入流
/// 支持多种采样格式（F32/I16/U16），自动转换为单声道
fn build_input_stream(device: &cpal::Device, tx: mpsc::Sender<Vec<f32>>) -> Result<cpal::Stream> {
    let supported = device.default_input_config()?;
    let config = supported.config();
    let sample_format = supported.sample_format();
    let channels = config.channels as usize;

    // 错误回调
    let err_fn = |err| eprintln!("音频流错误: {:?}", err);

    println!(
        "输入格式: {:?}, 声道: {}, 采样率: {}",
        sample_format, channels, config.sample_rate.0
    );

    // 根据采样格式构建输入流
    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _| {
                if data.is_empty() { return; }
                // 多声道转单声道：取平均值
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
                // I16 转 F32 并归一化到 [-1, 1]
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
                // U16 转 F32（偏移 32768）并归一化
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

/// 创建语音活动检测器（VAD）
/// 使用 Silero VAD 模型检测语音边界
fn create_vad(model_dir: &PathBuf) -> VoiceActivityDetector {
    let mut config = VadModelConfig::default();
    config.silero_vad.model = Some(model_dir.join("silero_vad.onnx").to_string_lossy().to_string());
    config.silero_vad.threshold = 0.5;                    // 检测阈值
    config.silero_vad.min_silence_duration = 0.1;        // 最小静音时长（秒）
    config.silero_vad.min_speech_duration = 0.25;         // 最小语音时长（秒）
    config.silero_vad.max_speech_duration = 8.0;         // 最大语音时长（秒）
    config.silero_vad.window_size = 512;
    config.sample_rate = 16000;
    config.debug = false;

    VoiceActivityDetector::create(&config, 20.0).expect("创建 VAD 失败")
}

/// 创建离线识别器
/// 使用 Qwen3 ASR 模型进行语音转文字
fn create_recognizer(model_dir: &PathBuf) -> OfflineRecognizer {
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.qwen3_asr = OfflineQwen3ASRModelConfig {
        conv_frontend: Some(model_dir.join("conv_frontend.onnx").to_string_lossy().to_string()),
        encoder: Some(model_dir.join("encoder.int8.onnx").to_string_lossy().to_string()),
        decoder: Some(model_dir.join("decoder.int8.onnx").to_string_lossy().to_string()),
        tokenizer: Some(model_dir.join("tokenizer").to_string_lossy().to_string()),
        ..Default::default()
    };
    config.model_config.tokens = Some(String::new());
    config.model_config.num_threads = 4;
    config.model_config.debug = false;

    println!("正在加载模型...");
    let recognizer = OfflineRecognizer::create(&config).expect("创建识别器失败");
    println!("模型加载完成");
    recognizer
}
