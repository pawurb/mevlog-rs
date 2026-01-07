use crate::cmd::tui::app::{AppMode, PrimaryTab};
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
const SELECT: &str = "<Enter/o> ";
const QUIT_LABEL: &str = " | Quit ";
const QUIT: &str = "<q>";

/// Renders the bottom controls bar showing context-aware keybindings
pub fn render_key_bindings(
    frame: &mut Frame,
    area: Rect,
    mode: &AppMode,
    active_tab: Option<PrimaryTab>,
    popup_open: bool,
) {
    let controls_line = match mode {
        AppMode::SelectNetwork => {
            if popup_open {
                Line::from(vec![
                    " Type to search ".into(),
                    " | Close ".into(),
                    "<Enter/Esc>".blue().bold(),
                ])
            } else {
                Line::from(vec![
                    NAV_NETWORKS.blue().bold(),
                    SELECT_LABEL.into(),
                    SELECT.blue().bold(),
                    " | Search ".into(),
                    "<s>".blue().bold(),
                    " | Clear ".into(),
                    "<c>".blue().bold(),
                    QUIT_LABEL.into(),
                    QUIT.blue().bold(),
                ])
            }
        }
        AppMode::Main => match active_tab {
            Some(PrimaryTab::Explore) => {
                if popup_open {
                    Line::from(vec![
                        "[1-3] ".blue().bold(),
                        "Tabs".into(),
                        " | Scroll ".into(),
                        "<n/m>".blue().bold(),
                        " | Close ".into(),
                        "<Esc/o>".blue().bold(),
                        QUIT_LABEL.into(),
                        QUIT.blue().bold(),
                    ])
                } else {
                    Line::from(vec![
                        NAV_TXS.blue().bold(),
                        NAV_BLOCKS_LABEL.into(),
                        NAV_BLOCKS.blue().bold(),
                        " | Open ".into(),
                        "<o>".blue().bold(),
                        QUIT_LABEL.into(),
                        QUIT.blue().bold(),
                    ])
                }
            }
            Some(PrimaryTab::Search) => Line::from(vec![
                NAV_TXS.blue().bold(),
                NAV_BLOCKS_LABEL.into(),
                NAV_BLOCKS.blue().bold(),
                QUIT_LABEL.into(),
                QUIT.blue().bold(),
            ]),
            None => Line::from(vec![]),
        },
    };

    let block = Block::bordered().border_set(border::PLAIN);

    let paragraph = Paragraph::new(controls_line).block(block).left_aligned();

    frame.render_widget(paragraph, area);
}
