use ratatui::crossterm::event::*;

use crate::video_widget::VideoWidget;

#[derive(PartialEq)]
pub enum State {
    Exited,
    Playing,
    Paused,
}

#[derive(PartialEq, Debug)]
pub enum Action {
    Pause,
    Resume,
    Resize(u16, u16),
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
            state: State::Playing,
            video_widget: VideoWidget::new(filepath.into())?,
        })
    }

    pub fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> anyhow::Result<()> {
        let area = terminal.get_frame().area();
        self.action(Action::Resize(area.width, area.height))?;
        while self.state != State::Exited {
            self.handle_events()?;
            terminal.draw(|frame| self.draw(frame))?;
        }
        Ok(())
    }

    pub fn draw(&self, frame: &mut ratatui::Frame) {
        frame.render_widget(&self.video_widget, frame.area());
    }

    fn handle_events(&mut self) -> anyhow::Result<()> {
        if self.state == State::Playing {
            if self.video_widget.update().is_err() {
                self.state = State::Paused;
            }
            if !ratatui::crossterm::event::poll(self.video_widget.wait_time().unwrap_or_default())?
            {
                return Ok(());
            }
        }

        match ratatui::crossterm::event::read()? {
            Event::Key(event) if event.kind == KeyEventKind::Press => match event.code {
                KeyCode::Char('q') => self.state = State::Exited,
                KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.state = State::Exited;
                }
                KeyCode::Char('k') => {
                    self.action(if self.state == State::Paused {
                        Action::Resume
                    } else {
                        Action::Pause
                    })?;
                }
                _ => {}
            },
            Event::Resize(width, height) => self.action(Action::Resize(width, height))?,
            _ => {}
        };
        Ok(())
    }

    fn action(&mut self, action: Action) -> anyhow::Result<()> {
        match action {
            Action::Pause => self.state = State::Paused,
            Action::Resume => self.state = State::Playing,
            _ => (),
        };
        self.video_widget.action(action)
    }
}
