//! Helpers for using embedded-sdmmc on Linux

use chrono::Timelike;
use embedded_sdmmc::{
    blockdevice::BLOCK_LEN_U64, Block, BlockCount, BlockDevice, BlockIdx, TimeSource, Timestamp,
};
use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::Path;

#[derive(Debug)]
pub struct LinuxBlockDevice {
    file: RefCell<File>,
    print_blocks: bool,
}

impl LinuxBlockDevice {
    pub fn new<P>(device_name: P, print_blocks: bool) -> Result<LinuxBlockDevice, std::io::Error>
    where
        P: AsRef<Path>,
    {
        Ok(LinuxBlockDevice {
            file: RefCell::new(
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(device_name)?,
            ),
            print_blocks,
        })
    }
}

impl BlockDevice for LinuxBlockDevice {
    type Error = std::io::Error;

    fn read(
        &mut self,
        blocks: &mut [Block],
        first_block_index: BlockIdx,
    ) -> Result<(), Self::Error> {
        self.file
            .borrow_mut()
            .seek(SeekFrom::Start(first_block_index.0 * BLOCK_LEN_U64))?;
        for block in blocks.iter_mut() {
            self.file.borrow_mut().read_exact(block)?;
            if self.print_blocks {
                println!("Read block {:?}: {:?}", first_block_index, &block);
            }
        }
        Ok(())
    }

    fn write(&mut self, blocks: &[Block], first_block_index: BlockIdx) -> Result<(), Self::Error> {
        self.file
            .borrow_mut()
            .seek(SeekFrom::Start(first_block_index.0 * BLOCK_LEN_U64))?;
        for block in blocks.iter() {
            self.file.borrow_mut().write_all(block)?;
            if self.print_blocks {
                println!("Wrote: {:?}", &block);
            }
        }
        Ok(())
    }

    fn block_count(&self) -> Result<BlockCount, Self::Error> {
        let num_blocks = self.file.borrow().metadata().unwrap().len() / 512;
        Ok(BlockCount(num_blocks))
    }
}

#[derive(Debug)]
pub struct Clock;

impl TimeSource for Clock {
    fn get_timestamp(&self) -> Timestamp {
        use chrono::Datelike;
        let local: chrono::DateTime<chrono::Local> = chrono::Local::now();
        Timestamp {
            year_since_1970: (local.year() - 1970) as u8,
            zero_indexed_month: local.month0() as u8,
            zero_indexed_day: local.day0() as u8,
            hours: local.hour() as u8,
            minutes: local.minute() as u8,
            seconds: local.second() as u8,
        }
    }
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
