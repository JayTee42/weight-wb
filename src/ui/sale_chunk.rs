use super::{App, Focus};

use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

impl App {
    pub(super) fn draw_sale_chunk<B: Backend>(&mut self, frame: &mut Frame<B>, chunk: Rect) {
        // Build and render the block.
        let block = Block::default()
            .title("Verkauf")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(
                Style::default()
                    .fg(if self.focus == Focus::Sale {
                        Color::LightBlue
                    } else {
                        Color::DarkGray
                    })
                    .bg(Color::Black),
            );

        let inner_chunk = block.inner(chunk).inner(&Margin {
            horizontal: 1,
            vertical: 0,
        });

        frame.render_widget(block, chunk);

        // If no product has been chosen, we simply show an empty block with some text.
        let Some(product) = self.selected_product() else {
            let empty_paragraph = Paragraph::new("Es ist kein Produkt ausgewählt.")
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: true })
                .alignment(Alignment::Center);

            frame.render_widget(empty_paragraph, inner_chunk);

            return;
        };

        // Split the block into details and actions.
        let vert_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
            .split(inner_chunk);

        let details_chunk = vert_chunks[0];
        let actions_chunk = vert_chunks[1];

        // Build the paragraph for the details.
        let euro_per_kg = (product.ct_per_kg as f64) / 100.0;
        let euro_per_kg_str = format!("{:.2} €", euro_per_kg);
        let mut details = Vec::with_capacity(7);

        details.push(Spans::from(vec![
            Span::styled(
                "Name: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &product.name,
                Style::default().fg(Color::DarkGray).bg(Color::Black),
            ),
        ]));

        details.push(Spans::from(vec![
            Span::styled(
                "Kilopreis: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &euro_per_kg_str,
                Style::default().fg(Color::DarkGray).bg(Color::Black),
            ),
        ]));

        details.push(Spans::from(vec![
            Span::styled(
                "Zutaten: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &product.ingredients,
                Style::default().fg(Color::DarkGray).bg(Color::Black),
            ),
        ]));

        details.push(Spans::from(vec![
            Span::styled(
                "Zusatzinformationen: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &product.additional_info,
                Style::default().fg(Color::DarkGray).bg(Color::Black),
            ),
        ]));

        if let Some(mhd) = product.expiration_date_formatted() {
            details.push(Spans::from(vec![
                Span::styled(
                    "Ungeöffnet mindestens haltbar bis: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(mhd, Style::default().fg(Color::DarkGray).bg(Color::Black)),
            ]));
        }

        details.push(Spans::from(Span::styled(
            "─".repeat(details_chunk.width as _),
            Style::default().fg(Color::DarkGray).bg(Color::Black),
        )));

        if let Ok(weight_kg) = self.weight() {
            let euro = weight_kg * euro_per_kg;
            let euro_str = format!("{:.02} €", euro);

            details.push(Spans::from(vec![
                Span::styled(
                    "Preis: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    euro_str,
                    Style::default().fg(Color::DarkGray).bg(Color::Black),
                ),
            ]));
        }

        let paragraph = Paragraph::new(details).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, details_chunk);

        // Build list items for the actions.
        let item_style = Style::default().fg(Color::DarkGray).bg(Color::Black);

        let items = vec![
            ListItem::new("Verkaufen und Bon drucken").style(item_style),
            ListItem::new("Nur verkaufen").style(item_style),
            ListItem::new("Nur Bon drucken").style(item_style),
        ];

        // Build and render the list.
        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .fg(if self.focus == Focus::Sale {
                        Color::Green
                    } else {
                        Color::White
                    })
                    .bg(Color::Black),
            )
            .highlight_symbol("⇨ ");

        frame.render_stateful_widget(list, actions_chunk, &mut self.action_list_state);
    }
}
