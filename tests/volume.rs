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
        .open_raw_volume(embedded_sdmmc::VolumeIdx(0))
        .expect("open volume 0");

    // Fail to Open Volume 0 again
    assert!(matches!(
        volume_mgr.open_raw_volume(embedded_sdmmc::VolumeIdx(0)),
        Err(embedded_sdmmc::Error::VolumeAlreadyOpen)
    ));

    volume_mgr.close_volume(fat16_volume).expect("close fat16");

    // Open Volume 1
    let fat32_volume = volume_mgr
        .open_raw_volume(embedded_sdmmc::VolumeIdx(1))
        .expect("open volume 1");

    // Fail to Volume 1 again
    assert!(matches!(
        volume_mgr.open_raw_volume(embedded_sdmmc::VolumeIdx(1)),
        Err(embedded_sdmmc::Error::VolumeAlreadyOpen)
    ));

    // Open Volume 0 again
    let fat16_volume = volume_mgr
        .open_raw_volume(embedded_sdmmc::VolumeIdx(0))
        .expect("open volume 0");

    // Open any volume - too many volumes (0 and 1 are open)
    assert!(matches!(
        volume_mgr.open_raw_volume(embedded_sdmmc::VolumeIdx(0)),
        Err(embedded_sdmmc::Error::TooManyOpenVolumes)
    ));

    volume_mgr.close_volume(fat16_volume).expect("close fat16");
    volume_mgr.close_volume(fat32_volume).expect("close fat32");

    // This isn't a valid volume
    assert!(matches!(
        volume_mgr.open_raw_volume(embedded_sdmmc::VolumeIdx(2)),
        Err(embedded_sdmmc::Error::FormatError(_e))
    ));

    // This isn't a valid volume
    assert!(matches!(
        volume_mgr.open_raw_volume(embedded_sdmmc::VolumeIdx(9)),
        Err(embedded_sdmmc::Error::NoSuchVolume)
    ));

    let _root_dir = volume_mgr.open_root_dir(fat32_volume).expect("Open dir");

    assert!(matches!(
        volume_mgr.close_volume(fat32_volume),
        Err(embedded_sdmmc::Error::VolumeStillInUse)
    ));
}

#[test]
fn close_volume_too_early() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut volume_mgr = embedded_sdmmc::VolumeManager::new(disk, time_source);

    let volume = volume_mgr
        .open_raw_volume(embedded_sdmmc::VolumeIdx(0))
        .expect("open volume 0");
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");

    // Dir open
    assert!(matches!(
        volume_mgr.close_volume(volume),
        Err(embedded_sdmmc::Error::VolumeStillInUse)
    ));

    let _test_file = volume_mgr
        .open_file_in_dir(root_dir, "64MB.DAT", embedded_sdmmc::Mode::ReadOnly)
        .expect("open test file");

    volume_mgr.close_dir(root_dir).unwrap();

    // File open, not dir open
    assert!(matches!(
        volume_mgr.close_volume(volume),
        Err(embedded_sdmmc::Error::VolumeStillInUse)
    ));
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
