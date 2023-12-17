extern crate embedded_sdmmc;

mod linux;
use linux::*;

use embedded_sdmmc::{Error, VolumeManager};

fn main() -> Result<(), embedded_sdmmc::Error<std::io::Error>> {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let filename = args.next().unwrap_or_else(|| "/dev/mmcblk0".into());
    let print_blocks = args.find(|x| x == "-v").map(|_| true).unwrap_or(false);
    let lbd = LinuxBlockDevice::new(filename, print_blocks).map_err(Error::DeviceError)?;
    let mut volume_mgr: VolumeManager<LinuxBlockDevice, Clock, 8, 8, 4> =
        VolumeManager::new_with_limits(lbd, Clock, 0xAA00_0000);
    let mut volume = volume_mgr
        .open_volume(embedded_sdmmc::VolumeIdx(1))
        .unwrap();
    println!("Volume: {:?}", volume);
    let mut root_dir = volume.open_root_dir().unwrap();

    let mut file_num = 0;
    loop {
        file_num += 1;
        let file_name = format!("{}.da", file_num);
        println!("opening file {file_name} for writing");
        let mut file = root_dir
            .open_file_in_dir(
                file_name.as_str(),
                embedded_sdmmc::Mode::ReadWriteCreateOrTruncate,
            )
            .unwrap();
        let buf = b"hello world, from rust";
        println!("writing to file");
        file.write(&buf[..]).unwrap();
        println!("closing file");
        drop(file);
    }
}
