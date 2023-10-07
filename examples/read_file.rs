//! Read File Example.
//!
//! ```bash
//! $ cargo run --example read_file -- ./disk.img
//! Reading file README.TXT...
//! 00000000 [54, 68, 69, 73, 20, 69, 73, 20, 61, 20, 46, 41, 54, 31, 36, 20] |This.is.a.FAT16.|
//! 00000010 [70, 61, 74, 69, 74, 69, 6f, 6e, 2e, 20, 49, 74, 20, 63, 6f, 6e] |patition..It.con|
//! 00000020 [74, 61, 69, 6e, 73, 20, 66, 6f, 75, 72, 20, 66, 69, 6c, 65, 73] |tains.four.files|
//! 00000030 [20, 61, 6e, 64, 20, 61, 20, 64, 69, 72, 65, 63, 74, 6f, 72, 79] |.and.a.directory|
//! 00000040 [2e, 0a, 0a, 2a, 20, 54, 68, 69, 73, 20, 66, 69, 6c, 65, 20, 28] |...*.This.file.(|
//! 00000050 [52, 45, 41, 44, 4d, 45, 2e, 54, 58, 54, 29, 0a, 2a, 20, 41, 20] |README.TXT).*.A.|
//! 00000060 [36, 34, 20, 4d, 69, 42, 20, 66, 69, 6c, 65, 20, 66, 75, 6c, 6c] |64.MiB.file.full|
//! 00000070 [20, 6f, 66, 20, 7a, 65, 72, 6f, 73, 20, 28, 36, 34, 4d, 42, 2e] |.of.zeros.(64MB.|
//! 00000080 [44, 41, 54, 29, 2e, 0a, 2a, 20, 41, 20, 33, 35, 30, 30, 20, 62] |DAT)..*.A.3500.b|
//! 00000090 [79, 74, 65, 20, 66, 69, 6c, 65, 20, 66, 75, 6c, 6c, 20, 6f, 66] |yte.file.full.of|
//! 000000a0 [20, 72, 61, 6e, 64, 6f, 6d, 20, 64, 61, 74, 61, 2e, 0a, 2a, 20] |.random.data..*.|
//! 000000b0 [41, 20, 64, 69, 72, 65, 63, 74, 6f, 72, 79, 20, 63, 61, 6c, 6c] |A.directory.call|
//! 000000c0 [65, 64, 20, 54, 45, 53, 54, 0a, 2a, 20, 41, 20, 7a, 65, 72, 6f] |ed.TEST.*.A.zero|
//! 000000d0 [20, 62, 79, 74, 65, 20, 66, 69, 6c, 65, 20, 69, 6e, 20, 74, 68] |.byte.file.in.th|
//! 000000e0 [65, 20, 54, 45, 53, 54, 20, 64, 69, 72, 65, 63, 74, 6f, 72, 79] |e.TEST.directory|
//! 000000f0 [20, 63, 61, 6c, 6c, 65, 64, 20, 45, 4d, 50, 54, 59, 2e, 44, 41] |.called.EMPTY.DA|
//! 00000100 [54, 0a, 0d]                                                     |T...............|
//! ```
//!
//! If you pass a block device it should be unmounted. No testing has been
//! performed with Windows raw block devices - please report back if you try
//! this! There is a gzipped example disk image which you can gunzip and test
//! with if you don't have a suitable block device.
//!
//! ```bash
//! zcat ./tests/disk.img.gz > ./disk.img
//! $ cargo run --example read_file -- ./disk.img
//! ```

extern crate embedded_sdmmc;

mod linux;
use linux::*;

const FILE_TO_READ: &str = "README.TXT";

use embedded_sdmmc::{Error, Mode, VolumeIdx, VolumeManager};

fn main() -> Result<(), embedded_sdmmc::Error<std::io::Error>> {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let filename = args.next().unwrap_or_else(|| "/dev/mmcblk0".into());
    let print_blocks = args.find(|x| x == "-v").map(|_| true).unwrap_or(false);
    let lbd = LinuxBlockDevice::new(filename, print_blocks).map_err(Error::DeviceError)?;
    let mut volume_mgr: VolumeManager<LinuxBlockDevice, Clock, 8, 8, 4> =
        VolumeManager::new_with_limits(lbd, Clock, 0xAA00_0000);
    let mut volume = volume_mgr.open_volume(VolumeIdx(0))?;
    let mut root_dir = volume.open_root_dir()?;
    println!("\nReading file {}...", FILE_TO_READ);
    let mut f = root_dir.open_file_in_dir(FILE_TO_READ, Mode::ReadOnly)?;
    while !f.is_eof() {
        let mut buffer = [0u8; 16];
        let offset = f.offset();
        let mut len = f.read(&mut buffer)?;
        print!("{:08x} {:02x?}", offset, &buffer[0..len]);
        while len < buffer.len() {
            print!("    ");
            len += 1;
        }
        print!(" |");
        for b in buffer.iter() {
            let ch = char::from(*b);
            if ch.is_ascii_graphic() {
                print!("{}", ch);
            } else {
                print!(".");
            }
        }
        println!("|");
    }
    Ok(())
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
