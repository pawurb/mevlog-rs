use crate::cmd::tui::data::ChainEntryJson;
use ratatui::{
    Frame,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{Block, Paragraph},
};

pub struct StatusBar<'a> {
    chain: Option<&'a ChainEntryJson>,
    current_block: Option<u64>,
    is_loading: bool,
    loading_block: Option<u64>,
}

impl<'a> StatusBar<'a> {
    pub fn new(
        chain: Option<&'a ChainEntryJson>,
        current_block: Option<u64>,
        is_loading: bool,
        loading_block: Option<u64>,
    ) -> Self {
        Self {
            chain,
            current_block,
            is_loading,
            loading_block,
        }
    }

    pub fn render(&self, area: Rect, frame: &mut Frame) {
        let mut status_parts = vec![];

        if self.is_loading && self.current_block.is_none() {
            status_parts.push("⋯ ".into());
            status_parts.push("Connecting...".into());
        } else if self.is_loading {
            status_parts.push("⋯ ".yellow());
            status_parts.push("Loading...".yellow().bold());
        } else {
            status_parts.push("✓ ".green());
            status_parts.push("Ready".green().bold());
        }

        status_parts.push(" | Network ".into());

        if let Some(chain) = self.chain {
            status_parts.push(chain.name.clone().into());
            status_parts.push(" (".into());
            status_parts.push(chain.chain_id.to_string().yellow());
            status_parts.push(")".into());
        } else {
            status_parts.push("Unknown".dark_gray());
        }

        status_parts.push(" | Block ".into());

        if let Some(loading_block) = self.loading_block {
            status_parts.push(loading_block.to_string().yellow());
        } else if let Some(current_block) = self.current_block {
            status_parts.push(current_block.to_string().into());
        } else {
            status_parts.push("N/A".dark_gray());
        }

        let status_line = Line::from(status_parts);

        let block = Block::bordered()
            .title(" Status ")
            .border_set(border::PLAIN);

        let paragraph = Paragraph::new(status_line).block(block).left_aligned();

        frame.render_widget(paragraph, area);
    }
}
