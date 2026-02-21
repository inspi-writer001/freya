use crate::CompressMessage;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
};
use std::{io, sync::mpsc};

#[derive(Debug)]
pub struct App {
    exit: bool,
    pub is_compressing: bool,
    pub progress: f64, // A percentage from 0.0 to 1.0
    pub status_message: String,
    pub receiver: Option<mpsc::Receiver<CompressMessage>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            exit: false,
            is_compressing: false,
            progress: 0.0,
            status_message: " Start by pressing 'o' to open a file".to_string(),
            receiver: None,
        }
    }
}

impl App {
    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        // Use poll with a timeout so the loop can also check compression progress
        if event::poll(std::time::Duration::from_millis(50))? {
            match event::read()? {
                // it's important to check that the event is a key press event as
                // crossterm also emits key release and repeat events on Windows.
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event)
                }
                _ => {}
            };
        }
        self.check_compression_progress();
        Ok(())
    }

    fn check_compression_progress(&mut self) {
        if let Some(ref receiver) = self.receiver {
            while let Ok(msg) = receiver.try_recv() {
                match msg {
                    CompressMessage::Progress {
                        bytes_processed,
                        total_bytes,
                    } => {
                        if total_bytes > 0 {
                            self.progress = bytes_processed as f64 / total_bytes as f64;
                        }
                    }
                    CompressMessage::Finished => {
                        self.is_compressing = false;
                        self.progress = 1.0;
                        self.status_message = " Compression complete!".to_string();
                        self.receiver = None;
                        return;
                    }
                    CompressMessage::Error(e) => {
                        self.is_compressing = false;
                        self.progress = 0.0;
                        self.status_message = format!(" Error: {}", e);
                        self.receiver = None;
                        return;
                    }
                }
            }
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Char('o') => {
                // 1. Open the native OS file dialogue
                if let Some(input_path) = rfd::FileDialog::new().pick_file() {
                    // 2. The user picked a file, so read it
                    // 2. Automatically create the output path (e.g., "document.pdf" -> "document.pdf.zst")
                    let mut output_path = input_path.clone();
                    let mut new_extension =
                        output_path.extension().unwrap_or_default().to_os_string();
                    new_extension.push(".zst");
                    output_path.set_extension(new_extension);

                    // 3. Set up the communication channel for the background thread
                    let (tx, rx) = std::sync::mpsc::channel();
                    self.receiver = Some(rx);
                    self.is_compressing = true;
                    self.progress = 0.0;

                    // Let the user know we're starting
                    self.status_message = format!(
                        " Compressing {:?}",
                        input_path.file_name().unwrap_or_default()
                    );

                    crate::start_compression(
                        input_path.to_string_lossy().to_string(),
                        output_path.to_string_lossy().to_string(),
                        tx,
                    );
                } else {
                    // TODO handle error gracefully
                    self.status_message = format!(
                        "Not Compressing ",
                        // input_path.file_name().unwrap_or_default()
                    );
                }
            }
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::vertical([
            Constraint::Length(5), // Height for the description block (borders + text + padding)
            Constraint::Length(3), // Height for the instruction block
            Constraint::Min(0),    // The remaining empty space on the screen
        ])
        .split(area);
        let title = Line::from(" Freya - Lossless Compression for files ".bold());
        let instructions = Line::from(vec![
            " Open File ".into(),
            "<o> |".blue().bold(),
            " Quit ".into(),
            "<Q> ".blue().bold(),
        ]);
        let block1 = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_style(Style::new().blue())
            .border_set(border::DOUBLE);

        let description_text = Text::from(vec![Line::from(vec![
            " Freya helps compress your file types without losing the quality of the files.".into(),
        ])]);

        let instruction_text = Text::from(vec![Line::from(vec![
            self.status_message.to_string().yellow(),
        ])]);

        Paragraph::new(description_text)
            .left_aligned()
            .block(block1)
            .render(chunks[0], buf);

        let block2 = Block::bordered()
            .border_style(Style::new().blue())
            .border_set(border::DOUBLE);
        Paragraph::new(instruction_text)
            .left_aligned()
            .block(block2)
            .render(chunks[1], buf);
    }
}
