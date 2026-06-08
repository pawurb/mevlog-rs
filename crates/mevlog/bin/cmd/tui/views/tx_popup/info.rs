use mevlog::misc::utils::GWEI_F64;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::cmd::tui::data::TransactionJson;

const LABEL_WIDTH: usize = 19;

pub(super) fn render_info_tab(tx: &TransactionJson, area: Rect, frame: &mut Frame, scroll: u16) -> u16 {
    let lines = build_tx_lines(tx);
    super::render_scrollable(area, frame, lines, scroll)
}

fn build_tx_lines(tx: &TransactionJson) -> Vec<Line<'static>> {
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

    {
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
