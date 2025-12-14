use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    symbols::border,
    widgets::{Block, Cell, HighlightSpacing, Row, Table, TableState},
    DefaultTerminal, Frame,
};

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
    items: Vec<(String, String, String)>,
    exit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            items: vec![
                ("A1".to_string(), "B1".to_string(), "C1".to_string()),
                ("A2".to_string(), "B2".to_string(), "C2".to_string()),
                ("A3".to_string(), "B3".to_string(), "C3".to_string()),
            ],
            table_state: TableState::default().with_selected(0),
            exit: false,
        }
    }
}

impl TuiArgs {
    pub async fn run(&self) -> io::Result<()> {
        let mut terminal = ratatui::init();
        let app_result = App::default().run(&mut terminal);
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
        let header = Row::new(vec![Cell::from("A"), Cell::from("B"), Cell::from("C")])
            .style(HEADER_STYLE)
            .height(1);

        let rows: Vec<Row> = self
            .items
            .iter()
            .map(|(a, b, c)| {
                Row::new(vec![
                    Cell::from(a.as_str()),
                    Cell::from(b.as_str()),
                    Cell::from(c.as_str()),
                ])
            })
            .collect();

        let widths = [
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::bordered()
                    .title(" Table ")
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
