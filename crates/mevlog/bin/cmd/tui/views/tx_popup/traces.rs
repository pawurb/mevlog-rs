use mevlog::models::mev_transaction::CallExtract;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

pub fn render_traces_tab(
    area: Rect,
    frame: &mut Frame,
    traces: Option<&[CallExtract]>,
    is_loading: bool,
    scroll: u16,
) {
    if is_loading {
        let paragraph =
            Paragraph::new("Loading traces...").style(Style::default().fg(Color::Yellow));
        frame.render_widget(paragraph, area);
        return;
    }

    let Some(traces) = traces else {
        let paragraph =
            Paragraph::new("Loading traces...").style(Style::default().fg(Color::Yellow));
        frame.render_widget(paragraph, area);
        return;
    };

    if traces.is_empty() {
        let paragraph =
            Paragraph::new("No traces found").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    let lines = build_traces_lines(traces);

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(paragraph, area);
}

fn build_traces_lines(traces: &[CallExtract]) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    lines.push(Line::from(Span::styled(
        format!("Calls ({}):", traces.len()),
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));

    for (index, trace) in traces.iter().enumerate() {
        append_trace_lines(&mut lines, index, trace);
    }

    lines
}

fn append_trace_lines(lines: &mut Vec<Line<'static>>, index: usize, trace: &CallExtract) {
    lines.push(Line::from(Span::styled(
        format!("  [{}]", index),
        Style::default().fg(Color::Yellow),
    )));

    lines.push(Line::from(vec![
        Span::styled("    From: ", Style::default().fg(Color::White)),
        Span::styled(trace.from.to_string(), Style::default().fg(Color::Cyan)),
    ]));

    lines.push(Line::from(vec![
        Span::styled("    To:   ", Style::default().fg(Color::White)),
        Span::styled(trace.to.to_string(), Style::default().fg(Color::Magenta)),
    ]));

    lines.push(Line::from(vec![
        Span::raw("    "),
        Span::styled(
            trace.signature.clone(),
            Style::default().fg(Color::LightGreen),
        ),
    ]));
}
