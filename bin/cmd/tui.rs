mod app;
mod data;
mod views;

use std::io;

use app::App;
use data::DataFetcher;
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
        let fetcher = DataFetcher::new(self.conn_opts.rpc_url.clone(), self.conn_opts.chain_id);

        let items = fetcher.fetch("latest").await.map_err(io::Error::other)?;

        let mut terminal = ratatui::init();
        let app_result = App::new(items).run(&mut terminal);
        ratatui::restore();
        app_result
    }
}
