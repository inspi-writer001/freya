use crate::{CompressMessage, CompressionLevel};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{DefaultTerminal, Frame};
use std::{io, sync::mpsc};

#[derive(Debug)]
pub struct App {
    exit: bool,
    pub is_compressing: bool,
    pub is_decompressing: bool,
    pub progress: f64, // A percentage from 0.0 to 1.0
    pub status_message: String,
    pub receiver: Option<mpsc::Receiver<CompressMessage>>,
    pub last_compression_result: Option<String>,
    pub compression_finished_at: Option<std::time::Instant>,
    pub compression_level: CompressionLevel,
}

impl Default for App {
    fn default() -> Self {
        Self {
            exit: false,
            is_compressing: false,
            is_decompressing: false,
            progress: 0.0,
            status_message: " Press 'o' to compress or 'd' to decompress a file".to_string(),
            receiver: None,
            last_compression_result: None,
            compression_finished_at: None,
            compression_level: CompressionLevel::Normal,
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
                        self.receiver = None;

                        if self.is_decompressing {
                            self.is_decompressing = false;
                            self.status_message = " Decompression complete!".to_string();
                            self.last_compression_result = Some(format!(
                                "\nDecompression successful!\nSaved to: {}\nCompressed: {} bytes\nDecompressed: {} bytes\n",
                                output_path, original_size, compressed_size
                            ));
                        } else {
                            let ratio = if original_size > 0 {
                                (compressed_size as f64 / original_size as f64) * 100.0
                            } else {
                                0.0
                            };

                            self.status_message = " Compression complete!".to_string();
                            self.last_compression_result = Some(format!(
                                "\nCompression successful!\nSaved to: {}\nOriginal: {} bytes\nCompressed: {} bytes ({:.2}% of original)\n",
                                output_path, original_size, compressed_size, ratio
                            ));
                        }

                        self.compression_finished_at = Some(std::time::Instant::now());
                        return;
                    }
                    CompressMessage::Error(e) => {
                        self.is_compressing = false;
                        self.is_decompressing = false;
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
            // Up arrow → decrease toward Fast (slower = smaller file, so intuitive "up = better")
            KeyCode::Up => {
                if !self.is_compressing {
                    self.compression_level = self.compression_level.decrease();
                }
            }
            // Down arrow → increase toward Best
            KeyCode::Down => {
                if !self.is_compressing {
                    self.compression_level = self.compression_level.increase();
                }
            }
            KeyCode::Char('d') => {
                if let Some(input_path) = rfd::FileDialog::new()
                    .add_filter("Zstd compressed", &["zst"])
                    .pick_file()
                {
                    let output_path = input_path.with_extension("");

                    let (tx, rx) = std::sync::mpsc::channel();
                    self.receiver = Some(rx);
                    self.is_compressing = true;
                    self.is_decompressing = true;
                    self.progress = 0.0;
                    self.compression_finished_at = None;

                    self.status_message = format!(
                        " Decompressing {:?}",
                        input_path.file_name().unwrap_or_default()
                    );

                    crate::start_decompression(
                        input_path.to_string_lossy().to_string(),
                        output_path.to_string_lossy().to_string(),
                        tx,
                    );
                }
            }
            KeyCode::Char('o') => {
                // 1. Open the native OS file dialogue
                if let Some(input_path) = rfd::FileDialog::new()
                    .set_title("Select file to compress")
                    .pick_file()
                {
                    // 2. Compute the default output path (e.g., "document.pdf" -> "document.pdf.zst")
                    let mut default_output = input_path.clone();
                    let mut new_extension =
                        default_output.extension().unwrap_or_default().to_os_string();
                    new_extension.push(".zst");
                    default_output.set_extension(new_extension);

                    // 3. Show a "Save As" dialog so the user can choose where to save
                    let default_name = default_output
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let mut save_dialog = rfd::FileDialog::new()
                        .set_title("Save compressed file as")
                        .set_file_name(&default_name)
                        .add_filter("Zstandard", &["zst"]);
                    if let Some(parent) = input_path.parent() {
                        save_dialog = save_dialog.set_directory(parent);
                    }
                    let output_path = save_dialog.save_file().unwrap_or(default_output);

                    // 3. Set up the communication channel for the background thread
                    let (tx, rx) = std::sync::mpsc::channel();
                    self.receiver = Some(rx);
                    self.is_compressing = true;
                    self.progress = 0.0;
                    self.compression_finished_at = None;

                    // Let the user know we're starting
                    self.status_message = format!(
                        " Compressing {:?} [{}]",
                        input_path.file_name().unwrap_or_default(),
                        self.compression_level.label(),
                    );

                    crate::start_compression(
                        input_path.to_string_lossy().to_string(),
                        output_path.to_string_lossy().to_string(),
                        tx,
                        self.compression_level,
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
