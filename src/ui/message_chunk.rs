use super::App;

use tui::{
    backend::Backend,
    layout::{Alignment, Margin, Rect},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

#[derive(Copy, Clone, PartialEq, Eq)]
pub(super) enum MessageType {
    Info,
    Error,
}

impl App {
    pub(super) fn draw_message_chunk<B: Backend>(
        &mut self,
        frame: &mut Frame<B>,
        chunk: Rect,
        ty: MessageType,
        text: &str,
    ) {
        let (title, fg_color) = match ty {
            MessageType::Info => ("Information", Color::Green),
            MessageType::Error => ("Fehler", Color::LightRed),
        };

        // Build and render the block.
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().fg(fg_color).bg(Color::Black));

        let inner_chunk = block.inner(chunk).inner(&Margin {
            horizontal: 1,
            vertical: 0,
        });

        frame.render_widget(block, chunk);

        // Build the paragraph for the message.
        let paragraph = Paragraph::new(Spans::from(Span::styled(
            text,
            Style::default().fg(fg_color).bg(Color::Black),
        )))
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center);

        frame.render_widget(paragraph, inner_chunk);
    }
}
