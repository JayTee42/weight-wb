use crate::{
    db::{Database, ProductEntry, SaleEntry},
    printer::{AttachError, Model as PrinterModel, PrintError, Printer},
    voucher::{
        Alignment as VoucherAlignment, Builder as VoucherBuilder, Spacing as VoucherSpacing,
    },
    weight::{Scales, WeightResult},
};

use std::error::Error;
use std::io;
use std::thread;

use chrono::{DateTime, Duration, Utc};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use image::io::Reader as ImageReader;

use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    widgets::{Clear, ListState},
    Frame, Terminal,
};

mod dialog_chunk;
use dialog_chunk::DialogAction;

mod message_chunk;
use message_chunk::MessageType;

mod product_chunk;

mod sale_chunk;

mod status_chunk;

#[derive(Copy, Clone, PartialEq, Eq)]
enum Focus {
    Product,
    Sale,
    Dialog,
    Message,
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

#[derive(Clone)]
enum Popup {
    Dialog {
        action: Action,
        product: ProductEntry,
        weight_kg: f64,
    },

    Message {
        ty: MessageType,
        text: String,
    },
}

pub struct App {
    now: DateTime<Utc>,
    db: Database,
    scales: Scales,
    printer: Result<Printer, AttachError>,
    reconnect_printer_date: DateTime<Utc>,
    focus: Focus,
    popup: Option<Popup>,
    product_list_state: ListState,
    action_list_state: ListState,
    dialog_list_state: ListState,
}

impl App {
    fn on_startup(&mut self) -> Result<(), Box<dyn Error>> {
        // Adjust the product index for the first time.
        self.reset_selected_product_idx();

        // Start with the default (= sell + print) action.
        self.action_list_state.select(Some(0));

        // Try to connect to the printer.
        self.reconnect_printer()?;

        Ok(())
    }

    fn on_tick(&mut self) -> Result<(), Box<dyn Error>> {
        // Check if we should reconnect the printer.
        if self.reconnect_printer_date <= self.now {
            self.reconnect_printer()?;
        }

        Ok(())
    }

    fn weight(&self) -> WeightResult {
        self.scales.weight()
    }

    fn reconnect_printer(&mut self) -> Result<(), Box<dyn Error>> {
        // Ensure that the old printer is dropped first!
        self.printer = Err(AttachError::NoPrinter);

        // Now try to reattach it.
        let model_filter = self
            .db
            .info()
            .printer_model
            .as_deref()
            .map(PrinterModel::try_from)
            .transpose()?;

        self.printer = Printer::attach(model_filter);

        if self.printer.is_ok() {
            self.reconnect_printer_date = self.now + Duration::seconds(120);
        } else {
            self.reconnect_printer_date = self.now + Duration::seconds(10);
        }

        Ok(())
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

    fn selected_dialog_action(&mut self) -> DialogAction {
        match self.dialog_list_state.selected().unwrap() {
            0 => DialogAction::Confirm,
            1 => DialogAction::Cancel,

            _ => unreachable!(),
        }
    }

    fn select_previous_dialog_action(&mut self) {
        let idx = self.dialog_list_state.selected().unwrap();

        if idx > 0 {
            self.dialog_list_state.select(Some(idx - 1));
        }
    }

    fn select_next_dialog_action(&mut self) {
        let idx = self.dialog_list_state.selected().unwrap();

        if idx < (2 - 1) {
            self.dialog_list_state.select(Some(idx + 1));
        }
    }

    fn show_message(&mut self, ty: MessageType, text: String) {
        self.popup = Some(Popup::Message { ty, text });
        self.focus = Focus::Message;
    }

    fn show_dialog(&mut self, action: Action, product: ProductEntry, weight_kg: f64) {
        self.popup = Some(Popup::Dialog {
            action,
            product,
            weight_kg,
        });

        self.focus = Focus::Dialog;

        // The dialog always starts with a preselection of "Ok".
        self.dialog_list_state.select(Some(0));
    }

    fn navigate(&mut self, navigation: Navigation) {
        use Navigation::*;

        match (self.focus, navigation) {
            (Focus::Product, Up) => self.select_previous_product(),
            (Focus::Product, Down) => self.select_next_product(),
            (Focus::Product, Right) => self.focus = Focus::Sale,
            (Focus::Sale, Up) => self.select_previous_action(),
            (Focus::Sale, Down) => self.select_next_action(),
            (Focus::Sale, Left) => self.focus = Focus::Product,
            (Focus::Dialog, Up) => self.select_previous_dialog_action(),
            (Focus::Dialog, Down) => self.select_next_dialog_action(),
            _ => (),
        }
    }

    fn perform_action(&mut self) -> Result<(), Box<dyn Error>> {
        match self.focus {
            Focus::Sale => {
                // If there is no product or weight, we exit early.
                // Because we must cache the product in the confirmation dialog, it must be cloned.
                let Some(product) = self.selected_product().map(Clone::clone) else {
                    return Ok(());
                };

                let weight_kg = match self.weight() {
                    Ok(weight) => weight,

                    Err(err) => {
                        // Show an error message.
                        self.show_message(
                            MessageType::Error,
                            format!("Fehler beim Zugriff auf die Waage: {}", err),
                        );

                        return Ok(());
                    }
                };

                // Show a confirmation dialog.
                self.show_dialog(self.selected_action(), product, weight_kg);

                Ok(())
            }

            Focus::Dialog => {
                let Some(Popup::Dialog { action, product, weight_kg }) = self.popup.take() else {
                    panic!("Dialog is focused, but not present.");
                };

                match self.selected_dialog_action() {
                    DialogAction::Confirm => {
                        // Should we print a voucher?
                        if action.print && !self.print_voucher(&product, weight_kg, true)? {
                            return Ok(());
                        }

                        // Should we add a sale?
                        if action.sale && !self.perform_sale(&product, weight_kg)? {
                            return Ok(());
                        }

                        // Show a success message.
                        self.show_message(
                            MessageType::Info,
                            String::from("Vorgang erfolgreich abgeschlossen"),
                        );
                    }

                    DialogAction::Cancel => {
                        // Back to the sale chunk.
                        self.focus = Focus::Sale;
                    }
                }

                Ok(())
            }

            Focus::Message => {
                // Back to the sale chunk.
                self.popup = None;
                self.focus = Focus::Sale;

                Ok(())
            }

            _ => Ok(()),
        }
    }

    fn print_voucher(
        &mut self,
        product: &ProductEntry,
        weight_kg: f64,
        should_retry: bool,
    ) -> Result<bool, Box<dyn Error>> {
        // Check if a printer is present.
        let printer = match &self.printer {
            Ok(printer) => printer,

            Err(err) => {
                // If there is no printer, try to reconnect it once.
                if should_retry {
                    self.reconnect_printer()?;
                    return self.print_voucher(product, weight_kg, false);
                }

                // Show an error message.
                self.show_message(
                    MessageType::Error,
                    format!("Fehler beim Zugriff auf den Drucker: {}", err),
                );

                return Ok(false);
            }
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
            // Weight
            .start_text_component(&format!("Gewicht: {:.3} kg", weight_kg).replacen(".", ",", 1))
            .spacing(VoucherSpacing::horz_vert(16.0, 12.0))
            .font_size(25.0)
            .finalize_text_component()
            // Price
            .start_text_component(&format!("Preis: {:.2} €", euro).replacen(".", ",", 1))
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
            // Try a reconnect once on USB errors.
            if matches!(err, PrintError::USBError(_)) {
                self.reconnect_printer()?;
                return self.print_voucher(product, weight_kg, false);
            }

            // Show an error message.
            self.show_message(MessageType::Error, format!("Fehler beim Drucken: {}", err));

            return Ok(false);
        }

        // Sleep for a moment until we are done printing.
        thread::sleep(Duration::seconds(2).to_std().unwrap());

        Ok(true)
    }

    fn perform_sale(&self, product: &ProductEntry, weight_kg: f64) -> Result<bool, Box<dyn Error>> {
        let sale = SaleEntry::new(self.now, product.name.clone(), weight_kg, product.ct_per_kg);
        self.db.add_sale(&sale)?;

        Ok(true)
    }

    fn run_in_terminal<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<(), Box<dyn Error>> {
        // Perform the startup logic.
        self.on_startup()?;

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
                            self.db.reload_info()?;
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
                self.on_tick()?;
                last_tick = self.now;
            }
        }
    }

    fn draw_ui<B: Backend>(&mut self, frame: &mut Frame<B>) {
        // Split the window into body and status line.
        let vert_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(4)].as_ref())
            .split(frame.size());

        let body_chunk = vert_chunks[0];
        let status_chunk = vert_chunks[1];

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
        self.draw_status_chunk(frame, status_chunk);

        // Is there a popup?
        // Borrow checker shenanigans ...
        let popup = self.popup.take();

        if let Some(popup) = &popup {
            // Crop a centered rectangle to render the popup into.
            let (percent_x, percent_y, min_y) = match popup {
                Popup::Dialog { .. } => (70, 15, 7),
                Popup::Message { .. } => (70, 10, 3),
            };

            let popup_chunk = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Percentage((100 - percent_y) / 2),
                        Constraint::Min(min_y),
                        Constraint::Percentage((100 - percent_y) / 2),
                    ]
                    .as_ref(),
                )
                .split(frame.size());

            let popup_chunk = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    [
                        Constraint::Percentage((100 - percent_x) / 2),
                        Constraint::Percentage(percent_x),
                        Constraint::Percentage((100 - percent_x) / 2),
                    ]
                    .as_ref(),
                )
                .split(popup_chunk[1])[1];

            // Clear the background.
            frame.render_widget(Clear, popup_chunk);

            // Render the popup.
            match popup {
                Popup::Dialog {
                    action,
                    product,
                    weight_kg,
                } => self.draw_dialog_chunk(frame, popup_chunk, *action, product, *weight_kg),

                Popup::Message { ty, text } => {
                    self.draw_message_chunk(frame, popup_chunk, *ty, text)
                }
            }
        }

        self.popup = popup;
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        // Instantiate the app.
        let now = Utc::now();
        let db = Database::open_or_create("db.sqlite")?;
        let scales = Scales::on_serial_port(&db.info().serial_port);

        let mut app = App {
            now,
            db,
            scales,
            printer: Err(AttachError::NoPrinter),
            reconnect_printer_date: now,
            focus: Focus::Product,
            popup: None,
            product_list_state: Default::default(),
            action_list_state: Default::default(),
            dialog_list_state: Default::default(),
        };

        // Configure the terminal.
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        // Instantiate TUI.
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Run the app.
        let result = app.run_in_terminal(&mut terminal);

        // Restore the terminal.
        disable_raw_mode()?;

        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;

        terminal.show_cursor()?;

        result
    }
}
