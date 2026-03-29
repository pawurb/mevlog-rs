use mevlog::{misc::opcode_color::OpcodeColor, models::json::mev_opcode_json::MEVOpcodeJson};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use revm::bytecode::OpCode;

pub fn render_opcodes_tab(
    area: Rect,
    frame: &mut Frame,
    opcodes: Option<&[MEVOpcodeJson]>,
    is_loading: bool,
    scroll: u16,
) {
    if is_loading {
        let paragraph =
            Paragraph::new("Loading opcodes...").style(Style::default().fg(Color::Yellow));
        frame.render_widget(paragraph, area);
        return;
    }

    let Some(opcodes) = opcodes else {
        let paragraph =
            Paragraph::new("Loading opcodes...").style(Style::default().fg(Color::Yellow));
        frame.render_widget(paragraph, area);
        return;
    };

    if opcodes.is_empty() {
        let paragraph =
            Paragraph::new("No opcodes found").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    let header_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);

    let header = Line::from(vec![
        Span::styled(format!("{:<8}  ", "PC"), header_style),
        Span::styled(format!("{:<16}  ", "OP"), header_style),
        Span::styled(format!("{:<8}  ", "COST"), header_style),
        Span::styled("GAS_LEFT", header_style),
    ]);
    frame.render_widget(Paragraph::new(header), chunks[0]);

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(opcodes.len());

    for opcode in opcodes {
        let op_color = OpCode::parse(&opcode.op)
            .map(|op| op.color())
            .unwrap_or(Color::White);

        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<8}  ", opcode.pc),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("{:<16}  ", opcode.op),
                Style::default().fg(op_color),
            ),
            Span::styled(
                format!("{:<8}  ", opcode.cost),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                format!("{}", opcode.gas_left),
                Style::default().fg(Color::Green),
            ),
        ]));
    }

    let paragraph = Paragraph::new(lines).scroll((scroll, 0));
    frame.render_widget(paragraph, chunks[1]);
}
