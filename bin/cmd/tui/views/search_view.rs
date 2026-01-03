use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Paragraph},
};

pub struct SearchView;

impl SearchView {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&self, area: Rect, frame: &mut Frame) {
        let text = "WIP - Search functionality coming soon";

        let paragraph = Paragraph::new(text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::bordered().title(" Search "))
            .centered();

        frame.render_widget(paragraph, area);
    }
}
