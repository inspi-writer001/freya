use crate::CompressMessage;
use std::io::{Read, Write};
use std::sync::mpsc;

pub fn start_compression(
    input_path: String,
    output_path: String,
    tx: mpsc::Sender<CompressMessage>,
) {
    std::thread::spawn(move || {
        let run = || -> std::io::Result<()> {
            let mut input_file = std::fs::File::open(&input_path)?;
            let total_bytes = input_file.metadata()?.len();
            let output_file = std::fs::File::create(&output_path)?;

            let mut encoder = zstd::stream::Encoder::new(output_file, 3)?;
            let mut buffer = [0u8; 64 * 1024]; // 64KB buffer
            let mut bytes_processed: u64 = 0;

            loop {
                let bytes_read = input_file.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                encoder.write_all(&buffer[..bytes_read])?;
                bytes_processed += bytes_read as u64;
                let _ = tx.send(CompressMessage::Progress {
                    bytes_processed,
                    total_bytes,
                });
            }

            encoder.finish()?;
            Ok(())
        };

        match run() {
            Ok(_) => {
                let _ = tx.send(CompressMessage::Finished);
            }
            Err(e) => {
                let _ = tx.send(CompressMessage::Error(e.to_string()));
            }
        }
    });
}
