use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};
use revm::primitives::Address;

use crate::cmd::tui::data::MEVTransactionJson;

const ERC20_TRANSFER_SIGNATURE: &str = "Transfer(address,address,uint256)";

pub fn render_transfers_tab(tx: &MEVTransactionJson, area: Rect, frame: &mut Frame, scroll: u16) {
    let lines = build_transfers_lines(tx);

    if lines.is_empty() {
        let paragraph =
            Paragraph::new("No transfers found").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(paragraph, area);
}

fn build_transfers_lines(tx: &MEVTransactionJson) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut index = 0usize;

    let has_native_transfer = tx.value != "0";
    if has_native_transfer {
        lines.push(Line::from(Span::styled(
            "Native:",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));

        if let Some(to) = tx.to {
            append_transfer_lines(
                &mut lines,
                index,
                &tx.from.to_string(),
                &to.to_string(),
                &tx.display_value,
                None,
            );
            index += 1;
        }
        lines.push(Line::from(""));
    }

    let erc20_transfers: Vec<_> = tx
        .log_groups
        .iter()
        .flat_map(|group| {
            group
                .logs
                .iter()
                .filter(|log| log.signature == ERC20_TRANSFER_SIGNATURE)
                .map(move |log| (group.source, log))
        })
        .collect();

    if !erc20_transfers.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("ERC20 ({}):", erc20_transfers.len()),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));

        for (token_address, log) in erc20_transfers {
            if log.topics.len() >= 3 {
                let from = extract_address_from_topic(&log.topics[1]);
                let to = extract_address_from_topic(&log.topics[2]);

                let amount_display = log.amount.clone().unwrap_or_else(|| "?".to_string());

                let token_display = log
                    .symbol
                    .clone()
                    .unwrap_or_else(|| token_address.to_string());

                append_transfer_lines(
                    &mut lines,
                    index,
                    &from.to_string(),
                    &to.to_string(),
                    &amount_display,
                    Some(&token_display),
                );
                index += 1;
            }
        }
    }

    lines
}

fn append_transfer_lines(
    lines: &mut Vec<Line<'static>>,
    index: usize,
    from: &str,
    to: &str,
    amount: &str,
    token_symbol: Option<&str>,
) {
    lines.push(Line::from(Span::styled(
        format!("  [{}]", index),
        Style::default().fg(Color::Yellow),
    )));

    lines.push(Line::from(vec![
        Span::styled("    From: ", Style::default().fg(Color::White)),
        Span::styled(from.to_string(), Style::default().fg(Color::Cyan)),
    ]));

    lines.push(Line::from(vec![
        Span::styled("    To:   ", Style::default().fg(Color::White)),
        Span::styled(to.to_string(), Style::default().fg(Color::Magenta)),
    ]));

    let mut amount_spans = vec![
        Span::raw("    "),
        Span::styled(amount.to_string(), Style::default().fg(Color::White)),
    ];

    if let Some(symbol) = token_symbol {
        amount_spans.push(Span::raw(" "));
        amount_spans.push(Span::styled(
            symbol.to_string(),
            Style::default().fg(Color::Yellow),
        ));
    }

    lines.push(Line::from(amount_spans));
}

fn extract_address_from_topic(topic: &revm::primitives::FixedBytes<32>) -> Address {
    let bytes = topic.as_slice();
    Address::from_slice(&bytes[12..32])
}
