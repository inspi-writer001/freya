use crate::CompressMessage;
use std::io::{BufReader, Read, Write};
use std::sync::mpsc;
use zstd::stream::Decoder;

pub fn start_compression(
    input_path: String,
    output_path: String,
    tx: mpsc::Sender<CompressMessage>,
) {
    std::thread::spawn(move || {
        let run = || -> std::io::Result<(u64, u64, String)> {
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

            let output_file = encoder.finish()?;
            let compressed_size = output_file.metadata()?.len();
            Ok((total_bytes, compressed_size, output_path))
        };

        match run() {
            Ok((original_size, compressed_size, path)) => {
                let _ = tx.send(CompressMessage::Finished {
                    original_size,
                    compressed_size,
                    output_path: path,
                });
            }
            Err(e) => {
                let _ = tx.send(CompressMessage::Error(e.to_string()));
            }
        }
    });
}

// reads a .zst file and writes the original bytes back out.
// Decoder::new() will reject non-zstd input, so we don't need separate validation.
pub fn start_decompression(
    input_path: String,
    output_path: String,
    tx: mpsc::Sender<CompressMessage>,
) {
    std::thread::spawn(move || {
        let run = || -> std::io::Result<(u64, u64, String)> {
            let input_file = std::fs::File::open(&input_path)?;
            let compressed_size = input_file.metadata()?.len();

            // BufReader here because Decoder does many small reads internally
            let mut decoder = Decoder::new(BufReader::new(input_file))?;

            let mut output_file = std::fs::File::create(&output_path)?;
            let mut buffer = [0u8; 64 * 1024];
            let mut bytes_processed: u64 = 0;

            loop {
                let bytes_read = decoder.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                output_file.write_all(&buffer[..bytes_read])?;
                bytes_processed += bytes_read as u64;
                let _ = tx.send(CompressMessage::Progress {
                    bytes_processed,
                    total_bytes: compressed_size,
                });
            }

            // Return compressed size first, decompressed second the Finished
            // handler in app.rs knows to flip the labels when is_decompressing is set
            Ok((compressed_size, bytes_processed, output_path))
        };

        match run() {
            Ok((compressed_size, decompressed_size, path)) => {
                let _ = tx.send(CompressMessage::Finished {
                    original_size: compressed_size,
                    compressed_size: decompressed_size,
                    output_path: path,
                });
            }
            Err(e) => {
                let _ = tx.send(CompressMessage::Error(e.to_string()));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    // make sure we get the exact same bytes back.
    #[test]
    fn compress_then_decompress_roundtrip() {
        let dir = std::env::temp_dir().join("freya_test_roundtrip");
        std::fs::create_dir_all(&dir).unwrap();

        let original_path = dir.join("input.txt");
        let compressed_path = dir.join("input.txt.zst");
        let decompressed_path = dir.join("output.txt");

        // Repeat enough to actually exercise chunked reading (5KB+)
        let original_data = b"Hello Freya! This is a round-trip compression accelerated test.\n".repeat(100);
        std::fs::write(&original_path, &original_data).unwrap();

        // Step 1: compress the file and drain the channel until we get Finished
        let (tx, rx) = mpsc::channel();
        start_compression(
            original_path.to_string_lossy().to_string(),
            compressed_path.to_string_lossy().to_string(),
            tx,
        );

        let mut finished = false;
        for msg in rx {
            if let CompressMessage::Finished { .. } = msg {
                finished = true;
                break;
            }
            if let CompressMessage::Error(e) = msg {
                panic!("Compression failed: {}", e);
            }
        }
        assert!(finished, "Never received Finished message from compression");

        // Step 2: decompress the .zst we just created
        let (tx, rx) = mpsc::channel();
        start_decompression(
            compressed_path.to_string_lossy().to_string(),
            decompressed_path.to_string_lossy().to_string(),
            tx,
        );

        let mut finished = false;
        for msg in rx {
            if let CompressMessage::Finished { .. } = msg {
                finished = true;
                break;
            }
            if let CompressMessage::Error(e) = msg {
                panic!("Decompression failed: {}", e);
            }
        }
        assert!(finished, "Never received Finished message from decompression");

        // Step 3: byte-for-byte comparison -- if this fails, something is very wrong
        let result = std::fs::read(&decompressed_path).unwrap();
        assert_eq!(original_data, result, "Decompressed data does not match original");

        // Don't leave temp files lying around
        std::fs::remove_dir_all(&dir).ok();
    }
}
