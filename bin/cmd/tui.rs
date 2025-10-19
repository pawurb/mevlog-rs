use mevlog::{
    misc::{
        shared_init::{ConnOpts, SharedOpts, init_deps},
    },
};

use color_eyre::Result;
use crossterm::event::{self, Event};
use ratatui::{DefaultTerminal, Frame};

#[derive(Debug, clap::Parser)]
pub struct TuiArgs {
    #[command(flatten)]
    shared_opts: SharedOpts,

    #[command(flatten)]
    conn_opts: ConnOpts,
}

impl TuiArgs {
    pub async fn run(&self) -> Result<()> {
      color_eyre::install()?;
      let terminal = ratatui::init();
      let result = run(terminal);
      ratatui::restore();
      result
    }
}

fn run(mut terminal: DefaultTerminal) -> Result<()> {
    loop {
        terminal.draw(render)?;
        if matches!(event::read()?, Event::Key(_)) {
            break Ok(());
        }
    }
}

fn render(frame: &mut Frame) {
    frame.render_widget("hello world", frame.area());
}
