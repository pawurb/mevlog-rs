use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    symbols::border,
    widgets::{Block, Cell, HighlightSpacing, Row, Table, TableState},
};

use crate::cmd::tui::data::TxRow;

const HEADER_STYLE: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);
const SELECTED_ROW_STYLE: Style = Style::new()
    .bg(Color::DarkGray)
    .add_modifier(Modifier::BOLD);

pub struct TxsTable<'a> {
    items: &'a [TxRow],
}

impl<'a> TxsTable<'a> {
    pub fn new(items: &'a [TxRow]) -> Self {
        Self { items }
    }

    pub fn render(&self, area: Rect, frame: &mut Frame, state: &mut TableState) {
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

        frame.render_stateful_widget(table, area, state);
    }
}
