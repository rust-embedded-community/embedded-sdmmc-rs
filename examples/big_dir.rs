//! Big Directory Example.
//!
//! Attempts to create an infinite number of files in the root directory of the
//! first volume of the given block device. This is basically to see what
//! happens when the root directory runs out of space.
//!
//! ```bash
//! $ cargo run --example big_dir -- ./disk.img
//! $ cargo run --example big_dir -- /dev/mmcblk0
//! ```
//!
//! If you pass a block device it should be unmounted. There is a gzipped
//! example disk image which you can gunzip and test with if you don't have a
//! suitable block device.
//!
//! ```bash
//! zcat ./tests/disk.img.gz > ./disk.img
//! $ cargo run --example big_dir -- ./disk.img
//! ```

mod linux;
use linux::*;

use embedded_sdmmc::{Error, Mode, VolumeIdx};

type VolumeManager = embedded_sdmmc::VolumeManager<LinuxBlockDevice, Clock, 8, 4, 4>;

fn main() -> Result<(), Error<std::io::Error>> {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let filename = args.next().unwrap_or_else(|| "/dev/mmcblk0".into());
    let print_blocks = args.find(|x| x == "-v").map(|_| true).unwrap_or(false);
    let lbd = LinuxBlockDevice::new(filename, print_blocks).map_err(Error::DeviceError)?;
    let volume_mgr: VolumeManager = VolumeManager::new_with_limits(lbd, Clock, 0xAA00_0000);
    let volume = volume_mgr.open_volume(VolumeIdx(0)).unwrap();
    println!("Volume: {:?}", volume);
    let root_dir = volume.open_root_dir().unwrap();

    let mut file_num = 0;
    loop {
        file_num += 1;
        let file_name = format!("{}.da", file_num);
        println!("opening file {file_name} for writing");
        let file = root_dir
            .open_file_in_dir(file_name.as_str(), Mode::ReadWriteCreateOrTruncate)
            .unwrap();
        let buf = b"hello world, from rust";
        println!("writing to file");
        file.write(&buf[..]).unwrap();
        println!("closing file");
        drop(file);
    }
}
