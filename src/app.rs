use crate::CompressMessage;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Gauge, Paragraph, Widget},
};
use std::{io, sync::mpsc};

#[derive(Debug)]
pub struct App {
    exit: bool,
    pub is_compressing: bool,
    pub progress: f64, // A percentage from 0.0 to 1.0
    pub status_message: String,
    pub receiver: Option<mpsc::Receiver<CompressMessage>>,
    pub last_compression_result: Option<String>,
    pub compression_finished_at: Option<std::time::Instant>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            exit: false,
            is_compressing: false,
            progress: 0.0,
            status_message: " Start by pressing 'o' to open a file".to_string(),
            receiver: None,
            last_compression_result: None,
            compression_finished_at: None,
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

        // Handle the 2-second auto-exit delay
        if let Some(finished_at) = self.compression_finished_at {
            if finished_at.elapsed() >= std::time::Duration::from_secs(2) {
                self.exit();
            }
        }

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
                    CompressMessage::Finished {
                        original_size,
                        compressed_size,
                        output_path,
                    } => {
                        self.is_compressing = false;
                        self.progress = 1.0;
                        self.status_message = " Compression complete!".to_string();
                        self.receiver = None;

                        let ratio = if original_size > 0 {
                            (compressed_size as f64 / original_size as f64) * 100.0
                        } else {
                            0.0
                        };

                        self.last_compression_result = Some(format!(
                            "\n✅ Compression successful!\n📂 Saved to: {}\n📊 Original: {} bytes \n📉 Compressed: {} bytes ({:.2}% of original)\n",
                            output_path, original_size, compressed_size, ratio
                        ));

                        self.compression_finished_at = Some(std::time::Instant::now());
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
        if !self.is_compressing && self.progress > 0.0 {
            self.progress = 0.0;
        }

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
                    self.compression_finished_at = None;

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
        let mut constraints = vec![
            Constraint::Length(5), // Height for the description block (borders + text + padding)
            Constraint::Length(3), // Height for the instruction block
        ];

        let show_progress = self.is_compressing || self.progress > 0.0;
        if show_progress {
            constraints.push(Constraint::Length(3)); // Height for the progress block
        }
        constraints.push(Constraint::Min(0)); // The remaining empty space on the screen

        let chunks = Layout::vertical(constraints).split(area);
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

        if show_progress {
            let percentage = (self.progress * 100.0).clamp(0.0, 100.0) as u16;
            let gauge = Gauge::default()
                .block(
                    Block::bordered()
                        .title(" Progress ")
                        .border_style(Style::default().blue()),
                )
                .gauge_style(Style::default().fg(ratatui::style::Color::Yellow))
                .ratio(self.progress.clamp(0.0, 1.0))
                .label(format!("{}%", percentage));
            gauge.render(chunks[2], buf);
        }
    }
}
