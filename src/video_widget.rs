use crate::{app::Action, audio_player::AudioPlayer, decoder::VideoDecoder};
use ffmpeg_next as ffmpeg;
use ratatui::{prelude::*, widgets::*};

pub struct VideoWidget {
    pub video_decoder: Option<VideoDecoder>,
    next_frame: Option<ffmpeg::frame::Video>,
    filepath: std::path::PathBuf,
    audio_player: AudioPlayer,
    resume_time: std::time::Instant,
    resume_frame_timestamp: std::time::Duration,
}

impl VideoWidget {
    pub fn new(filepath: std::path::PathBuf) -> anyhow::Result<Self> {
        Ok(Self {
            audio_player: AudioPlayer::new(&filepath)?,
            video_decoder: VideoDecoder::from_file(&filepath).ok(),
            next_frame: None,
            resume_time: std::time::Instant::now(),
            resume_frame_timestamp: std::time::Duration::ZERO,
            filepath,
        })
    }

    pub fn update(&mut self) -> anyhow::Result<()> {
        let real_timestamp = self.real_timestamp();
        if let Some(decoder) = self.video_decoder.as_mut() {
            decoder.common.real_timestamp = real_timestamp;
            self.next_frame = Some(decoder.next_frame()?);
        };
        Ok(())
    }

    /// Get the current video timestamp on where it should be based on the real time
    pub fn real_timestamp(&self) -> std::time::Duration {
        std::time::Instant::now() - self.resume_time + self.resume_frame_timestamp
    }

    pub fn frame_timestamp(&self) -> std::time::Duration {
        if let Some(decoder) = self.video_decoder.as_ref()
            && let Some(next_frame) = self.next_frame.as_ref()
        {
            decoder.common.timestamp(next_frame.pts())
        } else {
            self.real_timestamp()
        }
    }

    pub fn total_duration(&self) -> std::time::Duration {
        self.video_decoder
            .as_ref()
            .map(|d| d.common.total_duration())
            .unwrap_or(self.audio_player.total_duration())
    }

    pub fn action(&mut self, action: Action) -> anyhow::Result<()> {
        match action {
            Action::Resize(width, height) => {
                if let Some(decoder) = self.video_decoder.as_mut() {
                    decoder.set_output_size(width as u32, height as u32)?;
                }
            }
            Action::Resume => {
                self.resume_time = std::time::Instant::now();
                self.resume_frame_timestamp = self.frame_timestamp();
            }
            Action::Seek(timestamp) => {
                if let Some(decoder) = self.video_decoder.as_mut() {
                    decoder.common.seek(timestamp)?;
                    decoder.decoder.flush();
                }
                self.resume_time = std::time::Instant::now();
                self.resume_frame_timestamp = timestamp;
            }
            _ => (),
        }
        self.audio_player.action(action);
        Ok(())
    }
}

impl Widget for &VideoWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let filename = self
            .filepath
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let total_duration = self.total_duration().as_secs();
        let current_secs = self.frame_timestamp().as_secs();

        let video_info = self.video_decoder.as_ref().map(|d| {
            format!(
                "{} fps {}x{}",
                d.common.frame_rate(),
                d.decoder.width(),
                d.decoder.height(),
            )
        });

        let block = Block::bordered()
            .title_top(Line::from(filename).centered())
            .title_top(Line::from(video_info.unwrap_or_default()))
            .title_bottom(Line::from(format!(
                "{:0>2}:{:0>2} / {:0>2}:{:0>2}",
                current_secs / 60,
                current_secs % 60,
                total_duration / 60,
                total_duration % 60,
            )))
            .title_bottom(
                Line::from(vec![
                    " Pause ".into(),
                    "<K>".blue().bold(),
                    " Seek backwards ".into(),
                    "<J>".blue().bold(),
                    " Seek forwards ".into(),
                    "<L> ".blue().bold(),
                ])
                .right_aligned(),
            );

        if let Some(frame) = self.next_frame.as_ref() {
            for y in 0..area.height as usize {
                for x in 0..area.width as usize {
                    let pos = Position::new(x as u16 + area.x, y as u16 + area.y);
                    if let Some(row) = frame.data(0).get(y * frame.stride(0)..)
                        && let Some(rgb) = row.as_chunks::<3>().0.get(x)
                    {
                        let color = Color::Rgb(rgb[0], rgb[1], rgb[2]);
                        buf[pos].set_char(' ').set_bg(color);
                    }
                }
            }
        }

        block.render(area, buf);
    }
}

// fn get_char(brightness: u8) -> char {
//     let levels = [' ', '.', '\'', ':', '-', '*', '%', '#'];
//     let normalized = brightness as f32 / u8::MAX as f32;
//     levels[(normalized * levels.len() as f32) as usize]
// }
