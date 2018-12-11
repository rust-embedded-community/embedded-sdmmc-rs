extern crate embedded_sdmmc;

use embedded_sdmmc::{
    Block, BlockCount, BlockDevice, BlockIdx, Controller, Error, Mode, TimeSource, Timestamp,
    VolumeIdx,
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

    fn write(&mut self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
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

fn main() -> Result<(), Error<std::io::Error>> {
    let mut args = std::env::args().skip(1);
    let filename = args.next().unwrap_or("/dev/mmcblk0".into());
    let print_blocks = args.find(|x| x == "-v").map(|_| true).unwrap_or(false);
    let lbd = LinuxBlockDevice::new(filename, print_blocks).map_err(Error::DeviceError)?;
    println!("lbd: {:?}", lbd);
    let mut controller = Controller::new(lbd, Clock);
    for i in 0..3 {
        let volume = controller.get_volume(VolumeIdx(i));
        println!("volume {}: {:#?}", i, volume);
        if let Ok(volume) = volume {
            let dir = controller.open_root_dir(&volume)?;
            println!("\tListing root directory:");
            controller.iterate_dir(&volume, &dir, |x| {
                println!("\t\tFound: {:?}", x);
            })?;
            println!("\tFinding README.TXT...");
            println!(
                "\tFound README.TXT?: {:?}",
                controller.find_directory_entry(&volume, &dir, "README.TXT")
            );
            let mut f = controller.open_file_in_dir(&volume, &dir, "README.TXT", Mode::ReadOnly)?;
            println!("FILE STARTS:");
            while !f.eof() {
                let mut buffer = [0u8; 32];
                let num_read = controller.read(&volume, &mut f, &mut buffer)?;
                for b in &buffer[0..num_read] {
                    if *b == 10 {
                        print!("\\n");
                    }
                    print!("{}", *b as char);
                }
            }
            println!("EOF");
            controller.close_file(&volume, f)?;
            assert!(controller.open_root_dir(&volume).is_err());
            controller.close_dir(&volume, dir);
            assert!(controller.open_root_dir(&volume).is_ok());
        }
    }
    Ok(())
}
