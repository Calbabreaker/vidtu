use crate::{app::State, decoder::VideoDecoder};
use ratatui::{prelude::*, widgets::*};

pub struct VideoWidget {
    decoder: VideoDecoder,
    next_frame: ffmpeg_next::util::frame::Video,
    filepath: std::path::PathBuf,
}

impl VideoWidget {
    pub fn new(filepath: std::path::PathBuf) -> anyhow::Result<Self> {
        Ok(Self {
            decoder: VideoDecoder::from_file(&filepath)?,
            next_frame: ffmpeg_next::util::frame::Video::empty(),
            filepath,
        })
    }

    pub fn update(&mut self) -> anyhow::Result<State> {
        let last_frame_timestap = self.decoder.frame_timestamp(&self.next_frame);
        self.next_frame = self.decoder.next_frame()?;
        let timestamp = self.decoder.frame_timestamp(&self.next_frame);

        Ok(State::WaitForFrame(timestamp - last_frame_timestap))
    }

    pub fn resize(&mut self, width: u16, height: u16) -> anyhow::Result<()> {
        self.decoder.set_output_size(width as u32, height as u32)
    }
}

impl Widget for &VideoWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let frame = &self.next_frame;

        let filename = self
            .filepath
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let total_duration = self.decoder.total_duration().as_secs();
        let current_secs = self.decoder.frame_timestamp(frame).as_secs();
        let block = Block::bordered()
            .title(Line::from(filename).centered())
            .title_bottom(Line::from(format!(
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

        block.render(area, buf);
    }
}

fn get_char(brightness: u8) -> char {
    let levels = [' ', '.', '\'', ':', '-', '*', '%', '#'];
    let normalized = brightness as f32 / u8::MAX as f32;
    levels[(normalized * levels.len() as f32) as usize]
}
