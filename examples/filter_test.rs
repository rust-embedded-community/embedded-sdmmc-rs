//! # Tests the Embedded SDMMC Library
//! ```bash
//! $ cargo run --example filter_test -- /dev/mmcblk0
//! $ cargo run --example filter_test -- /dev/sda
//! ```
//!
//! If you pass a block device it should be unmounted. No testing has been
//! performed with Windows raw block devices - please report back if you try
//! this! There is a gzipped example disk image which you can gunzip and test
//! with if you don't have a suitable block device.
//!
//! ```bash
//! zcat ./disk.img.gz > ./disk.img
//! $ cargo run --example filter_test -- ./disk.img
//! ```

extern crate embedded_sdmmc;

const TEST_FILE_1_TO_CREATE: &'static str = "CREATE1.TST";
const TEST_FILE_2_TO_CREATE: &'static str = "CREATE2.TST";
const MARKDOWN_FILE_1_TO_CREATE: &'static str = "CREATE3.MD";
const RS_FILE_1_TO_CREATE: &'static str = "CREATE4.RS";

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
    let filename = args.next().unwrap_or_else(|| "/dev/sdb".into());
    let print_blocks = args.find(|x| x == "-v").map(|_| true).unwrap_or(false);
    let lbd = LinuxBlockDevice::new(filename, print_blocks)
        .map_err(Error::DeviceError)
        .unwrap();
    println!("lbd: {:?}", lbd);
    let mut volume_mgr = VolumeManager::new(lbd, Clock);
    let mut file_names: Vec<String> = vec![];

    for volume_idx in 0..=3 {
        let volume = volume_mgr.get_volume(VolumeIdx(volume_idx));
        println!("volume {}: {:#?}", volume_idx, volume);
        if let Ok(mut volume) = volume {
            let root_dir = volume_mgr.open_root_dir(&volume).unwrap();
            println!("\tListing root directory:");

            let f1 = volume_mgr
                .open_file_in_dir(
                    &mut volume,
                    &root_dir,
                    TEST_FILE_1_TO_CREATE,
                    Mode::ReadWriteCreate,
                )
                .unwrap();
            let f2 = volume_mgr
                .open_file_in_dir(
                    &mut volume,
                    &root_dir,
                    TEST_FILE_2_TO_CREATE,
                    Mode::ReadWriteCreate,
                )
                .unwrap();
            let f3 = volume_mgr
                .open_file_in_dir(
                    &mut volume,
                    &root_dir,
                    MARKDOWN_FILE_1_TO_CREATE,
                    Mode::ReadWriteCreate,
                )
                .unwrap();
            let f4 = volume_mgr
                .open_file_in_dir(
                    &mut volume,
                    &root_dir,
                    RS_FILE_1_TO_CREATE,
                    Mode::ReadWriteCreate,
                )
                .unwrap();

            volume_mgr
                .extention_filtered_iterate_dir(&volume, &root_dir, "TST", |file| {
                    file_names.push(file.name.to_string())
                })
                .unwrap();

            volume_mgr.close_file(&volume, f1).unwrap();
            volume_mgr.close_file(&volume, f2).unwrap();
            volume_mgr.close_file(&volume, f3).unwrap();
            volume_mgr.close_file(&volume, f4).unwrap();
            volume_mgr
                .delete_file_in_dir(&mut volume, &root_dir, TEST_FILE_1_TO_CREATE)
                .unwrap();
            volume_mgr
                .delete_file_in_dir(&mut volume, &root_dir, TEST_FILE_2_TO_CREATE)
                .unwrap();
            volume_mgr
                .delete_file_in_dir(&mut volume, &root_dir, MARKDOWN_FILE_1_TO_CREATE)
                .unwrap();
            volume_mgr
                .delete_file_in_dir(&mut volume, &root_dir, RS_FILE_1_TO_CREATE)
                .unwrap();
        }
    }

    assert_eq!(file_names, &[TEST_FILE_1_TO_CREATE, TEST_FILE_2_TO_CREATE])
}
