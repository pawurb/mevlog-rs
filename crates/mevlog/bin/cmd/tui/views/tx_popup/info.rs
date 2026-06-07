use mevlog::misc::utils::GWEI_F64;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::cmd::tui::data::TransactionJson;

const LABEL_WIDTH: usize = 19;

pub fn render_info_tab(
    tx: &TransactionJson,
    area: Rect,
    frame: &mut Frame,
    scroll: u16,
    tx_trace_loading: bool,
) {
    let lines = build_tx_lines(tx, tx_trace_loading);
    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(paragraph, area);
}

fn build_tx_lines(tx: &TransactionJson, tx_trace_loading: bool) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    let from_display = tx.from.to_string();

    let to_display = tx
        .to
        .map(|addr| addr.to_string())
        .unwrap_or_else(|| "CREATE".to_string());

    lines.push(build_label_value_line("From:", &from_display));
    lines.push(build_label_value_line("To:", &to_display));
    lines.push(Line::from(vec![
        Span::styled(
            format!("{:width$}", "Method:", width = LABEL_WIDTH),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(tx.signature.clone(), Style::default().fg(Color::Magenta)),
    ]));

    if !tx.success {
        lines.push(Line::from(Span::styled(
            "Tx reverted!",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    }

    lines.push(build_label_value_line("Value:", &tx.display_value));

    let gas_price_gwei = tx.gas_price as f64 / GWEI_F64;
    lines.push(build_label_value_line(
        "Gas Price:",
        &format!("{:.2} GWEI", gas_price_gwei),
    ));

    lines.push(build_label_value_line("Gas Tx Cost:", &tx.display_tx_cost));

    match &tx.display_coinbase_transfer {
        Some(coinbase) => {
            let display = tx
                .display_coinbase_transfer_usd
                .as_ref()
                .map(|usd| format!("{} | {}", coinbase, usd))
                .unwrap_or_else(|| coinbase.clone());
            lines.push(build_label_value_line("Coinbase Transfer:", &display));
        }
        None => {
            lines.push(build_label_na_line("Coinbase Transfer:", tx_trace_loading));
        }
    }

    match &tx.display_full_tx_cost {
        Some(full_cost) => {
            let display = tx
                .display_full_tx_cost_usd
                .as_ref()
                .map(|usd| format!("{} | {}", full_cost, usd))
                .unwrap_or_else(|| full_cost.clone());
            lines.push(build_label_value_line("Real Tx Cost:", &display));
        }
        None => {
            lines.push(build_label_na_line("Real Tx Cost:", tx_trace_loading));
        }
    }

    match tx.full_tx_cost.as_ref().and_then(|c| c.parse::<f64>().ok()) {
        Some(full_cost) if tx.gas_used > 0 => {
            let real_gas_price = full_cost / tx.gas_used as f64 / GWEI_F64;
            lines.push(build_label_value_line(
                "Real Gas Price:",
                &format!("{:.2} GWEI", real_gas_price),
            ));
        }
        Some(_) => {
            lines.push(build_label_value_line("Real Gas Price:", "0.00 GWEI"));
        }
        None => {
            lines.push(build_label_na_line("Real Gas Price:", tx_trace_loading));
        }
    }

    if !tx.logs.is_empty() {
        lines.push(Line::from(""));

        lines.push(Line::from(Span::styled(
            format!("Events ({}):", tx.logs.len()),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));

        for log in &tx.logs {
            let signature = log
                .signature
                .clone()
                .unwrap_or_else(|| "<unknown>".to_string());
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(log.address.to_string(), Style::default().fg(Color::Green)),
            ]));
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled("emit ", Style::default().fg(Color::Yellow)),
                Span::styled(signature, Style::default().fg(Color::Blue)),
            ]));
        }
    }

    lines
}

fn build_label_value_line(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{:width$}", label, width = LABEL_WIDTH),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(value.to_string()),
    ])
}

fn build_label_na_line(label: &str, is_loading: bool) -> Line<'static> {
    let value = if is_loading { "Loading..." } else { "N/A" };
    Line::from(vec![
        Span::styled(
            format!("{:width$}", label, width = LABEL_WIDTH),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            value.to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}
