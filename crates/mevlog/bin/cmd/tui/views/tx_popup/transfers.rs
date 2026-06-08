use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use revm::primitives::Address;

use crate::cmd::tui::data::TransactionJson;

const ERC20_TRANSFER_SIGNATURE: &str = "Transfer(address,address,uint256)";

pub(super) fn render_transfers_tab(
    tx: &TransactionJson,
    area: Rect,
    frame: &mut Frame,
    scroll: u16,
) -> u16 {
    let lines = build_transfers_lines(tx);
    super::render_scrollable(area, frame, lines, scroll)
}

fn build_transfers_lines(tx: &TransactionJson) -> Vec<Line<'static>> {
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
        .logs
        .iter()
        .filter(|log| log.signature.as_deref() == Some(ERC20_TRANSFER_SIGNATURE))
        .collect();

    lines.push(Line::from(Span::styled(
        format!("ERC20 ({}):", erc20_transfers.len()),
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));

    for log in erc20_transfers {
        if let (Some(topic1), Some(topic2)) = (&log.topic1, &log.topic2) {
            let from = extract_address_from_topic(topic1);
            let to = extract_address_from_topic(topic2);

            let amount_display = log.erc20_amount.clone().unwrap_or_else(|| "?".to_string());

            let token_display = log.address.to_string();

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
