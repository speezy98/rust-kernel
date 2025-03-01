

use alloc::vec::Vec;
use spin::Mutex;
use crate::fs::fat32::DiskIO;

/// Memory-based disk for testing
pub struct MemoryDisk {
    data: Mutex<Vec<u8>>,
    sector_size: usize,
}

impl MemoryDisk {
    pub fn new(size_in_sectors: usize, sector_size: usize) -> Self {
        let data = vec![0u8; size_in_sectors * sector_size];
        MemoryDisk {
            data: Mutex::new(data),
            sector_size,
        }
    }
    
    pub fn from_bytes(data: Vec<u8>, sector_size: usize) -> Self {
        MemoryDisk {
            data: Mutex::new(data),
            sector_size,
        }
    }
}

impl DiskIO for MemoryDisk {
    fn read_sectors(&self, start_sector: u32, sector_count: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
        let data = self.data.lock();
        let start_offset = start_sector as usize * self.sector_size;
        let end_offset = start_offset + (sector_count as usize * self.sector_size);
        
        if end_offset > data.len() {
            return Err("Read beyond disk boundaries");
        }
        
        if buffer.len() < (sector_count as usize * self.sector_size) {
            return Err("Buffer too small for requested sectors");
        }
        
        buffer[..(sector_count as usize * self.sector_size)]
            .copy_from_slice(&data[start_offset..end_offset]);
        
        Ok(())
    }
    
    fn write_sectors(&self, start_sector: u32, sector_count: u32, buffer: &[u8]) -> Result<(), &'static str> {
        let mut data = self.data.lock();
        let start_offset = start_sector as usize * self.sector_size;
        let end_offset = start_offset + (sector_count as usize * self.sector_size);
        
        if end_offset > data.len() {
            return Err("Write beyond disk boundaries");
        }
        
        if buffer.len() < (sector_count as usize * self.sector_size) {
            return Err("Buffer too small for requested sectors");
        }
        
        data[start_offset..end_offset]
            .copy_from_slice(&buffer[..(sector_count as usize * self.sector_size)]);
        
        Ok(())
    }
}

/// Trait for creating disk drivers
pub trait DiskDriver: DiskIO {
    fn sector_size(&self) -> usize;
    fn total_sectors(&self) -> usize;
}

impl DiskDriver for MemoryDisk {
    fn sector_size(&self) -> usize {
        self.sector_size
    }
    
    fn total_sectors(&self) -> usize {
        self.data.lock().len() / self.sector_size
    }
}

/// Simple ATA PIO driver for real hardware
pub struct AtaPioDisk {
    is_primary: bool,
    is_master: bool,
    sector_count: Mutex<usize>,
}

impl AtaPioDisk {
    pub fn new(is_primary: bool, is_master: bool) -> Self {
        let mut disk = AtaPioDisk {
            is_primary,
            is_master,
            sector_count: Mutex::new(0),
        };
        
        // Identify device to get sector count
        let mut identify_buffer = [0u8; 512];
        if let Ok(_) = disk.identify(&mut identify_buffer) {
            // Sectors are at words 60-61 for 28-bit LBA
            // or 100-103 for 48-bit LBA
            let lba48_sectors = 
                u64::from(u16::from_le_bytes([identify_buffer[200], identify_buffer[201]])) |
                (u64::from(u16::from_le_bytes([identify_buffer[202], identify_buffer[203]])) << 16) |
                (u64::from(u16::from_le_bytes([identify_buffer[204], identify_buffer[205]])) << 32) |
                (u64::from(u16::from_le_bytes([identify_buffer[206], identify_buffer[207]])) << 48);
                
            let lba28_sectors = 
                u32::from(u16::from_le_bytes([identify_buffer[120], identify_buffer[121]])) |
                (u32::from(u16::from_le_bytes([identify_buffer[122], identify_buffer[123]])) << 16);
                
            let total_sectors = if lba48_sectors > 0 && lba48_sectors < 0xFFFF_FFFF_FFFF_FFFF {
                lba48_sectors as usize
            } else {
                lba28_sectors as usize
            };
            
            *disk.sector_count.lock() = total_sectors;
        }
        
        disk
    }
    
    fn io_base(&self) -> u16 {
        if self.is_primary {
            0x1F0
        } else {
            0x170
        }
    }
    
    fn control_base(&self) -> u16 {
        if self.is_primary {
            0x3F6
        } else {
            0x376
        }
    }
    
    fn identify(&self, buffer: &mut [u8; 512]) -> Result<(), &'static str> {
        let io_base = self.io_base();
        let master_bit = if self.is_master { 0 } else { 0x10 };
        
        // Select drive
        unsafe {
            x86_64::instructions::port::Port::new(io_base + 6).write(master_bit as u8);
        }
        
        // Set unused bits to zero
        unsafe {
            x86_64::instructions::port::Port::new(io_base + 1).write(0u8);
            x86_64::instructions::port::Port::new(io_base + 2).write(0u8);
            x86_64::instructions::port::Port::new(io_base + 3).write(0u8);
            x86_64::instructions::port::Port::new(io_base + 4).write(0u8);
            x86_64::instructions::port::Port::new(io_base + 5).write(0u8);
        }
        
        // Send IDENTIFY command
        unsafe {
            x86_64::instructions::port::Port::new(io_base + 7).write(0xECu8);
        }
        
        // Check if device exists
        let status: u8 = unsafe {
            x86_64::instructions::port::Port::new(io_base + 7).read()
        };
        
        if status == 0 {
            return Err("Drive does not exist");
        }
        
        // Wait for data ready
        loop {
            let status: u8 = unsafe {
                x86_64::instructions::port::Port::new(io_base + 7).read()
            };
            
            if status & 0x08 != 0 {
                // DRQ is set, data is ready
                break;
            }
            
            if status & 0x01 != 0 {
                // Error
                return Err("Error during identify");
            }
        }
        
        // Read data
        let data_port = unsafe {
            x86_64::instructions::port::Port::<u16>::new(io_base)
        };
        
        for i in 0..256 {
            let data: u16 = unsafe { data_port.read() };
            buffer[i * 2] = (data & 0xFF) as u8;
            buffer[i * 2 + 1] = (data >> 8) as u8;
        }
        
        Ok(())
    }
}

impl DiskIO for AtaPioDisk {
    fn read_sectors(&self, start_sector: u32, sector_count: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
        if sector_count == 0 {
            return Ok(());
        }
        
        if buffer.len() < (sector_count as usize * 512) {
            return Err("Buffer too small for requested sectors");
        }
        
        let io_base = self.io_base();
        let master_bit = if self.is_master { 0 } else { 0x10 };
        
        for sector_idx in 0..sector_count {
            let lba = start_sector + sector_idx;
            
            // Select drive and upper LBA bits
            unsafe {
                x86_64::instructions::port::Port::new(io_base + 6).write(
                    (master_bit | ((lba >> 24) & 0x0F)) as u8
                );
            }
            
            // Set sector count to 1 (we read one sector at a time)
            unsafe {
                x86_64::instructions::port::Port::new(io_base + 2).write(1u8);
            }
            
            // Send LBA address
            unsafe {
                x86_64::instructions::port::Port::new(io_base + 3).write((lba & 0xFF) as u8);
                x86_64::instructions::port::Port::new(io_base + 4).write(((lba >> 8) & 0xFF) as u8);
                x86_64::instructions::port::Port::new(io_base + 5).write(((lba >> 16) & 0xFF) as u8);
            }
            
            // Send READ SECTORS command
            unsafe {
                x86_64::instructions::port::Port::new(io_base + 7).write(0x20u8);
            }
            
            // Wait for data ready
            loop {
                let status: u8 = unsafe {
                    x86_64::instructions::port::Port::new(io_base + 7).read()
                };
                
                if status & 0x08 != 0 {
                    // DRQ is set, data is ready
                    break;
                }
                
                if status & 0x01 != 0 {
                    // Error
                    return Err("Error during read");
                }
            }
            
            // Read data
            let data_port = unsafe {
                x86_64::instructions::port::Port::<u16>::new(io_base)
            };
            
            let offset = sector_idx as usize * 512;
            for i in 0..256 {
                let data: u16 = unsafe { data_port.read() };
                buffer[offset + i * 2] = (data & 0xFF) as u8;
                buffer[offset + i * 2 + 1] = (data >> 8) as u8;
            }
        }
        
        Ok(())
    }
    
    fn write_sectors(&self, start_sector: u32, sector_count: u32, buffer: &[u8]) -> Result<(), &'static str> {
        if sector_count == 0 {
            return Ok(());
        }
        
        if buffer.len() < (sector_count as usize * 512) {
            return Err("Buffer too small for requested sectors");
        }
        
        let io_base = self.io_base();
        let master_bit = if self.is_master { 0 } else { 0x10 };
        
        for sector_idx in 0..sector_count {
            let lba = start_sector + sector_idx;
            
            // Select drive and upper LBA bits
            unsafe {
                x86_64::instructions::port::Port::new(io_base + 6).write(
                    (master_bit | ((lba >> 24) & 0x0F)) as u8
                );
            }
            
            // Set sector count to 1 (we write one sector at a time)
            unsafe {
                x86_64::instructions::port::Port::new(io_base + 2).write(1u8);
            }
            
            // Send LBA address
            unsafe {
                x86_64::instructions::port::Port::new(io_base + 3).write((lba & 0xFF) as u8);
                x86_64::instructions::port::Port::new(io_base + 4).write(((lba >> 8) & 0xFF) as u8);
                x86_64::instructions::port::Port::new(io_base + 5).write(((lba >> 16) & 0xFF) as u8);
            }
            
            // Send WRITE SECTORS command
            unsafe {
                x86_64::instructions::port::Port::new(io_base + 7).write(0x30u8);
            }
            
            // Wait for ready to accept data
            loop {
                let status: u8 = unsafe {
                    x86_64::instructions::port::Port::new(io_base + 7).read()
                };
                
                if status & 0x08 != 0 {
                    // DRQ is set, ready for data
                    break;
                }
                
                if status & 0x01 != 0 {
                    // Error
                    return Err("Error during write preparation");
                }
            }
            
            // Write data
            let data_port = unsafe {
                x86_64::instructions::port::Port::<u16>::new(io_base)
            };
            
            let offset = sector_idx as usize * 512;
            for i in 0..256 {
                let low_byte = buffer[offset + i * 2] as u16;
                let high_byte = buffer[offset + i * 2 + 1] as u16;
                let word = low_byte | (high_byte << 8);
                unsafe { data_port.write(word); }
            }
            
            // Flush cache
            unsafe {
                x86_64::instructions::port::Port::new(io_base + 7).write(0xE7u8);
            }
            
            // Wait for operation to complete
            loop {
                let status: u8 = unsafe {
                    x86_64::instructions::port::Port::new(io_base + 7).read()
                };
                
                if status & 0x80 == 0 && status & 0x40 != 0 {
                    // BSY clear and RDY set
                    break;
                }
                
                if status & 0x01 != 0 {
                    // Error
                    return Err("Error during write");
                }
            }
        }
        
        Ok(())
    }
}

impl DiskDriver for AtaPioDisk {
    fn sector_size(&self) -> usize {
        512 // ATA sectors are always 512 bytes
    }
    
    fn total_sectors(&self) -> usize {
        *self.sector_count.lock()
    }
}