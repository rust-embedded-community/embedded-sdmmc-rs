//! Create File Example.
//!
//! ```bash
//! $ cargo run --example create_file -- ./disk.img
//! $ cargo run --example create_file -- /dev/mmcblk0
//! ```
//!
//! If you pass a block device it should be unmounted. There is a gzipped
//! example disk image which you can gunzip and test with if you don't have a
//! suitable block device.
//!
//! ```bash
//! zcat ./tests/disk.img.gz > ./disk.img
//! $ cargo run --example create_file -- ./disk.img
//! ```

mod linux;
use linux::*;

const FILE_TO_CREATE: &str = "CREATE.TXT";

use embedded_sdmmc::{Error, Mode, VolumeIdx};

type VolumeManager = embedded_sdmmc::VolumeManager<LinuxBlockDevice, Clock, 8, 4, 4>;

fn main() -> Result<(), Error<std::io::Error>> {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let filename = args.next().unwrap_or_else(|| "/dev/mmcblk0".into());
    let print_blocks = args.find(|x| x == "-v").map(|_| true).unwrap_or(false);
    let lbd = LinuxBlockDevice::new(filename, print_blocks).map_err(Error::DeviceError)?;
    let volume_mgr: VolumeManager = VolumeManager::new_with_limits(lbd, Clock, 0xAA00_0000);
    let volume = volume_mgr.open_volume(VolumeIdx(0))?;
    let root_dir = volume.open_root_dir()?;
    println!("\nCreating file {}...", FILE_TO_CREATE);
    // This will panic if the file already exists: use ReadWriteCreateOrAppend
    // or ReadWriteCreateOrTruncate instead if you want to modify an existing
    // file.
    let f = root_dir.open_file_in_dir(FILE_TO_CREATE, Mode::ReadWriteCreate)?;
    f.write(b"Hello, this is a new file on disk\r\n")?;
    Ok(())
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
