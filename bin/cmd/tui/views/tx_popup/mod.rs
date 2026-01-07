mod info;
mod opcodes;
mod traces;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

use crate::cmd::tui::{app::TxPopupTab, data::MEVTransactionJson};

pub fn render_tx_popup(
    tx: &MEVTransactionJson,
    area: Rect,
    frame: &mut Frame,
    scroll: u16,
    active_tab: TxPopupTab,
    explorer_url: Option<&str>,
) {
    let popup_width = (area.width as f32 * 0.8) as u16;
    let popup_height = (area.height as f32 * 0.8) as u16;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect {
        x: area.x + x,
        y: area.y + y,
        width: popup_width,
        height: popup_height,
    };

    frame.render_widget(Clear, popup_area);

    let block = Block::bordered().border_set(border::DOUBLE);

    let inner_area = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Tx hash line
            Constraint::Length(1), // Tab bar
            Constraint::Length(1), // Empty line
            Constraint::Min(0),    // Content
        ])
        .split(inner_area);

    render_tx_hash_line(inner_chunks[0], frame, tx, explorer_url);
    render_popup_tab_bar(inner_chunks[1], frame, active_tab);

    match active_tab {
        TxPopupTab::Info => info::render_info_tab(tx, inner_chunks[3], frame, scroll),
        TxPopupTab::Opcodes => opcodes::render_opcodes_tab(inner_chunks[3], frame),
        TxPopupTab::Traces => traces::render_traces_tab(inner_chunks[3], frame),
    }
}

fn render_tx_hash_line(
    area: Rect,
    frame: &mut Frame,
    tx: &MEVTransactionJson,
    explorer_url: Option<&str>,
) {
    let tx_hash = format!("0x{}", tx.tx_hash);
    let display_text = explorer_url
        .map(|url| format!("{}/tx/{}", url.trim_end_matches('/'), tx_hash))
        .unwrap_or(tx_hash);

    let line = Line::from(Span::styled(
        display_text,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));
    let paragraph = Paragraph::new(line);
    frame.render_widget(paragraph, area);
}


fn render_popup_tab_bar(area: Rect, frame: &mut Frame, active_tab: TxPopupTab) {
    let tabs = [
        (TxPopupTab::Info, "1", "Info"),
        (TxPopupTab::Opcodes, "2", "Opcodes"),
        (TxPopupTab::Traces, "3", "Traces"),
    ];

    let mut spans = Vec::new();

    for (i, (tab, num, name)) in tabs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  |  ", Style::default().fg(Color::DarkGray)));
        }

        let is_active = *tab == active_tab;

        spans.push(Span::styled("[", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(*num, Style::default().fg(Color::Yellow)));
        spans.push(Span::styled("] ", Style::default().fg(Color::DarkGray)));

        let tab_style = if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        spans.push(Span::styled(*name, tab_style));

        let indicator = if is_active { "*" } else { " " };
        spans.push(Span::styled(indicator, Style::default().fg(Color::Yellow)));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line);

    frame.render_widget(paragraph, area);
}
