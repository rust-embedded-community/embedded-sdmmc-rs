extern crate embedded_sdmmc;

use embedded_sdmmc::{Block, BlockDevice, BlockIdx, Controller, Error};
use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::Path;

#[derive(Debug)]
struct LinuxBlockDevice {
    file: File,
}

impl LinuxBlockDevice {
    fn new<P>(device_name: P) -> Result<LinuxBlockDevice, std::io::Error>
    where
        P: AsRef<Path>,
    {
        Ok(LinuxBlockDevice {
            file: File::open(device_name)?,
        })
    }
}

impl BlockDevice for LinuxBlockDevice {
    type Error = std::io::Error;

    fn read(&mut self, blocks: &mut [Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        self.file
            .seek(SeekFrom::Start(start_block_idx.into_bytes()))?;
        for block in blocks.iter_mut() {
            self.file.read_exact(&mut block.contents)?;
            println!("Read: {:?}", &block);
        }
        Ok(())
    }

    fn write(&mut self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        self.file
            .seek(SeekFrom::Start(start_block_idx.into_bytes()))?;
        for block in blocks.iter() {
            self.file.write_all(&block.contents)?;
            println!("Wrote: {:?}", &block);
        }
        Ok(())
    }
}

fn main() -> Result<(), Error<std::io::Error>> {
    let lbd = LinuxBlockDevice::new("/dev/mmcblk0").map_err(|e| Error::DeviceError(e))?;
    println!("lbd: {:?}", lbd);
    let mut controller = Controller::new(lbd);
    println!("volume 0: {:?}", controller.get_volume(0));
    println!("volume 1: {:?}", controller.get_volume(1));
    println!("volume 2: {:?}", controller.get_volume(2));
    println!("volume 3: {:?}", controller.get_volume(3));
    Ok(())
}
