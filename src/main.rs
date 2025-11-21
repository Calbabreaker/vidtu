use ffmpeg_next as ffmpeg;

use crate::app::App;

mod app;
mod audio_player;
mod decoder;
mod video_widget;

fn main() -> anyhow::Result<()> {
    ffmpeg::init()?;
    let mut app = App::new()?;
    let result = app.run(&mut ratatui::init());
    ratatui::restore();
    result
}
