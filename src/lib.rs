pub mod app;
pub mod compression;
pub mod ui;

pub use app::*;
pub use compression::*;
pub use ui::*;

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
