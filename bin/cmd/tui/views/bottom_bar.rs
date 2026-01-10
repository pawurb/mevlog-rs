use crate::cmd::tui::app::{AppMode, PrimaryTab, TxPopupTab};
use ratatui::{
    Frame,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{Block, Paragraph},
};

// Control text constants
const NAV_KEYS: &str = " <↑/↓/j/k> <←/→/h/l> ";
const NAV_NETWORKS: &str = " <↑/↓/j/k> ";
const SELECT_LABEL: &str = " | Select ";
const SELECT: &str = "<Enter/o> ";
const QUIT_LABEL: &str = " | Quit ";
const QUIT: &str = "<q>";

/// Renders the bottom controls bar showing context-aware keybindings
#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
pub fn render_key_bindings(
    frame: &mut Frame,
    area: Rect,
    mode: &AppMode,
    active_tab: Option<PrimaryTab>,
    search_popup_open: bool,
    tx_popup_open: bool,
    tx_popup_tab: TxPopupTab,
    block_popup_open: bool,
    info_popup_open: bool,
    can_go_back: bool,
) {
    let controls_line = match mode {
        AppMode::SelectNetwork => {
            if search_popup_open {
                Line::from(vec![
                    " Type to search ".into(),
                    " | Close ".into(),
                    "<Enter/Esc>".blue().bold(),
                ])
            } else {
                let mut items = vec![
                    NAV_NETWORKS.blue().bold(),
                    SELECT_LABEL.into(),
                    SELECT.blue().bold(),
                    " | Search ".into(),
                    "<s>".blue().bold(),
                    " | Clear ".into(),
                    "<c>".blue().bold(),
                ];
                if can_go_back {
                    items.push(" | Go back ".into());
                    items.push("<n/Esc>".blue().bold());
                }
                items.push(QUIT_LABEL.into());
                items.push(QUIT.blue().bold());
                Line::from(items)
            }
        }
        AppMode::Main => match active_tab {
            Some(PrimaryTab::Explore) => {
                if block_popup_open {
                    Line::from(vec![
                        " Enter block number ".into(),
                        " | Confirm ".into(),
                        "<Enter/o>".blue().bold(),
                        " | Latest ".into(),
                        "<l>".blue().bold(),
                        " | Cancel ".into(),
                        "<Esc>".blue().bold(),
                    ])
                } else if info_popup_open {
                    Line::from(vec![
                        " Refresh RPC ".into(),
                        "<r>".blue().bold(),
                        " | Close ".into(),
                        "<i/Esc>".blue().bold(),
                        QUIT_LABEL.into(),
                        QUIT.blue().bold(),
                    ])
                } else if tx_popup_open {
                    let mut items: Vec<ratatui::text::Span> = vec![];
                    if tx_popup_tab == TxPopupTab::Info {
                        items.push(" EVM trace ".into());
                        items.push("<t>".blue().bold());
                        items.push(" |".into());
                    }
                    items.push(" Scroll ".into());
                    items.push("<n/m>".blue().bold());
                    items.push(" | Close ".into());
                    items.push("<Esc/o>".blue().bold());
                    items.push(QUIT_LABEL.into());
                    items.push(QUIT.blue().bold());
                    Line::from(items)
                } else {
                    Line::from(vec![
                        NAV_KEYS.blue().bold(),
                        " | Block ".into(),
                        "<b>".blue().bold(),
                        " | Open ".into(),
                        "<o>".blue().bold(),
                        " | RPC ".into(),
                        "<i>".blue().bold(),
                        " | Networks ".into(),
                        "<n>".blue().bold(),
                        QUIT_LABEL.into(),
                        QUIT.blue().bold(),
                    ])
                }
            }
            Some(PrimaryTab::Search) => Line::from(vec![
                NAV_KEYS.blue().bold(),
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
