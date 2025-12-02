use goldboot_image::ImageHandle;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockState {
    Pending,   // Not yet written
    Writing,   // Currently being written
    Written,   // Successfully written
}

pub struct WriteProgress {
    pub percentage: f32,           // 0.0 to 1.0
    pub read_speed: f64,           // Bytes per second
    pub write_speed: f64,          // Bytes per second
    pub blocks_total: usize,       // Total number of blocks
    pub blocks_written: usize,     // Number of blocks written
    pub blocks_writing: usize,     // Number of blocks currently being written
    pub block_states: Vec<BlockState>, // State of each block
    pub start_time: Instant,
}

impl WriteProgress {
    pub fn new(total_blocks: usize) -> Self {
        Self {
            percentage: 0.0,
            read_speed: 0.0,
            write_speed: 0.0,
            blocks_total: total_blocks,
            blocks_written: 0,
            blocks_writing: 0,
            block_states: vec![BlockState::Pending; total_blocks],
            start_time: Instant::now(),
        }
    }

    pub fn elapsed_seconds(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }
}

pub struct AppState {
    // Image selection
    pub images: Vec<ImageHandle>,
    pub selected_image: Option<String>,

    // Device selection
    pub devices: Vec<block_utils::Device>,
    pub selected_device: Option<String>,

    // Confirmation
    pub confirm_progress: f32, // 0.0 to 1.0

    // Registry login
    pub registry_address: String,
    pub registry_password: String,
    pub show_registry_dialog: bool,

    // Image writing - detailed progress tracking
    pub write_progress: Option<Arc<Mutex<WriteProgress>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            images: crate::library::ImageLibrary::find_all().unwrap_or_default(),
            selected_image: None,
            devices: Vec::new(), // Loaded on-demand in select_device screen
            selected_device: None,
            confirm_progress: 0.0,
            registry_address: String::new(),
            registry_password: String::new(),
            show_registry_dialog: false,
            write_progress: None,
        }
    }

    /// Initialize write progress when entering ApplyImage screen
    pub fn init_write_progress(&mut self, total_size_bytes: u64) {
        // Calculate number of blocks (using 4MB blocks for visualization)
        const BLOCK_SIZE: u64 = 4 * 1024 * 1024; // 4MB blocks
        let total_blocks = ((total_size_bytes + BLOCK_SIZE - 1) / BLOCK_SIZE) as usize;

        // Cap at reasonable number for visualization (e.g., 1000 blocks max)
        let total_blocks = total_blocks.min(1000);

        self.write_progress = Some(Arc::new(Mutex::new(WriteProgress::new(total_blocks))));
    }
}
