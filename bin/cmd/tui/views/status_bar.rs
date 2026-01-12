use crate::cmd::tui::data::{ChainEntryJson, TraceMode};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
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
    trace_mode: Option<&'a TraceMode>,
    hide_block: bool,
}

impl<'a> StatusBar<'a> {
    pub fn new(
        chain: Option<&'a ChainEntryJson>,
        current_block: Option<u64>,
        is_loading: bool,
        loading_block: Option<u64>,
        trace_mode: Option<&'a TraceMode>,
    ) -> Self {
        Self {
            chain,
            current_block,
            is_loading,
            loading_block,
            trace_mode,
            hide_block: false,
        }
    }

    pub fn hide_block(mut self) -> Self {
        self.hide_block = true;
        self
    }

    pub fn render(&self, area: Rect, frame: &mut Frame) {
        let mut status_parts = vec![];

        if self.is_loading && self.current_block.is_none() {
            status_parts.push("⋯ ".into());
            status_parts.push("Connecting...".into());
        } else if self.is_loading {
            status_parts.push("⋯ ".yellow());
            status_parts.push("Loading...   ".yellow().bold());
        } else {
            status_parts.push("✓ ".green());
            status_parts.push("Ready        ".green().bold());
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

        if !self.hide_block {
            status_parts.push(" | Block ".into());

            if let Some(loading_block) = self.loading_block {
                status_parts.push(loading_block.to_string().yellow());
            } else if let Some(current_block) = self.current_block {
                status_parts.push(current_block.to_string().into());
            } else {
                status_parts.push("N/A".dark_gray());
            }
        }

        let status_line = Line::from(status_parts);

        let trace_mode_text = match self.trace_mode {
            Some(TraceMode::Revm) => "Trace: Revm",
            Some(TraceMode::RPC) => "Trace: RPC",
            None => "Trace: ...",
        };
        let trace_mode_line = Line::from(trace_mode_text);

        let block = Block::bordered()
            .title(" Status ")
            .border_set(border::PLAIN);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::horizontal([Constraint::Min(0), Constraint::Length(12)]).split(inner);

        let left_paragraph = Paragraph::new(status_line).left_aligned();
        let right_paragraph = Paragraph::new(trace_mode_line).right_aligned();

        frame.render_widget(left_paragraph, chunks[0]);
        frame.render_widget(right_paragraph, chunks[1]);
    }
}
