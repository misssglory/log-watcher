mod app;
mod file_source;
mod filter;
mod highlight;
mod input;
mod model;
mod ui;

use std::{io, time::Duration};

use anyhow::Result;
use crossterm::{
  event::{self, Event, KeyEventKind},
  execute,
  terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
    LeaveAlternateScreen,
  },
};
use ratatui::{backend::CrosstermBackend, Terminal};

use model::App;

fn main() -> Result<()> {
  let args: Vec<std::path::PathBuf> =
    std::env::args().skip(1).map(Into::into).collect();

  let paths = if args.is_empty() {
    vec!["app.log".into(), "executor.log".into()]
  } else {
    args
  };

  let mut app = App::new(paths)?;

  enable_raw_mode()?;
  let mut stdout = io::stdout();
  execute!(stdout, EnterAlternateScreen)?;
  let backend = CrosstermBackend::new(stdout);
  let mut terminal = Terminal::new(backend)?;

  let tick_rate = Duration::from_millis(200);

  while !app.should_quit {
    app.poll_file_updates()?;
    terminal.draw(|f| ui::render(f, &mut app))?;

    if event::poll(tick_rate)? {
      if let Event::Key(key) = event::read()? {
        if key.kind == KeyEventKind::Press {
          input::handle_key(&mut app, key)?;
        }
      }
    }
  }

  disable_raw_mode()?;
  execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
  terminal.show_cursor()?;
  Ok(())
}
