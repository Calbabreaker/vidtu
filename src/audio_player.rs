use std::fs::File;
use std::io::Write;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ffmpeg_next as ffmpeg;

use crate::{app::Action, decoder::AudioDecoder};

pub struct AudioPlayer {
    audio_stream: Option<cpal::Stream>,
    action_tx: std::sync::mpsc::Sender<Action>,
}

impl AudioPlayer {
    pub fn new(filepath: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let audio_decoder = AudioDecoder::from_file(&filepath);
        let mut audio_stream = None;

        let (action_tx, action_rx) = std::sync::mpsc::channel();

        if let Ok(mut audio_decoder) = audio_decoder
            && let Some(device) = cpal::default_host().default_output_device()
        {
            let config = device.default_output_config()?;
            audio_decoder.set_output_format(
                cpal_format_to_ffmpeg(config.sample_format())
                    .ok_or(anyhow::anyhow!("Unsupported output device data format"))?,
                config.channels() as u32,
                config.sample_rate().0,
            )?;

            let mut stream_state = StreamState::new(audio_decoder, action_rx);
            let stream = device.build_output_stream_raw(
                &config.config(),
                config.sample_format(),
                move |data, _| stream_state.data_callback(data.bytes_mut()),
                |err| panic!("err: {err}"),
                None,
            )?;
            stream.play()?;
            audio_stream = Some(stream);
        }

        Ok(Self {
            audio_stream,
            action_tx,
        })
    }

    pub fn action(&mut self, action: Action) {
        self.action_tx.send(action).unwrap();
    }
}

struct StreamState {
    audio_decoder: AudioDecoder,
    current_frame: Option<ffmpeg::frame::Audio>,
    frame_data_index: usize,
    action_rx: std::sync::mpsc::Receiver<Action>,
    paused: bool,
}

impl StreamState {
    fn new(mut audio_decoder: AudioDecoder, action_rx: std::sync::mpsc::Receiver<Action>) -> Self {
        Self {
            current_frame: audio_decoder.next_frame().ok(),
            audio_decoder,
            frame_data_index: 0,
            action_rx,
            paused: false,
        }
    }

    fn data_callback(&mut self, out: &mut [u8]) {
        while let Ok(action) = self.action_rx.try_recv() {
            match action {
                Action::Pause => self.paused = true,
                Action::Resume => self.paused = false,
                _ => (),
            }
        }

        if self.paused {
            for out_byte in out {
                *out_byte = 0;
            }
            return;
        }

        for out_byte in out {
            let Some(ref frame) = self.current_frame else {
                return;
            };

            *out_byte = frame.data(0)[self.frame_data_index];
            self.frame_data_index += 1;

            let frame_size = ffmpeg::format::sample::Buffer::size(
                frame.format(),
                frame.channels(),
                frame.samples(),
                false,
            );
            if self.frame_data_index >= frame_size {
                self.current_frame = self.audio_decoder.next_frame().ok();
                self.frame_data_index = 0;
            };
        }
    }
}

fn cpal_format_to_ffmpeg(format: cpal::SampleFormat) -> Option<ffmpeg::util::format::Sample> {
    use ffmpeg::{format::sample::Type, util::format::Sample};
    Some(match format {
        cpal::SampleFormat::I16 => Sample::I16(Type::Packed),
        cpal::SampleFormat::I32 => Sample::I32(Type::Packed),
        cpal::SampleFormat::I64 => Sample::I64(Type::Packed),
        cpal::SampleFormat::U8 => Sample::U8(Type::Packed),
        cpal::SampleFormat::F32 => Sample::F32(Type::Packed),
        cpal::SampleFormat::F64 => Sample::F64(Type::Packed),
        _ => return None,
    })
}
