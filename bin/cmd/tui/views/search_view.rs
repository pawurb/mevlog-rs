use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Paragraph},
};
use tui_input::Input;

struct FieldMeta {
    title: &'static str,
    placeholder: &'static str,
}

const FIELD_METADATA: [FieldMeta; 10] = [
    FieldMeta {
        title: "Blocks",
        placeholder: "latest",
    },
    FieldMeta {
        title: "Position",
        placeholder: "e.g. 0:5",
    },
    FieldMeta {
        title: "From",
        placeholder: "Filter by source",
    },
    FieldMeta {
        title: "To",
        placeholder: "Filter by target",
    },
    FieldMeta {
        title: "Event",
        placeholder: "Signature or regexp",
    },
    FieldMeta {
        title: "Not Event",
        placeholder: "Signature or regexp",
    },
    FieldMeta {
        title: "Method",
        placeholder: "Filter by method",
    },
    FieldMeta {
        title: "ERC20 Transfer",
        placeholder: "Contract address or address|amount",
    },
    FieldMeta {
        title: "Tx Cost",
        placeholder: "Gas tx cost",
    },
    FieldMeta {
        title: "Gas Price",
        placeholder: "Gas price",
    },
];

const FIELD_HEIGHT: u16 = 3;
const NUM_FIELDS: usize = 10;

pub struct SearchView<'a> {
    fields: &'a [&'a Input; 10],
    active_field: usize,
    editing: bool,
}

impl<'a> SearchView<'a> {
    pub fn new(fields: &'a [&'a Input; 10], active_field: usize, editing: bool) -> Self {
        Self {
            fields,
            active_field,
            editing,
        }
    }

    pub fn render(&self, area: Rect, frame: &mut Frame) {
        let visible_fields = (area.height / FIELD_HEIGHT) as usize;
        if visible_fields == 0 {
            return;
        }

        let scroll_offset = self.calculate_scroll_offset(visible_fields);

        let fields_to_render = visible_fields.min(NUM_FIELDS - scroll_offset);
        let constraints: Vec<Constraint> = (0..fields_to_render)
            .map(|_| Constraint::Length(FIELD_HEIGHT))
            .chain(std::iter::once(Constraint::Min(0)))
            .collect();

        let chunks = Layout::vertical(constraints).split(area);

        for i in 0..fields_to_render {
            let field_idx = scroll_offset + i;
            let input = self.fields[field_idx];
            let meta = &FIELD_METADATA[field_idx];
            self.render_input(frame, chunks[i], input, meta, field_idx);
        }
    }

    fn calculate_scroll_offset(&self, visible_fields: usize) -> usize {
        if self.active_field < visible_fields {
            0
        } else {
            (self.active_field - visible_fields + 1).min(NUM_FIELDS - visible_fields)
        }
    }

    fn render_input(
        &self,
        frame: &mut Frame,
        area: Rect,
        input: &Input,
        meta: &FieldMeta,
        field_idx: usize,
    ) {
        let is_active = self.active_field == field_idx;
        let is_editing = is_active && self.editing;

        let width = area.width.saturating_sub(3);
        let scroll = input.visual_scroll(width as usize);

        let value = input.value();
        let show_placeholder = value.is_empty() && !is_editing;
        let display_text = if show_placeholder {
            meta.placeholder
        } else {
            value
        };

        let style = if is_editing {
            Style::default().fg(Color::Yellow)
        } else if show_placeholder {
            Style::default().fg(Color::DarkGray)
        } else if is_active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        let block_style = if is_active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        let paragraph = Paragraph::new(display_text)
            .style(style)
            .scroll((0, scroll as u16))
            .block(
                Block::bordered()
                    .title(format!(" {} ", meta.title))
                    .style(block_style),
            );

        frame.render_widget(paragraph, area);

        if is_editing {
            let x = input.visual_cursor().max(scroll) - scroll + 1;
            frame.set_cursor_position((area.x + x as u16, area.y + 1));
        }
    }
}
