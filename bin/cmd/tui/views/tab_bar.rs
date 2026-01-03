use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::cmd::tui::app::Tab;

pub struct TabBar {
    active_tab: Tab,
}

impl TabBar {
    pub fn new(active_tab: Tab) -> Self {
        Self { active_tab }
    }

    pub fn render(&self, area: Rect, frame: &mut Frame) {
        let tabs = [(Tab::Explore, "1", "Explore"), (Tab::Search, "2", "Search")];

        let mut spans = Vec::new();

        for (i, (tab, num, name)) in tabs.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("  |  ", Style::default().fg(Color::DarkGray)));
            }

            let is_active = *tab == self.active_tab;

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
}
