//! File opening related tests

use embedded_sdmmc::{Mode, VolumeIdx, VolumeManager};

mod utils;

#[test]
fn append_file() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut volume_mgr: VolumeManager<utils::RamDisk<Vec<u8>>, utils::TestTimeSource, 4, 2, 1> =
        VolumeManager::new_with_limits(disk, time_source, 0xAA00_0000);
    let volume = volume_mgr
        .open_raw_volume(VolumeIdx(0))
        .expect("open volume");
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");

    // Open with string
    let f = volume_mgr
        .open_file_in_dir(root_dir, "README.TXT", Mode::ReadWriteTruncate)
        .expect("open file");

    // Should be enough to cause a few more clusters to be allocated
    let test_data = vec![0xCC; 1024 * 1024];
    volume_mgr.write(f, &test_data).expect("file write");

    let length = volume_mgr.file_length(f).expect("get length");
    assert_eq!(length, 1024 * 1024);

    let offset = volume_mgr.file_offset(f).expect("offset");
    assert_eq!(offset, 1024 * 1024);

    // Now wind it back 1 byte;
    volume_mgr.file_seek_from_current(f, -1).expect("Seeking");

    let offset = volume_mgr.file_offset(f).expect("offset");
    assert_eq!(offset, (1024 * 1024) - 1);

    // Write another megabyte, making `2 MiB - 1`
    volume_mgr.write(f, &test_data).expect("file write");

    let length = volume_mgr.file_length(f).expect("get length");
    assert_eq!(length, (1024 * 1024 * 2) - 1);

    volume_mgr.close_file(f).expect("close dir");

    // Now check the file length again

    let entry = volume_mgr
        .find_directory_entry(root_dir, "README.TXT")
        .expect("Find entry");
    assert_eq!(entry.size, (1024 * 1024 * 2) - 1);

    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
