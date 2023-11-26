use super::{Action, App};
use crate::db::ProductEntry;

use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum DialogAction {
    Confirm,
    Cancel,
}

impl App {
    pub(super) fn draw_dialog_chunk<B: Backend>(
        &mut self,
        frame: &mut Frame<B>,
        chunk: Rect,
        action: Action,
        product: &ProductEntry,
        weight_kg: Option<f64>,
    ) {
        // Build and render the block.
        let block = Block::default()
            .title("Aktion bestätigen")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().fg(Color::LightBlue).bg(Color::Black));

        let inner_chunk = block.inner(chunk).inner(&Margin {
            horizontal: 1,
            vertical: 0,
        });

        frame.render_widget(block, chunk);

        // Split the block into message and actions.
        let vert_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(2)].as_ref())
            .split(inner_chunk);

        let message_chunk = vert_chunks[0];
        let actions_chunk = vert_chunks[1];

        // Build the paragraph for the message.
        let sale_str = if product.is_kg_price {
            let weight_kg = weight_kg.expect("Product with kg price needs weight");
            let weight_str = format!("{:.3} kg", weight_kg).replacen('.', ",", 1);
            let euro_per_kg = (product.price_ct as f64) / 100.0;
            let euro = weight_kg * euro_per_kg;
            let euro_str = format!("{:.2} €", euro).replacen('.', ",", 1);

            format!("{} {} für {}", weight_str, product.name, euro_str)
        } else {
            let euro = (product.price_ct as f64) / 100.0;
            let euro_str = format!("{:.2} €", euro).replacen('.', ",", 1);

            format!("{} für {}", product.name, euro_str)
        };

        let paragraph = Paragraph::new(vec![
            Spans::from(Span::styled(
                sale_str,
                Style::default().fg(Color::Gray).bg(Color::Black),
            )),
            Spans::from(Span::styled(
                format!("Verkaufen: {}", if action.sale { "ja" } else { "nein" }),
                Style::default().fg(Color::Gray).bg(Color::Black),
            )),
            Spans::from(Span::styled(
                format!("Bon drucken: {}", if action.print { "ja" } else { "nein" }),
                Style::default().fg(Color::Gray).bg(Color::Black),
            )),
        ])
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center);

        frame.render_widget(paragraph, message_chunk);

        // Build list items for the actions.
        let item_style = Style::default().fg(Color::DarkGray).bg(Color::Black);

        let items = vec![
            ListItem::new("Ok").style(item_style),
            ListItem::new("Abbrechen").style(item_style),
        ];

        // Build and render the list.
        let list = List::new(items)
            .highlight_style(Style::default().fg(Color::Green).bg(Color::Black))
            .highlight_symbol("⇨ ");

        frame.render_stateful_widget(list, actions_chunk, &mut self.dialog_list_state);
    }
}
