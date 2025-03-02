pub mod fat32;

pub use fat32::FileSystem as Fat32FileSystem;

pub trait FileSystem {
    fn init(&mut self) -> Result<(), &'static str>;
    fn open(&mut self, path: &str) -> Result<FileHandle, &'static str>; // Changed from &self to &mut self
    fn read(&self, handle: &mut FileHandle, buffer: &mut [u8]) -> Result<usize, &'static str>;
    fn write(&mut self, handle: &mut FileHandle, buffer: &[u8]) -> Result<usize, &'static str>;
    fn close(&mut self, handle: FileHandle) -> Result<(), &'static str>;
}

#[derive(Debug, Clone, Copy)]  // Add Copy trait
pub struct FileHandle {
    pub id: usize,
    pub position: usize,
    pub size: usize,
}