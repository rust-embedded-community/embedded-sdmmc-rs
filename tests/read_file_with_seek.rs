mod utils;

static FILE_TO_READ: &str = "64MB.DAT";

#[test]
fn read_file_with_seek() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut volume_mgr = embedded_sdmmc::VolumeManager::new(disk, time_source);

    let mut volume = volume_mgr
        .open_volume(embedded_sdmmc::VolumeIdx(0))
        .unwrap();
    let mut root_dir = volume.open_root_dir().unwrap();
    println!("\nReading file {}...", FILE_TO_READ);
    let mut f = root_dir
        .open_file_in_dir(FILE_TO_READ, embedded_sdmmc::Mode::ReadOnly)
        .unwrap();
    f.seek_from_start(0x2c).unwrap();
    while f.offset() < 1000000 {
        let mut buffer = [0u8; 2048];
        f.read(&mut buffer).unwrap();
        f.seek_from_current(-1024).unwrap();
    }
    println!("Done!");
}
