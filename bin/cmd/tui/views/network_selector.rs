use mevlog::ChainEntryJson;
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Cell, Clear, HighlightSpacing, Paragraph, Row, Table, TableState},
};

const HEADER_STYLE: Style = Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);
const SELECTED_ROW_STYLE: Style = Style::new()
    .bg(Color::DarkGray)
    .add_modifier(Modifier::BOLD);

pub struct NetworkSelector<'a> {
    chains: &'a [ChainEntryJson],
    search_query: &'a str,
    is_loading: bool,
}

impl<'a> NetworkSelector<'a> {
    pub fn new(chains: &'a [ChainEntryJson], search_query: &'a str, is_loading: bool) -> Self {
        Self {
            chains,
            search_query,
            is_loading,
        }
    }

    pub fn render(&self, area: Rect, frame: &mut Frame, state: &mut TableState, popup_open: bool) {
        self.render_chains_table(area, frame, state);

        if popup_open {
            self.render_search_popup(frame);
        }
    }

    fn render_chains_table(&self, area: Rect, frame: &mut Frame, state: &mut TableState) {
        let header = Row::new(vec![
            Cell::from("Chain ID"),
            Cell::from("Name"),
            Cell::from("Symbol"),
        ])
        .style(HEADER_STYLE)
        .height(1);

        let rows: Vec<Row> = self
            .chains
            .iter()
            .map(|chain| {
                Row::new(vec![
                    Cell::from(chain.chain_id.to_string()),
                    Cell::from(chain.name.clone()),
                    Cell::from(chain.chain.clone()),
                ])
            })
            .collect();

        let widths = [
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ];

        let title = if self.is_loading {
            " Select Network (Loading...) "
        } else if self.chains.is_empty() {
            " Select Network (No matches) "
        } else {
            " Select Network "
        };

        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::bordered().title(title).border_set(border::THICK))
            .column_spacing(2)
            .row_highlight_style(SELECTED_ROW_STYLE)
            .highlight_symbol(">> ")
            .highlight_spacing(HighlightSpacing::Always);

        frame.render_stateful_widget(table, area, state);
    }

    fn render_search_popup(&self, frame: &mut Frame) {
        let popup_width = 60.min(frame.area().width - 4);
        let popup_height = 3;
        let popup_area = centered_rect(popup_width, popup_height, frame.area());

        let input_text = if self.search_query.is_empty() {
            Line::from(vec![
                Span::styled("Search: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "(type to filter, Enter/Esc to close)",
                    Style::default().fg(Color::DarkGray),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled("Search: ", Style::default().fg(Color::Yellow)),
                Span::raw(self.search_query),
                Span::styled("_", Style::default().fg(Color::Yellow)),
            ])
        };

        let popup = Paragraph::new(input_text).block(
            Block::bordered()
                .title(" Search Networks ")
                .style(Style::default().bg(Color::DarkGray))
                .border_set(border::THICK),
        );

        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup, popup_area);
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let horizontal = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center);
    let vertical = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
