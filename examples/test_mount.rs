//! # Tests the Embedded SDMMC Library
//!
//! This example should be given a file or block device as the first and only
//! argument. It will attempt to mount all four possible primary MBR
//! partitions, one at a time, prints the root directory and will print a file
//! called "README.TXT". It will then list the contents of the "TEST"
//! sub-directory.
//!
//! ```bash
//! $ cargo run --example test_mount -- /dev/mmcblk0
//! $ cargo run --example test_mount -- /dev/sda
//! ```
//!
//! If you pass a block device it should be unmounted. No testing has been
//! performed with Windows raw block devices - please report back if you try
//! this! There is a gzipped example disk image which you can gunzip and test
//! with if you don't have a suitable block device.
//!
//! ```bash
//! zcat ./disk.img.gz > ./disk.img
//! $ cargo run --example test_mount -- ./disk.img
//! ```

extern crate embedded_sdmmc;

const FILE_TO_PRINT: &'static str = "README.TXT";
const FILE_TO_CHECKSUM: &'static str = "64MB.DAT";

use embedded_sdmmc::{
    Block, BlockCount, BlockDevice, BlockIdx, Error, Mode, TimeSource, Timestamp, VolumeIdx,
    VolumeManager,
};
use std::cell::RefCell;
use std::fs::File;
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
            file: RefCell::new(File::open(device_name)?),
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
    let filename = args.next().unwrap_or("/dev/mmcblk0".into());
    let print_blocks = args.find(|x| x == "-v").map(|_| true).unwrap_or(false);
    let lbd = LinuxBlockDevice::new(filename, print_blocks)
        .map_err(Error::DeviceError)
        .unwrap();
    println!("lbd: {:?}", lbd);
    let mut volume_mgr = VolumeManager::new(lbd, Clock);
    for i in 0..=3 {
        let volume = volume_mgr.get_volume(VolumeIdx(i));
        println!("volume {}: {:#?}", i, volume);
        if let Ok(mut volume) = volume {
            let root_dir = volume_mgr.open_root_dir(&volume).unwrap();
            println!("\tListing root directory:");
            volume_mgr
                .iterate_dir(&volume, &root_dir, |x| {
                    println!("\t\tFound: {:?}", x);
                })
                .unwrap();
            println!("\tFinding {}...", FILE_TO_PRINT);
            println!(
                "\tFound {}?: {:?}",
                FILE_TO_PRINT,
                volume_mgr.find_directory_entry(&volume, &root_dir, FILE_TO_PRINT)
            );
            let mut f = volume_mgr
                .open_file_in_dir(&mut volume, &root_dir, FILE_TO_PRINT, Mode::ReadOnly)
                .unwrap();
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
            // Can't open file twice
            assert!(volume_mgr
                .open_file_in_dir(&mut volume, &root_dir, FILE_TO_PRINT, Mode::ReadOnly)
                .is_err());
            volume_mgr.close_file(&volume, f).unwrap();

            let test_dir = volume_mgr.open_dir(&volume, &root_dir, "TEST").unwrap();
            // Check we can't open it twice
            assert!(volume_mgr.open_dir(&volume, &root_dir, "TEST").is_err());
            // Print the contents
            println!("\tListing TEST directory:");
            volume_mgr
                .iterate_dir(&volume, &test_dir, |x| {
                    println!("\t\tFound: {:?}", x);
                })
                .unwrap();
            volume_mgr.close_dir(&volume, test_dir);

            // Checksum example file. We just sum the bytes, as a quick and dirty checksum.
            // We also read in a weird block size, just to exercise the offset calculation code.
            let mut f = volume_mgr
                .open_file_in_dir(&mut volume, &root_dir, FILE_TO_CHECKSUM, Mode::ReadOnly)
                .unwrap();
            println!("Checksuming {} bytes of {}", f.length(), FILE_TO_CHECKSUM);
            let mut csum = 0u32;
            while !f.eof() {
                let mut buffer = [0u8; 2047];
                let num_read = volume_mgr.read(&volume, &mut f, &mut buffer).unwrap();
                for b in &buffer[0..num_read] {
                    csum += u32::from(*b);
                }
            }
            println!("Checksum over {} bytes: {}", f.length(), csum);
            volume_mgr.close_file(&volume, f).unwrap();

            assert!(volume_mgr.open_root_dir(&volume).is_err());
            volume_mgr.close_dir(&volume, root_dir);
            assert!(volume_mgr.open_root_dir(&volume).is_ok());
        }
    }
}
