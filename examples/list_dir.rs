//! Recursive Directory Listing Example.
//!
//! ```bash
//! $ cargo run --example list_dir -- /dev/mmcblk0
//! Compiling embedded-sdmmc v0.5.0 (/Users/jonathan/embedded-sdmmc-rs)
//! Finished dev [unoptimized + debuginfo] target(s) in 0.20s
//!  Running `/Users/jonathan/embedded-sdmmc-rs/target/debug/examples/list_dir /dev/mmcblk0`
//! Listing /
//! README.TXT         258 2018-12-09 19:22:34
//! EMPTY.DAT            0 2018-12-09 19:21:16
//! TEST                 0 2018-12-09 19:23:16 <DIR>
//! 64MB.DAT      67108864 2018-12-09 19:21:38
//! FSEVEN~1             0 2023-09-21 11:32:04 <DIR>
//! Listing /TEST
//! .                    0 2018-12-09 19:21:02 <DIR>
//! ..                   0 2018-12-09 19:21:02 <DIR>
//! TEST.DAT          3500 2018-12-09 19:22:12
//! Listing /FSEVEN~1
//! .                    0 2023-09-21 11:32:22 <DIR>
//! ..                   0 2023-09-21 11:32:04 <DIR>
//! FSEVEN~1            36 2023-09-21 11:32:04
//! $
//! ```
//!
//! If you pass a block device it should be unmounted. No testing has been
//! performed with Windows raw block devices - please report back if you try
//! this! There is a gzipped example disk image which you can gunzip and test
//! with if you don't have a suitable block device.
//!
//! ```bash
//! zcat ./tests/disk.img.gz > ./disk.img
//! $ cargo run --example list_dir -- ./disk.img
//! ```

extern crate embedded_sdmmc;

mod linux;
use linux::*;

use embedded_sdmmc::{Directory, VolumeIdx, VolumeManager};

type Error = embedded_sdmmc::Error<std::io::Error>;

fn main() -> Result<(), Error> {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let filename = args.next().unwrap_or_else(|| "/dev/mmcblk0".into());
    let print_blocks = args.find(|x| x == "-v").map(|_| true).unwrap_or(false);
    let lbd = LinuxBlockDevice::new(filename, print_blocks).map_err(Error::DeviceError)?;
    let mut volume_mgr: VolumeManager<LinuxBlockDevice, Clock, 8, 8, 4> =
        VolumeManager::new_with_limits(lbd, Clock, 0xAA00_0000);
    let mut volume = volume_mgr.open_volume(VolumeIdx(0))?;
    let root_dir = volume.open_root_dir()?;
    list_dir(root_dir, "/")?;
    Ok(())
}

/// Recursively print a directory listing for the open directory given.
///
/// The path is for display purposes only.
fn list_dir(
    mut directory: Directory<LinuxBlockDevice, Clock, 8, 8, 4>,
    path: &str,
) -> Result<(), Error> {
    println!("Listing {}", path);
    let mut children = Vec::new();
    directory.iterate_dir(|entry| {
        println!(
            "{:12} {:9} {} {}",
            entry.name,
            entry.size,
            entry.mtime,
            if entry.attributes.is_directory() {
                "<DIR>"
            } else {
                ""
            }
        );
        if entry.attributes.is_directory()
            && entry.name != embedded_sdmmc::ShortFileName::parent_dir()
            && entry.name != embedded_sdmmc::ShortFileName::this_dir()
        {
            children.push(entry.name.clone());
        }
    })?;
    for child_name in children {
        let child_dir = directory.open_dir(&child_name)?;
        let child_path = if path == "/" {
            format!("/{}", child_name)
        } else {
            format!("{}/{}", path, child_name)
        };
        list_dir(child_dir, &child_path)?;
    }
    Ok(())
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
