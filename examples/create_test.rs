//! # Tests the Embedded SDMMC Library
//! ```bash
//! $ cargo run --example create_test -- /dev/mmcblk0
//! $ cargo run --example create_test -- /dev/sda
//! ```
//!
//! If you pass a block device it should be unmounted. No testing has been
//! performed with Windows raw block devices - please report back if you try
//! this! There is a gzipped example disk image which you can gunzip and test
//! with if you don't have a suitable block device.
//!
//! ```bash
//! zcat ./disk.img.gz > ./disk.img
//! $ cargo run --example create_test -- ./disk.img
//! ```

extern crate embedded_sdmmc;

const FILE_TO_CREATE: &'static str = "CREATE.TXT";

use embedded_sdmmc::{
    Block, BlockCount, BlockDevice, BlockIdx, Error, Mode, TimeSource, Timestamp, VolumeIdx,
    VolumeManager,
};
use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::Path;

#[derive(Debug)]
struct LinuxBlockDevice {
    file: RefCell<File>,
    print_blocks: bool,
}

impl LinuxBlockDevice {
    fn new<P>(device_name: P, print_blocks: bool) -> Result<LinuxBlockDevice, std::io::Error>
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
        &self,
        blocks: &mut [Block],
        start_block_idx: BlockIdx,
        reason: &str,
    ) -> Result<(), Self::Error> {
        self.file
            .borrow_mut()
            .seek(SeekFrom::Start(start_block_idx.into_bytes()))?;
        for block in blocks.iter_mut() {
            self.file.borrow_mut().read_exact(&mut block.contents)?;
            if self.print_blocks {
                println!(
                    "Read block ({}) {:?}: {:?}",
                    reason, start_block_idx, &block
                );
            }
        }
        Ok(())
    }

    fn write(&self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        self.file
            .borrow_mut()
            .seek(SeekFrom::Start(start_block_idx.into_bytes()))?;
        for block in blocks.iter() {
            self.file.borrow_mut().write_all(&block.contents)?;
            if self.print_blocks {
                println!("Wrote: {:?}", &block);
            }
        }
        Ok(())
    }

    fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
        let num_blocks = self.file.borrow().metadata().unwrap().len() / 512;
        Ok(BlockCount(num_blocks as u32))
    }
}

struct Clock;

impl TimeSource for Clock {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

fn main() {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let filename = args.next().unwrap_or_else(|| "/dev/mmcblk0".into());
    let print_blocks = args.find(|x| x == "-v").map(|_| true).unwrap_or(false);
    let lbd = LinuxBlockDevice::new(filename, print_blocks)
        .map_err(Error::DeviceError)
        .unwrap();
    println!("lbd: {:?}", lbd);
    let mut volume_mgr = VolumeManager::new(lbd, Clock);
    for volume_idx in 0..=3 {
        let volume = volume_mgr.get_volume(VolumeIdx(volume_idx));
        println!("volume {}: {:#?}", volume_idx, volume);
        if let Ok(mut volume) = volume {
            let root_dir = volume_mgr.open_root_dir(&volume).unwrap();
            println!("\tListing root directory:");
            volume_mgr
                .iterate_dir(&volume, &root_dir, |x| {
                    println!("\t\tFound: {:?}", x);
                })
                .unwrap();
            println!("\nCreating file {}...", FILE_TO_CREATE);
            // This will panic if the file already exists, use ReadWriteCreateOrAppend or
            // ReadWriteCreateOrTruncate instead
            let mut f = volume_mgr
                .open_file_in_dir(
                    &mut volume,
                    &root_dir,
                    FILE_TO_CREATE,
                    Mode::ReadWriteCreate,
                )
                .unwrap();
            println!("\nReading from file");
            println!("FILE STARTS:");
            while !f.eof() {
                let mut buffer = [0u8; 32];
                let num_read = volume_mgr.read(&volume, &mut f, &mut buffer).unwrap();
                for b in &buffer[0..num_read] {
                    if *b == 10 {
                        print!("\\n");
                    }
                    print!("{}", *b as char);
                }
            }
            println!("EOF");

            let buffer1 = b"\nFile Appended\n";
            let mut buffer: Vec<u8> = vec![];
            for _ in 0..64 {
                for _ in 0..15 {
                    buffer.push(b'a');
                }
                buffer.push(b'\n');
            }
            println!("\nAppending to file");
            let num_written1 = volume_mgr.write(&mut volume, &mut f, &buffer1[..]).unwrap();
            let num_written = volume_mgr.write(&mut volume, &mut f, &buffer[..]).unwrap();
            println!("Number of bytes written: {}\n", num_written + num_written1);
            volume_mgr.close_file(&volume, f).unwrap();

            let mut f = volume_mgr
                .open_file_in_dir(
                    &mut volume,
                    &root_dir,
                    FILE_TO_CREATE,
                    Mode::ReadWriteCreateOrAppend,
                )
                .unwrap();
            f.seek_from_start(0).unwrap();

            println!("\tFinding {}...", FILE_TO_CREATE);
            println!(
                "\tFound {}?: {:?}",
                FILE_TO_CREATE,
                volume_mgr.find_directory_entry(&volume, &root_dir, FILE_TO_CREATE)
            );
            println!("\nReading from file");
            println!("FILE STARTS:");
            while !f.eof() {
                let mut buffer = [0u8; 32];
                let num_read = volume_mgr.read(&volume, &mut f, &mut buffer).unwrap();
                for b in &buffer[0..num_read] {
                    if *b == 10 {
                        print!("\\n");
                    }
                    print!("{}", *b as char);
                }
            }
            println!("EOF");
            volume_mgr.close_file(&volume, f).unwrap();
        }
    }
}
