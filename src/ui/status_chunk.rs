use super::App;

use tui::{
    backend::Backend,
    layout::{Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

impl App {
    pub(super) fn draw_status_chunk<B: Backend>(&mut self, frame: &mut Frame<B>, chunk: Rect) {
        // Build and render the block.
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .style(Style::default().fg(Color::DarkGray).bg(Color::Black));

        let inner_chunk = block.inner(chunk).inner(&Margin {
            horizontal: 1,
            vertical: 0,
        });

        frame.render_widget(block, chunk);

        // Build the status line.
        let mut status = Vec::with_capacity(2);

        // Scales
        match self.weight() {
            Ok(weight_kg) => {
                let weight_str = if weight_kg >= 0.0 {
                    format!("{:.3} kg", weight_kg).replacen(".", ",", 1)
                } else {
                    String::from("-----")
                };

                status.push(Spans::from(vec![
                    Span::styled(
                        "Waage: ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        weight_str,
                        Style::default().fg(Color::Green).bg(Color::Black),
                    ),
                ]))
            }

            Err(err) => status.push(Spans::from(vec![
                Span::styled(
                    "Waage: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}", err),
                    Style::default().fg(Color::LightRed).bg(Color::Black),
                ),
            ])),
        }

        // Printer
        match self.printer {
            Ok(_) => status.push(Spans::from(vec![
                Span::styled(
                    "Drucker: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "verbunden",
                    Style::default().fg(Color::Green).bg(Color::Black),
                ),
            ])),

            Err(err) => status.push(Spans::from(vec![
                Span::styled(
                    "Drucker: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{}", err),
                    Style::default().fg(Color::LightRed).bg(Color::Black),
                ),
            ])),
        }

        let paragraph = Paragraph::new(status).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner_chunk);
    }
}
