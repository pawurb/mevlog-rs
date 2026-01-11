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
    explorer_url: Option<&'a str>,
}

impl<'a> TxsTable<'a> {
    pub fn new(items: &'a [MEVTransactionJson]) -> Self {
        Self {
            items,
            title: None,
            show_block_number: false,
            explorer_url: None,
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

    pub fn with_explorer_url(mut self, url: Option<&'a str>) -> Self {
        self.explorer_url = url;
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

        let visible_rows = area.height.saturating_sub(3) as usize;
        let total = self.items.len();

        if total == 0 || visible_rows == 0 {
            let table = Table::new(Vec::<Row>::new(), Vec::<Constraint>::new())
                .header(header)
                .block(
                    Block::bordered()
                        .title(
                            self.title
                                .clone()
                                .unwrap_or_else(|| " Transactions ".into()),
                        )
                        .border_set(border::THICK),
                );
            frame.render_widget(table, area);
            return;
        }

        let selected = state.selected().unwrap_or(0);
        let offset = if selected < visible_rows {
            0
        } else {
            (selected - visible_rows + 1).min(total.saturating_sub(visible_rows))
        };
        let end = (offset + visible_rows).min(total);
        let visible_items = &self.items[offset..end];

        let rows: Vec<Row> = visible_items
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

        let title = if let Some(custom_title) = &self.title {
            format!(
                "{} [{}-{} of {}] ",
                custom_title.trim(),
                offset + 1,
                end,
                total
            )
        } else if let Some(tx) = self.items.first() {
            let block_info = if let Some(explorer) = self.explorer_url {
                format!(
                    "{}/block/{}",
                    explorer.trim_end_matches('/'),
                    tx.block_number
                )
            } else {
                format!("Block {}", tx.block_number)
            };
            format!(
                " Transactions ({}) [{}-{} of {}] ",
                block_info,
                offset + 1,
                end,
                total
            )
        } else {
            " Transactions ".to_string()
        };

        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::bordered().title(title).border_set(border::THICK))
            .column_spacing(1)
            .row_highlight_style(SELECTED_ROW_STYLE)
            .highlight_symbol(">> ")
            .highlight_spacing(HighlightSpacing::Always);

        let relative_selected = selected - offset;
        let mut render_state = TableState::default().with_selected(Some(relative_selected));
        frame.render_stateful_widget(table, area, &mut render_state);
    }
}
