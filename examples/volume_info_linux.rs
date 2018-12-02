extern crate embedded_sdmmc;

use embedded_sdmmc::{
    Block, BlockDevice, BlockIdx, Controller, Error, TimeSource, Timestamp, VolumeIdx,
};
use std::cell::RefCell;
use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::Path;

#[derive(Debug)]
struct LinuxBlockDevice {
    file: RefCell<File>,
}

impl LinuxBlockDevice {
    fn new<P>(device_name: P) -> Result<LinuxBlockDevice, std::io::Error>
    where
        P: AsRef<Path>,
    {
        Ok(LinuxBlockDevice {
            file: RefCell::new(File::open(device_name)?),
        })
    }
}

impl BlockDevice for LinuxBlockDevice {
    type Error = std::io::Error;

    fn read(&self, blocks: &mut [Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        self.file
            .borrow_mut()
            .seek(SeekFrom::Start(start_block_idx.into_bytes()))?;
        for block in blocks.iter_mut() {
            self.file.borrow_mut().read_exact(&mut block.contents)?;
            println!("Read: {:?}", &block);
        }
        Ok(())
    }

    fn write(&mut self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        self.file
            .borrow_mut()
            .seek(SeekFrom::Start(start_block_idx.into_bytes()))?;
        for block in blocks.iter() {
            self.file.borrow_mut().write_all(&block.contents)?;
            // println!("Wrote: {:?}", &block);
        }
        Ok(())
    }

    fn num_blocks(&self) -> Result<BlockIdx, Self::Error> {
        let num_blocks = self.file.borrow().metadata().unwrap().len() / 512;
        Ok(BlockIdx(num_blocks as u32))
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

fn main() -> Result<(), Error<std::io::Error>> {
    let lbd = LinuxBlockDevice::new("/dev/mmcblk0").map_err(|e| Error::DeviceError(e))?;
    println!("lbd: {:?}", lbd);
    let mut controller = Controller::new(lbd, Clock);
    println!("volume 0: {:?}", controller.get_volume(VolumeIdx(0)));
    println!("volume 1: {:?}", controller.get_volume(VolumeIdx(1)));
    println!("volume 2: {:?}", controller.get_volume(VolumeIdx(2)));
    println!("volume 3: {:?}", controller.get_volume(VolumeIdx(3)));
    let volume = controller.get_volume(VolumeIdx(0)).unwrap();
    let dir = controller.open_root_dir(&volume)?;
    println!(
        "Finding TEST.TXT: {:?}",
        controller.find_directory_entry(&volume, &dir, "TEST.TXT")
    );
    println!("Listing root directory:");
    controller.iterate_dir(&volume, &dir, |x| {
        println!("Found: {:?}", x);
    })?;
    println!(
        "Finding rand_1MB.DAT: {:?}",
        controller.find_directory_entry(&volume, &dir, "rand_1MB.DAT")
    );
    assert!(controller.open_root_dir(&volume).is_err());
    controller.close_dir(&volume, dir);
    assert!(controller.open_root_dir(&volume).is_ok());
    Ok(())
}
