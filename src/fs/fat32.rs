use crate::println;
use alloc::vec::Vec;
use alloc::string::String;
use core::convert::TryInto;
use alloc::vec;
use crate::fs::FileHandle;

// FAT32 Disk Layout Constants
const BYTES_PER_SECTOR: usize = 512;
const SECTORS_PER_CLUSTER: usize = 8;
const RESERVED_SECTORS: usize = 32;
const NUM_FATS: usize = 2;
const ROOT_DIR_CLUSTERS: usize = 2;

#[repr(C, packed)]
pub struct FatBootSector {
    jmp_boot: [u8; 3],
    oem_name: [u8; 8],
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sector_count: u16,
    fat_count: u8,
    root_entry_count: u16,
    total_sectors_16: u16,
    media_type: u8,
    sectors_per_fat_16: u16,
    sectors_per_track: u16,
    head_count: u16,
    hidden_sectors: u32,
    total_sectors_32: u32,
    
    // FAT32 specific
    sectors_per_fat_32: u32,
    ext_flags: u16,
    fs_version: u16,
    root_cluster: u32,
    fs_info: u16,
    backup_boot_sector: u16,
    reserved: [u8; 12],
    drive_number: u8,
    reserved1: u8,
    boot_signature: u8,
    volume_id: u32,
    volume_label: [u8; 11],
    fs_type: [u8; 8],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DirectoryEntry {
    // Your existing fields remain unchanged
    name: [u8; 8],
    ext: [u8; 3],
    attributes: u8,
    reserved: u8,
    creation_time_tenths: u8,
    creation_time: u16,
    creation_date: u16,
    last_access_date: u16,
    first_cluster_high: u16,
    last_modification_time: u16,
    last_modification_date: u16,
    first_cluster_low: u16,
    file_size: u32,
}

impl DirectoryEntry {
    // Check if the entry is free (unused)
    pub fn is_free(&self) -> bool {
        self.name[0] == 0xE5 || self.name[0] == 0x00
    }
    
    // Check if the entry is a directory
    pub fn is_directory(&self) -> bool {
        (self.attributes & 0x10) != 0
    }
    
    // Check if the entry is a regular file
    pub fn is_file(&self) -> bool {
        (self.attributes & 0x10) == 0 && (self.attributes & 0x08) == 0
    }
    
    // Get the file name
    pub fn get_name(&self) -> String {
        let mut name = String::new();
        
        // Copy the base name (trim spaces)
        for i in 0..8 {
            if self.name[i] == b' ' {
                break;
            }
            name.push(self.name[i] as char);
        }
        
        // Add the extension if it exists
        if self.ext[0] != b' ' {
            name.push('.');
            for i in 0..3 {
                if self.ext[i] == b' ' {
                    break;
                }
                name.push(self.ext[i] as char);
            }
        }
        
        name
    }
    
    // Get the first cluster
    pub fn get_first_cluster(&self) -> u32 {
        ((self.first_cluster_high as u32) << 16) | (self.first_cluster_low as u32)
    }
}

// Simple disk interface for reading/writing sectors
pub trait Disk {
    fn read_sector(&self, sector: u32, buffer: &mut [u8]) -> Result<(), &'static str>;
    fn write_sector(&mut self, sector: u32, buffer: &[u8]) -> Result<(), &'static str>;
    fn total_sectors(&self) -> u32;
}

// Memory-based disk for testing
pub struct MemoryDisk {
    data: Vec<u8>,
    sector_size: usize,
}

impl MemoryDisk {
    pub fn new(sector_size: usize, total_sectors: usize) -> Self {
        let size = sector_size * total_sectors;
        MemoryDisk {
            data: vec![0; size],
            sector_size,
        }
    }
}

impl Disk for MemoryDisk {
    fn read_sector(&self, sector: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
        let start = (sector as usize) * self.sector_size;
        let end = start + buffer.len().min(self.sector_size);
        
        if end > self.data.len() {
            return Err("Sector read out of bounds");
        }
        
        buffer[..self.sector_size].copy_from_slice(&self.data[start..end]);
        Ok(())
    }
    
    fn write_sector(&mut self, sector: u32, buffer: &[u8]) -> Result<(), &'static str> {
        let start = (sector as usize) * self.sector_size;
        let end = start + buffer.len().min(self.sector_size);
        
        if end > self.data.len() {
            return Err("Sector write out of bounds");
        }
        
        self.data[start..end].copy_from_slice(&buffer[..self.sector_size]);
        Ok(())
    }
    
    fn total_sectors(&self) -> u32 {
        (self.data.len() / self.sector_size) as u32
    }
}

pub struct FileSystem<D: Disk> {
    disk: D,
    fat_start_sector: u32,
    data_start_sector: u32,
    root_dir_cluster: u32,
    sectors_per_cluster: u32,
    bytes_per_sector: u32,
    next_file_handle_id: usize,
    open_files: Vec<(FileHandle, Vec<u32>)>, // FileHandle and cluster chain
}

impl<D: Disk> FileSystem<D> {
    pub fn new(disk: D) -> Self {
        FileSystem {
            disk,
            fat_start_sector: 0,
            data_start_sector: 0,
            root_dir_cluster: 0,
            sectors_per_cluster: 0,
            bytes_per_sector: 0,
            next_file_handle_id: 1,
            open_files: Vec::new(),
        }
    }
    
    // Initialize the filesystem by reading the boot sector
    fn read_boot_sector(&mut self) -> Result<FatBootSector, &'static str> {
        let mut buffer = [0u8; BYTES_PER_SECTOR];
        self.disk.read_sector(0, &mut buffer)?;
        
        // Safety: This is unsafe because we're interpreting the bytes as a struct
        // The FatBootSector struct must match the on-disk layout exactly
        let boot_sector = unsafe {
            core::ptr::read_unaligned(buffer.as_ptr() as *const FatBootSector)
        };
        
        Ok(boot_sector)
    }
    
    // Convert a cluster number to a sector number
    fn cluster_to_sector(&self, cluster: u32) -> u32 {
        // First data cluster (2) starts at data_start_sector
        // Cluster numbers start at 2 in FAT32
        let data_cluster = cluster - 2;
        self.data_start_sector + (data_cluster * self.sectors_per_cluster)
    }
    
    // Read the FAT to get the next cluster in a chain
    fn get_next_cluster(&self, cluster: u32) -> Result<u32, &'static str> {
        let fat_offset = cluster * 4; // Each FAT entry is 4 bytes
        let fat_sector = self.fat_start_sector + (fat_offset / self.bytes_per_sector);
        let entry_offset = (fat_offset % self.bytes_per_sector) as usize;
        
        let mut buffer = [0u8; BYTES_PER_SECTOR];
        self.disk.read_sector(fat_sector, &mut buffer)?;
        
        let next_cluster = u32::from_le_bytes(buffer[entry_offset..entry_offset+4]
            .try_into()
            .map_err(|_| "Invalid FAT entry")?);
        
        // Mask out the top 4 bits (reserved in FAT32)
        let next_cluster = next_cluster & 0x0FFFFFFF;
        
        // Check for end-of-chain marker
        if next_cluster >= 0x0FFFFFF8 {
            return Ok(0); // End of chain
        }
        
        Ok(next_cluster)
    }
    
    // Read a full cluster into a buffer
    fn read_cluster(&self, cluster: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
        let start_sector = self.cluster_to_sector(cluster);
        let cluster_size = (self.sectors_per_cluster * self.bytes_per_sector) as usize;
        
        if buffer.len() < cluster_size {
            return Err("Buffer too small for cluster");
        }
        
        for i in 0..self.sectors_per_cluster {
            let sector = start_sector + i;
            let offset = (i * self.bytes_per_sector) as usize;
            let sector_buffer = &mut buffer[offset..offset + self.bytes_per_sector as usize];
            self.disk.read_sector(sector, sector_buffer)?;
        }
        
        Ok(())
    }
    
    // Find a file or directory by name in a directory cluster
    fn find_in_directory(&self, dir_cluster: u32, name: &str) -> Result<Option<DirectoryEntry>, &'static str> {
        let cluster_size = (self.sectors_per_cluster * self.bytes_per_sector) as usize;
        let mut buffer = vec![0u8; cluster_size];
        let mut current_cluster = dir_cluster;
        
        let name_upper = name.to_uppercase();
        
        while current_cluster != 0 {
            self.read_cluster(current_cluster, &mut buffer)?;
            
            // Iterate through directory entries in the cluster
            let entries_per_cluster = cluster_size / core::mem::size_of::<DirectoryEntry>();
            
            for i in 0..entries_per_cluster {
                let offset = i * core::mem::size_of::<DirectoryEntry>();
                let entry_ptr = buffer[offset..].as_ptr() as *const DirectoryEntry;
                
                // Safety: This assumes DirectoryEntry matches disk format exactly
                let entry = unsafe { &*entry_ptr };
                
                if entry.is_free() {
                    continue;
                }
                
                let entry_name = entry.get_name();
                if entry_name.to_uppercase() == name_upper {
                    return Ok(Some(*entry));
                }
            }
            
            // Move to the next cluster in the chain
            current_cluster = self.get_next_cluster(current_cluster)?;
        }
        
        Ok(None)
    }
    
    // Follow a path to find a file or directory
    fn find_by_path(&self, path: &str) -> Result<Option<DirectoryEntry>, &'static str> {
        let mut current_cluster = self.root_dir_cluster;
        
        // Split the path into components
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        
        if components.is_empty() {
            return Err("Invalid path");
        }
        
        // Navigate through directories
        for (i, component) in components.iter().enumerate() {
            match self.find_in_directory(current_cluster, component)? {
                Some(entry) => {
                    if i == components.len() - 1 {
                        // Last component, return the entry
                        return Ok(Some(entry));
                    } else if entry.is_directory() {
                        // Continue to the next directory
                        current_cluster = entry.get_first_cluster();
                    } else {
                        // Not a directory but not the last component
                        return Err("Not a directory");
                    }
                }
                None => return Ok(None),
            }
        }
        
        Ok(None)
    }
    
    // Build a cluster chain for a file
    fn build_cluster_chain(&self, start_cluster: u32) -> Result<Vec<u32>, &'static str> {
        let mut chain = Vec::new();
        let mut current_cluster = start_cluster;
        
        // Prevent infinite loops due to corrupted FAT
        let max_clusters = 1_000_000; // Arbitrary limit
        let mut count = 0;
        
        while current_cluster != 0 && current_cluster < 0x0FFFFFF8 && count < max_clusters {
            chain.push(current_cluster);
            current_cluster = self.get_next_cluster(current_cluster)?;
            count += 1;
        }
        
        Ok(chain)
    }
}

impl<D: Disk> crate::fs::FileSystem for FileSystem<D> {
    fn init(&mut self) -> Result<(), &'static str> {
        // Read the boot sector
        let boot_sector = self.read_boot_sector()?;
        
        // Initialize filesystem parameters
        self.bytes_per_sector = boot_sector.bytes_per_sector as u32;
        self.sectors_per_cluster = boot_sector.sectors_per_cluster as u32;
        self.root_dir_cluster = boot_sector.root_cluster;
        
        // Calculate important sector locations
        self.fat_start_sector = boot_sector.reserved_sector_count as u32;
        let fat_size = boot_sector.sectors_per_fat_32;
        self.data_start_sector = self.fat_start_sector + (NUM_FATS as u32 * fat_size);
        
        println!("FAT32 filesystem initialized:");
        println!("  Bytes per sector: {}", self.bytes_per_sector);
        println!("  Sectors per cluster: {}", self.sectors_per_cluster);
        println!("  FAT start sector: {}", self.fat_start_sector);
        println!("  Data start sector: {}", self.data_start_sector);
        println!("  Root directory cluster: {}", self.root_dir_cluster);
        
        Ok(())
    }
    
    fn open(&mut self, path: &str) -> Result<FileHandle, &'static str> {
        // Find the file by path
        let entry = match self.find_by_path(path)? {
            Some(entry) => entry,
            None => return Err("File not found"),
        };
        
        if entry.is_directory() {
            return Err("Cannot open a directory as a file");
        }
        
        let handle = FileHandle {
            id: self.next_file_handle_id,
            position: 0,
            size: entry.file_size as usize,
        };
        
        // Build the cluster chain for the file
        let cluster_chain = self.build_cluster_chain(entry.get_first_cluster())?;
        
        // Store the file handle and its cluster chain
        self.open_files.push((handle, cluster_chain));  // This now works because handle implements Copy
        
        // Increment the next file handle ID
        self.next_file_handle_id += 1;
        
        Ok(handle)  // Returns a copy of the handle
    }
    
    fn read(&self, handle: &mut FileHandle, buffer: &mut [u8]) -> Result<usize, &'static str> {
        // Find the file in the open files list
        let chain = match self.open_files.iter().find(|(h, _)| h.id == handle.id) {
            Some((_, chain)) => chain,
            None => return Err("Invalid file handle"),
        };
        
        // Check if we're at EOF
        if handle.position >= handle.size {
            return Ok(0);
        }
        
        // Calculate which cluster and offset within the cluster we need
        let cluster_size = (self.sectors_per_cluster * self.bytes_per_sector) as usize;
        let cluster_index = handle.position / cluster_size;
        
        if cluster_index >= chain.len() {
            return Err("Invalid cluster index");
        }
        
        let cluster = chain[cluster_index];
        let cluster_offset = handle.position % cluster_size;
        
        // Calculate how much to read
        let bytes_to_read = buffer.len().min(handle.size - handle.position);
        
        // Read the data
        let mut temp_buffer = vec![0u8; cluster_size];
        self.read_cluster(cluster, &mut temp_buffer)?;
        
        buffer[..bytes_to_read].copy_from_slice(&temp_buffer[cluster_offset..cluster_offset + bytes_to_read]);
        
        // Update position
        handle.position += bytes_to_read;
        
        Ok(bytes_to_read)
    }
    
    fn write(&mut self, _handle: &mut FileHandle, _buffer: &[u8]) -> Result<usize, &'static str> {
        // Writing is more complex and involves FAT updates
        // For simplicity, our initial implementation is read-only
        Err("Write operations not implemented")
    }
    
    fn close(&mut self, handle: FileHandle) -> Result<(), &'static str> {
        // Remove the file from the open files list
        let position = self.open_files.iter().position(|(h, _)| h.id == handle.id);
        
        match position {
            Some(index) => {
                self.open_files.remove(index);
                Ok(())
            }
            None => Err("Invalid file handle"),
        }
    }
}