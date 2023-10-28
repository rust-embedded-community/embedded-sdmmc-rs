//! Delete File Example.
//!
//! ```bash
//! $ cargo run --example delete_file -- ./disk.img
//! $ cargo run --example delete_file -- /dev/mmcblk0
//! ```
//!
//! NOTE: THIS EXAMPLE DELETES A FILE CALLED README.TXT. IF YOU DO NOT WANT THAT
//! FILE DELETED FROM YOUR DISK IMAGE, DO NOT RUN THIS EXAMPLE.
//!
//! If you pass a block device it should be unmounted. No testing has been
//! performed with Windows raw block devices - please report back if you try
//! this! There is a gzipped example disk image which you can gunzip and test
//! with if you don't have a suitable block device.
//!
//! ```bash
//! zcat ./tests/disk.img.gz > ./disk.img
//! $ cargo run --example delete_file -- ./disk.img
//! ```

extern crate embedded_sdmmc;

mod linux;
use linux::*;

const FILE_TO_DELETE: &str = "README.TXT";

use embedded_sdmmc::{Error, VolumeIdx, VolumeManager};

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
    println!("Deleting file {}...", FILE_TO_DELETE);
    root_dir.delete_file_in_dir(FILE_TO_DELETE)?;
    println!("Deleted!");
    Ok(())
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
