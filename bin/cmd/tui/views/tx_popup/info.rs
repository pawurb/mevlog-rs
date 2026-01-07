use mevlog::misc::utils::GWEI_F64;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::cmd::tui::data::MEVTransactionJson;

const LABEL_WIDTH: usize = 19;

pub fn render_info_tab(tx: &MEVTransactionJson, area: Rect, frame: &mut Frame, scroll: u16) {
    let lines = build_tx_lines(tx);
    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(paragraph, area);
}

fn build_tx_lines(tx: &MEVTransactionJson) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    let from_display = tx
        .from_ens
        .as_ref()
        .map(|ens| format!("{} ({})", short_addr(&tx.from.to_string()), ens))
        .unwrap_or_else(|| tx.from.to_string());

    let to_display = tx
        .to
        .map(|addr| {
            tx.to_ens
                .as_ref()
                .map(|ens| format!("{} ({})", short_addr(&addr.to_string()), ens))
                .unwrap_or_else(|| addr.to_string())
        })
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
            lines.push(build_label_na_line("Coinbase Transfer:"));
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
            lines.push(build_label_na_line("Real Tx Cost:"));
        }
    }

    match tx.full_tx_cost {
        Some(full_cost) if tx.gas_used > 0 => {
            let real_gas_price = full_cost as f64 / tx.gas_used as f64 / GWEI_F64;
            lines.push(build_label_value_line(
                "Real Gas Price:",
                &format!("{:.2} GWEI", real_gas_price),
            ));
        }
        _ => {
            lines.push(build_label_na_line("Real Gas Price:"));
        }
    }

    if !tx.log_groups.is_empty() {
        lines.push(Line::from(""));

        let total_logs: usize = tx.log_groups.iter().map(|g| g.logs.len()).sum();
        lines.push(Line::from(Span::styled(
            format!("Events ({}):", total_logs),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));

        for group in &tx.log_groups {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(group.source.to_string(), Style::default().fg(Color::Green)),
                Span::styled(group.source.to_string(), Style::default().fg(Color::Green)),
            ]));

            for log in &group.logs {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled("emit ", Style::default().fg(Color::Yellow)),
                    Span::styled(log.signature.clone(), Style::default().fg(Color::Blue)),
                ]));
            }
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

fn build_label_na_line(label: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{:width$}", label, width = LABEL_WIDTH),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "N/A".to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn short_addr(addr: &str) -> String {
    if addr.len() > 12 {
        format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
    } else {
        addr.to_string()
    }
}
