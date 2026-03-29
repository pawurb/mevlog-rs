mod app;
mod data;
mod views;
use std::io;

use app::App;
use mevlog::misc::shared_init::{ConnOpts, SharedOpts};

#[derive(Debug, clap::Parser)]
pub struct TuiArgs {
    #[command(flatten)]
    shared_opts: SharedOpts,

    #[command(flatten)]
    conn_opts: ConnOpts,
}

impl TuiArgs {
    pub async fn run(&self) -> io::Result<()> {
        let mut terminal = ratatui::init();
        let app_result = App::new(vec![], &self.conn_opts).run(&mut terminal);
        ratatui::restore();
        app_result
    }
}
