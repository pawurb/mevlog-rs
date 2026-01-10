use mevlog::misc::utils::GWEI_F64;
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    symbols::border,
    widgets::{Block, Cell, HighlightSpacing, Row, Table, TableState},
};

use crate::cmd::tui::data::MEVTransactionJson;

const HEADER_STYLE: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);
const SELECTED_ROW_STYLE: Style = Style::new()
    .bg(Color::DarkGray)
    .add_modifier(Modifier::BOLD);

pub struct TxsTable<'a> {
    items: &'a [MEVTransactionJson],
    title: Option<String>,
}

impl<'a> TxsTable<'a> {
    pub fn new(items: &'a [MEVTransactionJson]) -> Self {
        Self { items, title: None }
    }

    pub fn with_title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    pub fn render(&self, area: Rect, frame: &mut Frame, state: &mut TableState) {
        let header = Row::new(vec![
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
                let tx_hash = tx.tx_hash.to_string();
                let tx_hash_short = if tx_hash.len() > 10 {
                    format!("{}...", &tx_hash[..10])
                } else {
                    tx_hash
                };
                let from = tx.from.to_string();
                let from_short = if from.len() > 10 {
                    format!("{}...", &from[..10])
                } else {
                    from
                };
                let to_short = tx.to.map_or("-".to_string(), |t| {
                    let to = t.to_string();
                    if to.len() > 10 {
                        format!("{}...", &to[..10])
                    } else {
                        to
                    }
                });

                let gas_price_gwei = tx.gas_price as f64 / GWEI_F64;
                Row::new(vec![
                    Cell::from(tx_hash_short),
                    Cell::from(from_short),
                    Cell::from(to_short),
                    Cell::from(tx.display_value.clone()),
                    Cell::from(format!("{:.2} gwei", gas_price_gwei)),
                ])
            })
            .collect();

        let widths = [
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ];

        let title = self.title.clone().unwrap_or_else(|| {
            self.items
                .first()
                .map(|tx| format!(" Transactions (Block {}) ", tx.block_number))
                .unwrap_or_else(|| " Transactions ".to_string())
        });

        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::bordered().title(title).border_set(border::THICK))
            .column_spacing(1)
            .row_highlight_style(SELECTED_ROW_STYLE)
            .highlight_symbol(">> ")
            .highlight_spacing(HighlightSpacing::Always);

        frame.render_stateful_widget(table, area, state);
    }
}
