mod data;

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    symbols::border,
    widgets::{Block, Cell, HighlightSpacing, Row, Table, TableState},
    DefaultTerminal, Frame,
};

use data::{DataFetcher, TxRow};
use mevlog::misc::shared_init::{ConnOpts, SharedOpts};

// Styles identical to hotpath
const HEADER_STYLE: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);
const SELECTED_ROW_STYLE: Style = Style::new()
    .bg(Color::DarkGray)
    .add_modifier(Modifier::BOLD);

#[derive(Debug, clap::Parser)]
pub struct TuiArgs {
    #[command(flatten)]
    shared_opts: SharedOpts,

    #[command(flatten)]
    conn_opts: ConnOpts,
}

pub struct App {
    table_state: TableState,
    items: Vec<TxRow>,
    exit: bool,
}

impl App {
    pub fn new(items: Vec<TxRow>) -> Self {
        Self {
            table_state: TableState::default().with_selected(if items.is_empty() { None } else { Some(0) }),
            items,
            exit: false,
        }
    }
}

impl TuiArgs {
    pub async fn run(&self) -> io::Result<()> {
        let fetcher = DataFetcher::new(
            self.conn_opts.rpc_url.clone(),
            self.conn_opts.chain_id,
        );

        let items = fetcher
            .fetch("latest")
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let mut terminal = ratatui::init();
        let app_result = App::new(items).run(&mut terminal);
        ratatui::restore();
        app_result
    }
}

impl App {
    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        self.render_table(frame.area(), frame);
    }

    fn render_table(&mut self, area: Rect, frame: &mut Frame) {
        let header = Row::new(vec![
            Cell::from("Block"),
            Cell::from("Tx Hash"),
            Cell::from("From"),
            Cell::from("To"),
            Cell::from("Value"),
            Cell::from("Gas Price"),
        ])
        .style(HEADER_STYLE)
        .height(1);

        let rows: Vec<Row> = self
            .items
            .iter()
            .map(|tx| {
                let tx_hash_short = if tx.tx_hash.len() > 10 {
                    format!("{}...", &tx.tx_hash[..10])
                } else {
                    tx.tx_hash.clone()
                };
                let from_short = if tx.from.len() > 10 {
                    format!("{}...", &tx.from[..10])
                } else {
                    tx.from.clone()
                };
                let to_short = tx.to.as_ref().map_or("-".to_string(), |t| {
                    if t.len() > 10 {
                        format!("{}...", &t[..10])
                    } else {
                        t.clone()
                    }
                });

                Row::new(vec![
                    Cell::from(tx.block_number.to_string()),
                    Cell::from(tx_hash_short),
                    Cell::from(from_short),
                    Cell::from(to_short),
                    Cell::from(tx.display_value.clone()),
                    Cell::from(format!("{}", tx.gas_price)),
                ])
            })
            .collect();

        let widths = [
            Constraint::Length(10),
            Constraint::Length(13),
            Constraint::Length(13),
            Constraint::Length(13),
            Constraint::Min(15),
            Constraint::Min(15),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::bordered()
                    .title(" Transactions ")
                    .border_set(border::THICK),
            )
            .column_spacing(1)
            .row_highlight_style(SELECTED_ROW_STYLE)
            .highlight_symbol(">> ")
            .highlight_spacing(HighlightSpacing::Always);

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.exit(),
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn select_next(&mut self) {
        let count = self.items.len();
        if count == 0 {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => (i + 1).min(count - 1),
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn select_previous(&mut self) {
        let count = self.items.len();
        if count == 0 {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => i.saturating_sub(1),
            None => 0,
        };
        self.table_state.select(Some(i));
    }
}
