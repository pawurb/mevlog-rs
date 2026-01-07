use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
};

pub fn render_opcodes_tab(area: Rect, frame: &mut Frame) {
    let paragraph =
        Paragraph::new("WIP: Opcodes view coming soon").style(Style::default().fg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}
