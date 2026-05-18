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

pub struct AsrEngine {
    model_dir: PathBuf,
}

struct AsrState {
    buffer: Vec<f32>,
    offset: usize,
    speech_started: bool,
    started_time: Instant,
    final_text: String,
}

impl AsrEngine {
    pub fn new(model_dir: PathBuf) -> Self {
        Self { model_dir }
    }

    pub fn run<F>(
        &self,
        mut on_status: F,
        mut on_text: impl FnMut(String) + Send + 'static,
    ) where
        F: FnMut(String) + Send + 'static,
    {
        on_status("正在加载模型...".to_string());

        let vad = create_vad(&self.model_dir);
        let recognizer = create_recognizer(&self.model_dir);

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
        let mic_sample_rate = supported.sample_rate().0 as i32;
        let resampler = if mic_sample_rate != 16000 {
            println!("需要重采样: {} -> 16000", mic_sample_rate);
            Some(LinearResampler::create(mic_sample_rate, 16000).expect("创建重采样器失败"))
        } else {
            println!("不需要重采样");
            None
        };

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
        println!("音频流已启动");
        on_status("音频流已启动，等待说话...".to_string());

        let state = RefCell::new(AsrState {
            buffer: Vec::new(),
            offset: 0,
            speech_started: false,
            started_time: Instant::now(),
            final_text: String::new(),
        });

        let sample_rate = 16000i32;
        let window_size = 512usize;

        loop {
            while let Ok(samples) = rx.try_recv() {
                let mut s = state.borrow_mut();
                if let Some(ref resampler) = resampler {
                    let resampled = resampler.resample(&samples, false);
                    s.buffer.extend_from_slice(&resampled);
                } else {
                    s.buffer.extend_from_slice(&samples);
                }
            }

            {
                let mut s = state.borrow_mut();
                while s.offset + window_size <= s.buffer.len() {
                    vad.accept_waveform(&s.buffer[s.offset..s.offset + window_size]);
                    if !s.speech_started && vad.detected() {
                        s.speech_started = true;
                        s.started_time = Instant::now();
                    }
                    s.offset += window_size;
                }

                if !s.speech_started && s.buffer.len() > 10 * window_size {
                    let trim_amount = s.buffer.len() - 10 * window_size;
                    s.offset = s.offset.saturating_sub(trim_amount);
                    let new_buf = s.buffer[s.buffer.len() - 10 * window_size..].to_vec();
                    s.buffer = new_buf;
                }
            }

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

            while !vad.is_empty() {
                let mut s = state.borrow_mut();
                if let Some(segment) = vad.front() {
                    vad.pop();
                    let stream = recognizer.create_stream();
                    stream.accept_waveform(sample_rate, segment.samples());
                    recognizer.decode(&stream);
                    if let Some(result) = stream.get_result() {
                        if !result.text.is_empty() {
                            if !s.final_text.is_empty() {
                                s.final_text.push('。');
                            }
                            s.final_text.push_str(&result.text);
                            let display = s.final_text.clone();
                            on_text(display);
                        }
                    }
                }
                s.buffer.clear();
                s.offset = 0;
                s.speech_started = false;
            }

            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

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

fn create_vad(model_dir: &PathBuf) -> VoiceActivityDetector {
    let mut config = VadModelConfig::default();
    config.silero_vad.model = Some(model_dir.join("silero_vad.onnx").to_string_lossy().to_string());
    config.silero_vad.threshold = 0.5;
    config.silero_vad.min_silence_duration = 0.1;
    config.silero_vad.min_speech_duration = 0.25;
    config.silero_vad.max_speech_duration = 8.0;
    config.silero_vad.window_size = 512;
    config.sample_rate = 16000;
    config.debug = false;

    VoiceActivityDetector::create(&config, 20.0).expect("创建 VAD 失败")
}

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
