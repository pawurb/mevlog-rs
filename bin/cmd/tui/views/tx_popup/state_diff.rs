use mevlog::models::json::mev_state_diff_json::MEVStateDiffJson;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub fn render_state_diff_tab(
    area: Rect,
    frame: &mut Frame,
    state_diff: Option<&MEVStateDiffJson>,
    is_loading: bool,
    scroll: u16,
) {
    if is_loading {
        let paragraph =
            Paragraph::new("Loading state diff...").style(Style::default().fg(Color::Yellow));
        frame.render_widget(paragraph, area);
        return;
    }

    let Some(state_diff) = state_diff else {
        let paragraph =
            Paragraph::new("Loading state diff...").style(Style::default().fg(Color::Yellow));
        frame.render_widget(paragraph, area);
        return;
    };

    if state_diff.0.is_empty() {
        let paragraph =
            Paragraph::new("No storage changes").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    let mut lines: Vec<Line<'static>> = Vec::new();

    for (address, slots) in &state_diff.0 {
        lines.push(Line::from(vec![Span::styled(
            format!("{address}"),
            Style::default().fg(Color::Green),
        )]));

        for (slot, [before, after]) in slots {
            let before_str = before
                .map(|v| format!("{v}"))
                .unwrap_or_else(|| "null".to_string());
            let after_str = after
                .map(|v| format!("{v}"))
                .unwrap_or_else(|| "null".to_string());

            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{slot}"), Style::default().fg(Color::Yellow)),
            ]));
            lines.push(Line::from(vec![
                Span::raw("    Before: "),
                Span::styled(before_str, Style::default().fg(Color::Red)),
            ]));
            lines.push(Line::from(vec![
                Span::raw("    After:  "),
                Span::styled(after_str, Style::default().fg(Color::Cyan)),
            ]));
        }
        lines.push(Line::raw(""));
    }

    let paragraph = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(paragraph, area);
}
