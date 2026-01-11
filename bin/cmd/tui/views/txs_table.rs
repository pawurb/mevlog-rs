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
    show_block_number: bool,
}

impl<'a> TxsTable<'a> {
    pub fn new(items: &'a [MEVTransactionJson]) -> Self {
        Self {
            items,
            title: None,
            show_block_number: false,
        }
    }

    pub fn with_title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    pub fn with_block_number(mut self) -> Self {
        self.show_block_number = true;
        self
    }

    pub fn render(&self, area: Rect, frame: &mut Frame, state: &mut TableState) {
        let header_cells: Vec<Cell> = if self.show_block_number {
            vec![
                Cell::from("Block"),
                Cell::from("Index"),
                Cell::from("Hash"),
                Cell::from("Signature"),
                Cell::from("Gas Price"),
                Cell::from("Gas Cost"),
                Cell::from("Status"),
            ]
        } else {
            vec![
                Cell::from("Index"),
                Cell::from("Hash"),
                Cell::from("Signature"),
                Cell::from("Gas Price"),
                Cell::from("Gas Cost"),
                Cell::from("Status"),
            ]
        };

        let header = Row::new(header_cells).style(HEADER_STYLE).height(1);

        let rows: Vec<Row> = self
            .items
            .iter()
            .map(|tx| {
                let tx_hash = tx.tx_hash.to_string();
                let tx_hash_short = format!("{}...", &tx_hash[..12]);

                let signature = if tx.signature == "UNKNOWN" {
                    "<Unknown>".to_string()
                } else if tx.signature == "ETH_TRANSFER" {
                    "<ETH transfer>".to_string()
                } else {
                    tx.signature.clone()
                };

                let gas_price_gwei = tx.gas_price as f64 / GWEI_F64;
                let gas_cost = tx
                    .display_tx_cost_usd
                    .clone()
                    .unwrap_or_else(|| "-".to_string());

                let status = if tx.success { "✓" } else { "✗" };
                let status_style = if tx.success {
                    Style::new().fg(Color::Green)
                } else {
                    Style::new().fg(Color::Red)
                };

                let cells: Vec<Cell> = if self.show_block_number {
                    vec![
                        Cell::from(tx.block_number.to_string()).style(Style::new().fg(Color::Cyan)),
                        Cell::from(tx.index.to_string()).style(Style::new().fg(Color::Yellow)),
                        Cell::from(tx_hash_short).style(Style::new().fg(Color::Cyan)),
                        Cell::from(signature).style(Style::new().fg(Color::Red)),
                        Cell::from(format!("{:.2} gwei", gas_price_gwei)),
                        Cell::from(gas_cost).style(Style::new().fg(Color::Green)),
                        Cell::from(status).style(status_style),
                    ]
                } else {
                    vec![
                        Cell::from(tx.index.to_string()).style(Style::new().fg(Color::Yellow)),
                        Cell::from(tx_hash_short).style(Style::new().fg(Color::Cyan)),
                        Cell::from(signature).style(Style::new().fg(Color::Red)),
                        Cell::from(format!("{:.2} gwei", gas_price_gwei)),
                        Cell::from(gas_cost).style(Style::new().fg(Color::Green)),
                        Cell::from(status).style(status_style),
                    ]
                };

                Row::new(cells)
            })
            .collect();

        let widths: Vec<Constraint> = if self.show_block_number {
            vec![
                Constraint::Length(10),
                Constraint::Length(6),
                Constraint::Length(14),
                Constraint::Fill(1),
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Length(6),
            ]
        } else {
            vec![
                Constraint::Length(6),
                Constraint::Length(14),
                Constraint::Fill(1),
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Length(6),
            ]
        };

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
