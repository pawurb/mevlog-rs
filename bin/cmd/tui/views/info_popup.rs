use mevlog::{ChainEntryJson, misc::shared_init::ConnOpts};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Wrap},
};

const POPUP_WIDTH: u16 = 80;
const POPUP_HEIGHT: u16 = 12;

pub fn render_info_popup(
    area: Rect,
    frame: &mut Frame,
    chain: Option<&ChainEntryJson>,
    conn_opts: &ConnOpts,
    rpc_refreshing: bool,
) {
    let popup_width = POPUP_WIDTH.min(area.width.saturating_sub(4));
    let popup_height = POPUP_HEIGHT.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect {
        x: area.x + x,
        y: area.y + y,
        width: popup_width,
        height: popup_height,
    };

    frame.render_widget(Clear, popup_area);

    let block = Block::bordered()
        .border_set(border::DOUBLE)
        .title(" RPC Info ");

    let inner_area = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Chain name
            Constraint::Length(1), // Chain ID
            Constraint::Length(1), // Network
            Constraint::Length(1), // Explorer
            Constraint::Length(1), // Empty
            Constraint::Length(1), // RPC URL label
            Constraint::Min(1),    // RPC URL (can wrap)
        ])
        .split(inner_area);

    let chain_name = chain.map(|c| c.name.as_str()).unwrap_or("Unknown");
    let chain_id = chain
        .map(|c| c.chain_id)
        .or(conn_opts.chain_id)
        .map(|id| id.to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    let network = chain.map(|c| c.chain.as_str()).unwrap_or("Unknown");
    let explorer = chain
        .and_then(|c| c.explorer_url.as_deref())
        .unwrap_or("N/A");

    let rpc_url = conn_opts.rpc_url.as_deref().unwrap_or("Resolving...");

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Chain: ", Style::default().fg(Color::DarkGray)),
            Span::styled(chain_name, Style::default().fg(Color::White)),
        ])),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Chain ID: ", Style::default().fg(Color::DarkGray)),
            Span::styled(chain_id, Style::default().fg(Color::Yellow)),
        ])),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Network: ", Style::default().fg(Color::DarkGray)),
            Span::styled(network, Style::default().fg(Color::White)),
        ])),
        chunks[2],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Explorer: ", Style::default().fg(Color::DarkGray)),
            Span::styled(explorer, Style::default().fg(Color::Cyan)),
        ])),
        chunks[3],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            "RPC URL: ",
            Style::default().fg(Color::DarkGray),
        )])),
        chunks[5],
    );

    let rpc_style = if rpc_refreshing {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::ITALIC)
    } else {
        Style::default().fg(Color::Green)
    };

    let rpc_display = if rpc_refreshing {
        "Refreshing..."
    } else {
        rpc_url
    };

    frame.render_widget(
        Paragraph::new(Span::styled(rpc_display, rpc_style)).wrap(Wrap { trim: false }),
        chunks[6],
    );
}
