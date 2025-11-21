use crate::{app::Action, audio_player::AudioPlayer, decoder::VideoDecoder};
use ffmpeg_next as ffmpeg;
use ratatui::{prelude::*, widgets::*};

pub struct VideoWidget {
    video_decoder: Option<VideoDecoder>,
    next_frame: Option<ffmpeg::frame::Video>,
    filepath: std::path::PathBuf,
    audio_player: AudioPlayer,
    resume_time: std::time::Instant,
    pause_time: std::time::Instant,
}

impl VideoWidget {
    pub fn new(filepath: std::path::PathBuf) -> anyhow::Result<Self> {
        Ok(Self {
            audio_player: AudioPlayer::new(&filepath)?,
            video_decoder: VideoDecoder::from_file(&filepath).ok(),
            next_frame: None,
            resume_time: std::time::Instant::now(),
            pause_time: std::time::Instant::now(),
            filepath,
        })
    }

    pub fn update(&mut self) -> anyhow::Result<()> {
        if let Some(decoder) = self.video_decoder.as_mut() {
            self.next_frame = Some(decoder.next_frame()?);
        };
        Ok(())
    }

    pub fn wait_time(&self) -> Option<std::time::Duration> {
        let now = std::time::Instant::now();
        let decoder = self.video_decoder.as_ref()?;
        let next_frame = self.next_frame.as_ref()?;
        decoder
            .common
            .timestamp(next_frame.pts())
            .checked_sub(now - self.resume_time)
    }

    pub fn action(&mut self, action: Action) -> anyhow::Result<()> {
        match action {
            Action::Resize(width, height) => {
                if let Some(decoder) = self.video_decoder.as_mut() {
                    decoder.set_output_size(width as u32, height as u32)?;
                }
            }
            Action::Resume => self.resume_time += std::time::Instant::now() - self.pause_time,
            Action::Pause => self.pause_time = std::time::Instant::now(),
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

        let mut block = Block::bordered().title(Line::from(filename).centered());

        if let Some(frame) = self.next_frame.as_ref()
            && let Some(decoder) = self.video_decoder.as_ref()
        {
            let total_duration = decoder.common.total_duration().as_secs();
            let current_secs = decoder.common.timestamp(frame.pts()).as_secs();
            block = block.title_bottom(Line::from(format!(
                "{:0>2}:{:0>2} / {:0>2}:{:0>2}",
                current_secs / 60,
                current_secs % 60,
                total_duration / 60,
                total_duration % 60,
            )));

            for y in 0..area.height as usize {
                for x in 0..area.width as usize {
                    if let Some(luma) = frame.data(0).get(y * frame.stride(0) + x) {
                        let pos = Position::new(x as u16 + area.x, y as u16 + area.y);
                        buf[pos].set_char(get_char(*luma));
                    }
                }
            }
        }

        block.render(area, buf);
    }
}

fn get_char(brightness: u8) -> char {
    let levels = [' ', '.', '\'', ':', '-', '*', '%', '#'];
    let normalized = brightness as f32 / u8::MAX as f32;
    levels[(normalized * levels.len() as f32) as usize]
}
