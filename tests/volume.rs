//! Volume related tests

mod utils;

#[test]
fn open_all_volumes() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut volume_mgr: embedded_sdmmc::VolumeManager<
        utils::RamDisk<Vec<u8>>,
        utils::TestTimeSource,
        4,
        4,
        2,
    > = embedded_sdmmc::VolumeManager::new_with_limits(disk, time_source, 0x1000_0000);

    // Open Volume 0
    let fat16_volume = volume_mgr
        .open_volume(embedded_sdmmc::VolumeIdx(0))
        .expect("open volume 0");

    // Fail to Open Volume 0 again
    let r = volume_mgr.open_volume(embedded_sdmmc::VolumeIdx(0));
    let Err(embedded_sdmmc::Error::VolumeAlreadyOpen) = r else {
        panic!("Should have failed to open volume {:?}", r);
    };

    volume_mgr.close_volume(fat16_volume).expect("close fat16");

    // Open Volume 1
    let fat32_volume = volume_mgr
        .open_volume(embedded_sdmmc::VolumeIdx(1))
        .expect("open volume 1");

    // Fail to Volume 1 again
    let r = volume_mgr.open_volume(embedded_sdmmc::VolumeIdx(1));
    let Err(embedded_sdmmc::Error::VolumeAlreadyOpen) = r else {
        panic!("Should have failed to open volume {:?}", r);
    };

    // Open Volume 0 again
    let fat16_volume = volume_mgr
        .open_volume(embedded_sdmmc::VolumeIdx(0))
        .expect("open volume 0");

    // Open any volume - too many volumes (0 and 1 are open)
    let r = volume_mgr.open_volume(embedded_sdmmc::VolumeIdx(0));
    let Err(embedded_sdmmc::Error::TooManyOpenVolumes) = r else {
        panic!("Should have failed to open volume {:?}", r);
    };

    volume_mgr.close_volume(fat16_volume).expect("close fat16");
    volume_mgr.close_volume(fat32_volume).expect("close fat32");

    // This isn't a valid volume
    let r = volume_mgr.open_volume(embedded_sdmmc::VolumeIdx(2));
    let Err(embedded_sdmmc::Error::FormatError(_e)) = r else {
        panic!("Should have failed to open volume {:?}", r);
    };

    // This isn't a valid volume
    let r = volume_mgr.open_volume(embedded_sdmmc::VolumeIdx(9));
    let Err(embedded_sdmmc::Error::NoSuchVolume) = r else {
        panic!("Should have failed to open volume {:?}", r);
    };

    let _root_dir = volume_mgr.open_root_dir(fat32_volume).expect("Open dir");

    let r = volume_mgr.close_volume(fat32_volume);
    let Err(embedded_sdmmc::Error::VolumeStillInUse) = r else {
        panic!("Should have failed to close volume {:?}", r);
    };
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
