//! # Tests the Embedded SDMMC Library
//! ```bash
//! $ cargo run --example write_test -- /dev/mmcblk0
//! $ cargo run --example write_test -- /dev/sda
//! ```
//!
//! If you pass a block device it should be unmounted. No testing has been
//! performed with Windows raw block devices - please report back if you try
//! this!
//!
//! ```bash
//! gunzip -kf ./disk.img.gz
//! $ cargo run --example write_test -- ./disk.img
//! ```

extern crate embedded_sdmmc;

const FILE_TO_WRITE: &str = "README.TXT";

use embedded_sdmmc::{
    Block, BlockCount, BlockDevice, BlockIdx, Controller, Error, Mode, TimeSource, Timestamp,
    VolumeIdx,
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
    println!("Opening {:?}", filename);
    let print_blocks = args.find(|x| x == "-v").map(|_| true).unwrap_or(false);
    let lbd = LinuxBlockDevice::new(filename, print_blocks)
        .map_err(Error::DeviceError)
        .unwrap();
    println!("lbd: {:?}", lbd);
    let mut controller = Controller::new(lbd, Clock);
    let volume = controller.get_volume(VolumeIdx(0));
    println!("volume 0: {:#?}", volume);
    if let Ok(mut volume) = volume {
        let root_dir = controller.open_root_dir(&volume).unwrap();
        println!("\tListing root directory:");
        controller
            .iterate_dir(&volume, &root_dir, |x| {
                println!("\t\tFound: {:?}", x);
            })
            .unwrap();

        // This will panic if the file doesn't exist, use ReadWriteCreateOrTruncate or
        // ReadWriteCreateOrAppend instead. ReadWriteCreate also creates a file, but it returns an
        // error if the file already exists
        let mut f = controller
            .open_file_in_dir(&mut volume, &root_dir, FILE_TO_WRITE, Mode::ReadOnly)
            .unwrap();
        println!("\nReading from file {}\n", FILE_TO_WRITE);
        println!("FILE STARTS:");
        while !f.eof() {
            let mut buffer = [0u8; 32];
            let num_read = controller.read(&volume, &mut f, &mut buffer).unwrap();
            for b in &buffer[0..num_read] {
                if *b == 10 {
                    print!("\\n");
                }
                print!("{}", *b as char);
            }
        }
        println!("EOF\n");
        controller.close_file(&volume, f).unwrap();

        let mut f = controller
            .open_file_in_dir(&mut volume, &root_dir, FILE_TO_WRITE, Mode::ReadWriteAppend)
            .unwrap();

        let buffer1 = b"\nFile Appended\n";
        let buffer = [b'a'; 8192];
        println!("\nAppending to file");
        let num_written1 = controller.write(&mut volume, &mut f, &buffer1[..]).unwrap();
        let num_written = controller.write(&mut volume, &mut f, &buffer[..]).unwrap();
        println!("Number of bytes written: {}\n", num_written + num_written1);

        f.seek_from_start(0).unwrap();
        println!("\tFinding {}...", FILE_TO_WRITE);
        println!(
            "\tFound {}?: {:?}",
            FILE_TO_WRITE,
            controller.find_directory_entry(&volume, &root_dir, FILE_TO_WRITE)
        );
        println!("\nFILE STARTS:");
        while !f.eof() {
            let mut buffer = [0u8; 32];
            let num_read = controller.read(&volume, &mut f, &mut buffer).unwrap();
            for b in &buffer[0..num_read] {
                if *b == 10 {
                    print!("\\n");
                }
                print!("{}", *b as char);
            }
        }
        println!("EOF");
        controller.close_file(&volume, f).unwrap();

        println!("\nTruncating file");
        let mut f = controller
            .open_file_in_dir(
                &mut volume,
                &root_dir,
                FILE_TO_WRITE,
                Mode::ReadWriteTruncate,
            )
            .unwrap();

        let buffer = b"Hello\n";
        let num_written = controller.write(&mut volume, &mut f, &buffer[..]).unwrap();
        println!("\nNumber of bytes written: {}\n", num_written);

        println!("\tFinding {}...", FILE_TO_WRITE);
        println!(
            "\tFound {}?: {:?}",
            FILE_TO_WRITE,
            controller.find_directory_entry(&volume, &root_dir, FILE_TO_WRITE)
        );
        f.seek_from_start(0).unwrap();
        println!("\nFILE STARTS:");
        while !f.eof() {
            let mut buffer = [0u8; 32];
            let num_read = controller.read(&volume, &mut f, &mut buffer).unwrap();
            for b in &buffer[0..num_read] {
                if *b == 10 {
                    print!("\\n");
                }
                print!("{}", *b as char);
            }
        }
        println!("EOF");
        controller.close_file(&volume, f).unwrap();
    }
}
