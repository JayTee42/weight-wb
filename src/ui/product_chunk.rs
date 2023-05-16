use super::{App, Focus};

use tui::{
    backend::Backend,
    layout::{Alignment, Margin, Rect},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

impl App {
    pub(super) fn draw_product_chunk<B: Backend>(&mut self, frame: &mut Frame<B>, chunk: Rect) {
        // Build and render the block.
        let block = Block::default()
            .title("Produkte")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(
                Style::default()
                    .fg(if self.focus == Focus::Product {
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

        // If no product is available, we simply show an empty block with some text.
        if self.selected_product().is_none() {
            let empty_paragraph = Paragraph::new("Die Datenbank enthält keine Produkte.")
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: true })
                .alignment(Alignment::Center);

            frame.render_widget(empty_paragraph, inner_chunk);

            return;
        };

        // Build list items for the products.
        let items: Vec<_> = self
            .db
            .products()
            .iter()
            .map(|product| {
                ListItem::new(product.name.as_str())
                    .style(Style::default().fg(Color::DarkGray).bg(Color::Black))
            })
            .collect();

        // Build and render the product list.
        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .fg(if self.focus == Focus::Product {
                        Color::Green
                    } else {
                        Color::White
                    })
                    .bg(Color::Black),
            )
            .highlight_symbol("⇨ ");

        frame.render_stateful_widget(list, inner_chunk, &mut self.product_list_state);
    }
}
