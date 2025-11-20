use ratatui::crossterm::event::*;

use crate::video_widget::VideoWidget;

#[derive(PartialEq)]
pub enum State {
    Exited,
    WaitForFrame(std::time::Duration),
    Paused,
}

pub struct App {
    video_widget: VideoWidget,
    state: State,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let filepath = std::env::args()
            .nth(1)
            .ok_or(anyhow::anyhow!("Expected one argument"))?;

        Ok(Self {
            state: State::WaitForFrame(std::time::Duration::ZERO),
            video_widget: VideoWidget::new(filepath.into())?,
        })
    }

    pub fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> anyhow::Result<()> {
        let area = terminal.get_frame().area();
        self.resize(area.width, area.height)?;
        while self.state != State::Exited {
            if self.state != State::Paused {
                self.state = self.video_widget.update()?;
            }
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    pub fn draw(&self, frame: &mut ratatui::Frame) {
        frame.render_widget(&self.video_widget, frame.area());
    }

    fn handle_events(&mut self) -> anyhow::Result<()> {
        if let State::WaitForFrame(duration) = self.state
            && !ratatui::crossterm::event::poll(duration)?
        {
            return Ok(());
        }

        match ratatui::crossterm::event::read()? {
            Event::Key(event) if event.kind == KeyEventKind::Press => match event.code {
                KeyCode::Char('q') => self.state = State::Exited,
                KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.state = State::Exited;
                }
                KeyCode::Char('k') => {
                    self.state = if self.state == State::Paused {
                        State::WaitForFrame(std::time::Duration::ZERO)
                    } else {
                        State::Paused
                    }
                }
                _ => {}
            },
            Event::Resize(width, height) => self.resize(width, height)?,
            _ => {}
        };
        Ok(())
    }

    fn resize(&mut self, width: u16, height: u16) -> anyhow::Result<()> {
        self.video_widget.resize(width, height)
    }
}
