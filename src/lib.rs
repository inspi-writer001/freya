pub mod app;
pub mod compression;
pub mod ui;

pub use app::*;
pub use compression::*;
pub use ui::*;
/// The three compression presets exposed to the user.
/// Up/Down arrows cycle through them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionLevel {
    Fast,   // zstd level 1
    Normal, // zstd level 3 (zstd default)
    Best,   // zstd level 19
}

impl CompressionLevel {
    /// The zstd integer level to pass to the encoder.
    pub fn zstd_level(self) -> i32 {
        match self {
            CompressionLevel::Fast => 1,
            CompressionLevel::Normal => 3,
            CompressionLevel::Best => 19,
        }
    }

    /// Human-readable label shown in the UI.
    pub fn label(self) -> &'static str {
        match self {
            CompressionLevel::Fast => "Fast",
            CompressionLevel::Normal => "Normal",
            CompressionLevel::Best => "Best",
        }
    }

    /// Cycle upward (Down arrow — toward Best).
    pub fn increase(self) -> Self {
        match self {
            CompressionLevel::Fast => CompressionLevel::Normal,
            CompressionLevel::Normal => CompressionLevel::Best,
            CompressionLevel::Best => CompressionLevel::Best,
        }
    }

    /// Cycle downward (Up arrow — toward Fast).
    pub fn decrease(self) -> Self {
        match self {
            CompressionLevel::Fast => CompressionLevel::Fast,
            CompressionLevel::Normal => CompressionLevel::Fast,
            CompressionLevel::Best => CompressionLevel::Normal,
        }
    }
}

pub enum CompressMessage {
    Progress {
        bytes_processed: u64,
        total_bytes: u64,
    },
    Finished {
        original_size: u64,
        compressed_size: u64,
        output_path: String,
    },
    Error(String),
}
