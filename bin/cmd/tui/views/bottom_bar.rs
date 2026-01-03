use crate::cmd::tui::app::{AppMode, Tab};
use ratatui::{
    Frame,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{Block, Paragraph},
};

// Control text constants
const NAV_TXS: &str = " <↑/↓/j/k> ";
const NAV_BLOCKS_LABEL: &str = " | Nav Blocks ";
const NAV_BLOCKS: &str = "<←/→/h/l> ";
const NAV_NETWORKS: &str = " <↑/↓/j/k> ";
const SELECT_LABEL: &str = " | Select ";
const SELECT: &str = "<Enter> ";
const QUIT_LABEL: &str = " | Quit ";
const QUIT: &str = "<q>";

/// Renders the bottom controls bar showing context-aware keybindings
pub fn render_key_bindings(frame: &mut Frame, area: Rect, mode: &AppMode, active_tab: Option<Tab>) {
    let controls_line = match mode {
        AppMode::SelectNetwork => Line::from(vec![
            NAV_NETWORKS.blue().bold(),
            SELECT_LABEL.into(),
            SELECT.blue().bold(),
            QUIT_LABEL.into(),
            QUIT.blue().bold(),
        ]),
        AppMode::Main => match active_tab {
            Some(Tab::Explore) => Line::from(vec![
                NAV_TXS.blue().bold(),
                NAV_BLOCKS_LABEL.into(),
                NAV_BLOCKS.blue().bold(),
                QUIT_LABEL.into(),
                QUIT.blue().bold(),
            ]),
            Some(Tab::Search) => Line::from(vec![QUIT_LABEL.into(), QUIT.blue().bold()]),
            None => Line::from(vec![]),
        },
    };

    let block = Block::bordered().border_set(border::PLAIN);

    let paragraph = Paragraph::new(controls_line).block(block).left_aligned();

    frame.render_widget(paragraph, area);
}
