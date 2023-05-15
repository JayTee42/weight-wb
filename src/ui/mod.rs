use crate::{
    db::{Database, ProductEntry, SaleEntry},
    printer::{AttachError, Model, PrintError, Printer},
    voucher::{
        Alignment as VoucherAlignment, Builder as VoucherBuilder, Spacing as VoucherSpacing,
    },
};

use std::{error::Error, io};

use chrono::{DateTime, Duration, Utc};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use image::io::Reader as ImageReader;

use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

#[derive(Copy, Clone, PartialEq, Eq)]
enum Focus {
    Product,
    Sale,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Navigation {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Copy, Clone)]
struct Action {
    pub sale: bool,
    pub print: bool,
}

pub struct App {
    now: DateTime<Utc>,
    db: Database,
    printer: Result<Printer, AttachError>,
    reconnect_printer_date: DateTime<Utc>,
    focus: Focus,
    product_list_state: ListState,
    action_list_state: ListState,
}

impl App {
    fn on_startup(&mut self) {
        // Adjust the product index for the first time.
        self.reset_selected_product_idx();

        // Start with the default (= sell + print) action.
        self.action_list_state.select(Some(0));

        // Try to connect to the printer.
        self.reconnect_printer();
    }

    fn on_tick(&mut self) {
        // Check if we should reconnect the printer.
        if self.reconnect_printer_date <= self.now {
            self.reconnect_printer();
        }
    }

    fn reconnect_printer(&mut self) {
        // Ensure that the printer is dropped first!
        self.printer = Err(AttachError::NoPrinter);

        // Now try to reattach it.
        self.printer = Printer::attach(Some(Model::BrotherQL600));

        if self.printer.is_ok() {
            self.reconnect_printer_date = self.now + Duration::seconds(60);
        } else {
            self.reconnect_printer_date = self.now + Duration::seconds(10);
        }
    }

    fn selected_product_idx(&self) -> Option<usize> {
        self.product_list_state.selected()
    }

    fn selected_product(&self) -> Option<&ProductEntry> {
        self.selected_product_idx()
            .map(|idx| &self.db.products()[idx])
    }

    fn reset_selected_product_idx(&mut self) {
        let idx = if self.db.products().is_empty() {
            None
        } else {
            Some(0)
        };

        self.product_list_state.select(idx);
    }

    fn select_previous_product(&mut self) {
        if let Some(product_idx) = self.selected_product_idx() {
            if product_idx > 0 {
                self.product_list_state.select(Some(product_idx - 1));
            }
        }
    }

    fn select_next_product(&mut self) {
        if let Some(product_idx) = self.selected_product_idx() {
            if product_idx < (self.db.products().len() - 1) {
                self.product_list_state.select(Some(product_idx + 1));
            }
        }
    }

    fn selected_action(&self) -> Action {
        match self.action_list_state.selected().unwrap() {
            0 => Action {
                sale: true,
                print: true,
            },
            1 => Action {
                sale: true,
                print: false,
            },
            2 => Action {
                sale: false,
                print: true,
            },

            _ => unreachable!(),
        }
    }

    fn select_previous_action(&mut self) {
        let idx = self.action_list_state.selected().unwrap();

        if idx > 0 {
            self.action_list_state.select(Some(idx - 1));
        }
    }

    fn select_next_action(&mut self) {
        let idx = self.action_list_state.selected().unwrap();

        if idx < (3 - 1) {
            self.action_list_state.select(Some(idx + 1));
        }
    }

    fn navigate(&mut self, navigation: Navigation) {
        use Navigation::*;

        match self.focus {
            Focus::Product => match navigation {
                Up => self.select_previous_product(),
                Down => self.select_next_product(),
                Right => self.focus = Focus::Sale,
                _ => (),
            },

            Focus::Sale => match navigation {
                Up => self.select_previous_action(),
                Down => self.select_next_action(),
                Left => self.focus = Focus::Product,
                _ => (),
            },
        }
    }

    fn perform_action(&mut self) -> Result<(), Box<dyn Error>> {
        // If we are not in the sale chunk or there is no product, we exit early.
        if self.focus != Focus::Sale {
            return Ok(());
        }

        // Query the selected action.
        let action = self.selected_action();

        // TODO: Obtain the weight.
        let weight_kg = 42.3;

        // Should we print a voucher?
        if action.print {
            if !self.print_voucher(weight_kg, true)? {
                return Ok(());
            }
        }

        // Should we add a sale?
        if action.sale {
            if !self.perform_sale(weight_kg)? {
                return Ok(());
            }
        }

        // TODO: Show success message

        Ok(())
    }

    fn print_voucher(
        &mut self,
        weight_kg: f64,
        should_retry: bool,
    ) -> Result<bool, Box<dyn Error>> {
        // There must be a product to continue.
        let Some(product) = self.selected_product() else {
            // No error message here, the error is obvious.
            return Ok(false);
        };

        // If there is no printer, try to reconnect it once.
        let Ok(printer) = &self.printer else {
            if should_retry {
                self.reconnect_printer();
                return self.print_voucher( weight_kg, false);
            }

            // TODO: Show error message.
            return Ok(false);
        };

        // Calculate the price.
        let euro_per_kg = (product.ct_per_kg as f64) / 100.0;
        let euro = weight_kg * euro_per_kg;

        // Build the voucher.
        let logo = ImageReader::open("logo.png")
            .expect("Failed to load logo")
            .decode()
            .expect("Failed to decode logo");

        let mhd = product.expiration_date_formatted();
        let info = self.db.info();

        let trailer = format!(
            "{} · {} · {}, {}, · {} · {}",
            info.business, info.owners, info.street, info.locality, info.phone, info.mail
        );

        let voucher = VoucherBuilder::new(696, None)
            // Logo
            .start_image_component(&logo)
            .spacing(VoucherSpacing::horz_vert(20.0, 20.0))
            .finalize_image_component()
            // Product
            .start_text_component(&product.name)
            .spacing(VoucherSpacing::horz_vert(16.0, 16.0))
            .font_size(50.0)
            .alignment(VoucherAlignment::Center)
            .bold(true)
            .finalize_text_component()
            // Mass
            .start_text_component(&format!("Gewicht: {:.02} kg", weight_kg))
            .spacing(VoucherSpacing::horz_vert(16.0, 12.0))
            .font_size(25.0)
            .finalize_text_component()
            // Price
            .start_text_component(&format!("Preis: {:.02} €", euro))
            .spacing(VoucherSpacing::horz_vert(16.0, 24.0))
            .font_size(40.0)
            .bold(true)
            .finalize_text_component()
            // Ingredients
            .start_text_component(&format!("Zutaten: {}", product.ingredients))
            .spacing(VoucherSpacing::horz_vert(16.0, 12.0))
            .font_size(25.0)
            .finalize_text_component()
            // Additionals
            .start_text_component(&product.additional_info)
            .spacing(VoucherSpacing::horz_vert(16.0, 12.0))
            .font_size(25.0)
            .finalize_text_component()
            // Mhd
            .start_text_component(&format!(
                "Ungeöffnet mindestens haltbar bis: {}",
                mhd.as_deref().unwrap_or("-")
            ))
            .spacing(VoucherSpacing::horz_vert(16.0, 12.0))
            .font_size(25.0)
            .finalize_text_component()
            // Trailer
            .start_text_component(&trailer)
            .spacing(VoucherSpacing::lrtb(8.0, 8.0, 48.0, 8.0))
            .font_size(21.0)
            .alignment(VoucherAlignment::Center)
            .italic(true)
            .finalize_text_component()
            .build();

        // Try to print it.
        if let Err(err) = printer.print(&voucher) {
            // Try a reconnect on USB errors once.
            if matches!(err, PrintError::USBError(_)) {
                self.reconnect_printer();
                return self.print_voucher(weight_kg, false);
            }

            // TODO: Show error message
            return Ok(false);
        }

        Ok(true)
    }

    fn perform_sale(&self, weight_kg: f64) -> Result<bool, Box<dyn Error>> {
        // There must be a product to continue.
        let Some(product) = self.selected_product() else {
            // No error message here, the error is obvious.
            return Ok(false);
        };

        let sale = SaleEntry::new(self.now, product.name.clone(), weight_kg, product.ct_per_kg);
        self.db.add_sale(sale)?;

        Ok(true)
    }

    fn run_in_terminal<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<(), Box<dyn Error>> {
        // Perform the startup logic.
        self.on_startup();

        // Track the time to provide the application with a tick.
        let tick_rate = Duration::milliseconds(250);
        let mut last_tick = self.now;

        loop {
            // Set the current timestamp.
            self.now = Utc::now();
            let time_since_last_tick = self.now - last_tick;

            // Draw the UI.
            terminal.draw(|frame| self.draw_ui(frame))?;

            // Poll the terminal for events.
            // Make sure that we don't miss the next tick.
            let timeout = (tick_rate - time_since_last_tick).max(Duration::zero());

            if event::poll(timeout.to_std().unwrap())? {
                // Handle key events.
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('r') => {
                            self.db.reload_products()?;
                            self.reset_selected_product_idx();
                        }
                        KeyCode::Up => self.navigate(Navigation::Up),
                        KeyCode::Down => self.navigate(Navigation::Down),
                        KeyCode::Left => self.navigate(Navigation::Left),
                        KeyCode::Right => self.navigate(Navigation::Right),
                        KeyCode::Enter => self.perform_action()?,

                        _ => {}
                    }
                }
            }

            if time_since_last_tick >= tick_rate {
                self.on_tick();
                last_tick = self.now;
            }
        }
    }

    fn draw_ui<B: Backend>(&mut self, frame: &mut Frame<B>) {
        // Split the window into body and status line.
        let vert_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
            .split(frame.size());

        let body_chunk = vert_chunks[0];
        let status_line_chunk = vert_chunks[1];

        // Split the body into product and selection.
        let horz_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
            .split(body_chunk);

        let product_chunk = horz_chunks[0];
        let sale_chunk = horz_chunks[1];

        // Draw the chunks.
        self.draw_product_chunk(frame, product_chunk);
        self.draw_sale_chunk(frame, sale_chunk);
        self.draw_status_line_chunk(frame, status_line_chunk);
    }

    fn draw_product_chunk<B: Backend>(&mut self, frame: &mut Frame<B>, chunk: Rect) {
        // Build and render the block.
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Produkte")
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

    fn draw_sale_chunk<B: Backend>(&mut self, frame: &mut Frame<B>, chunk: Rect) {
        // Build and render the block.
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Verkauf")
            .style(
                Style::default()
                    .fg(if self.focus == Focus::Sale {
                        Color::LightBlue
                    } else {
                        Color::Gray
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
        let euro_per_kg = format!("{:.2} €", (product.ct_per_kg as f64) / 100.0);
        let mut details = Vec::with_capacity(5);

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
                &euro_per_kg,
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

    fn draw_status_line_chunk<B: Backend>(&mut self, frame: &mut Frame<B>, chunk: Rect) {
        // Build and render the block.
        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::DarkGray).bg(Color::Black));

        let inner_chunk = block.inner(chunk).inner(&Margin {
            horizontal: 1,
            vertical: 0,
        });

        frame.render_widget(block, chunk);

        // Build the status line.
        let mut status = Vec::with_capacity(2);

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
                    format!(
                        "{} · Nächster Versuch in {} Sekunde(n)",
                        err,
                        (self.reconnect_printer_date - self.now)
                            .max(Duration::zero())
                            .num_seconds()
                            + 1
                    ),
                    Style::default().fg(Color::LightRed).bg(Color::Black),
                ),
            ])),
        };

        let paragraph = Paragraph::new(status).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, inner_chunk);
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        // Instantiate the app.
        let now = Utc::now();

        let mut app = App {
            now,
            db: Database::open_or_create("db.sqlite")?,
            printer: Err(AttachError::NoPrinter),
            reconnect_printer_date: now,
            focus: Focus::Product,
            product_list_state: Default::default(),
            action_list_state: Default::default(),
        };

        // Configure the terminal.
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        // Instantiate TUI.
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Run the app.
        app.run_in_terminal(&mut terminal)?;

        // Restore the terminal.
        disable_raw_mode()?;

        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;

        terminal.show_cursor()?;

        Ok(())
    }
}
