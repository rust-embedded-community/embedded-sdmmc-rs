//! File opening related tests

use embedded_sdmmc::{Mode, VolumeIdx, VolumeManager};

mod utils;

#[test]
fn append_file() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let volume_mgr: VolumeManager<utils::RamDisk<Vec<u8>>, utils::TestTimeSource, 4, 2, 1> =
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

#[test]
fn flush_file() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let volume_mgr: VolumeManager<utils::RamDisk<Vec<u8>>, utils::TestTimeSource, 4, 2, 1> =
        VolumeManager::new_with_limits(disk, time_source, 0xAA00_0000);
    let volume = volume_mgr
        .open_raw_volume(VolumeIdx(0))
        .expect("open volume");
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");

    // Open with string
    let f = volume_mgr
        .open_file_in_dir(root_dir, "README.TXT", Mode::ReadWriteTruncate)
        .expect("open file");

    // Write some data to the file
    let test_data = vec![0xCC; 64];
    volume_mgr.write(f, &test_data).expect("file write");

    // Check that the file length is zero in the directory entry, as we haven't
    // flushed yet
    let entry = volume_mgr
        .find_directory_entry(root_dir, "README.TXT")
        .expect("find entry");
    assert_eq!(entry.size, 0);

    volume_mgr.flush_file(f).expect("flush");

    // Now check the file length again after flushing
    let entry = volume_mgr
        .find_directory_entry(root_dir, "README.TXT")
        .expect("find entry");
    assert_eq!(entry.size, 64);

    // Flush more writes
    volume_mgr.write(f, &test_data).expect("file write");
    volume_mgr.write(f, &test_data).expect("file write");
    volume_mgr.flush_file(f).expect("flush");

    // Now check the file length again, again
    let entry = volume_mgr
        .find_directory_entry(root_dir, "README.TXT")
        .expect("find entry");
    assert_eq!(entry.size, 64 * 3);
}

#[test]
fn random_access_write_file() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let volume_mgr: VolumeManager<utils::RamDisk<Vec<u8>>, utils::TestTimeSource, 4, 2, 1> =
        VolumeManager::new_with_limits(disk, time_source, 0xAA00_0000);
    let volume = volume_mgr
        .open_raw_volume(VolumeIdx(0))
        .expect("open volume");
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");

    // Open with string
    let f = volume_mgr
        .open_file_in_dir(root_dir, "README.TXT", Mode::ReadWriteTruncate)
        .expect("open file");

    let test_data = vec![0xCC; 1024];
    volume_mgr.write(f, &test_data).expect("file write");

    let length = volume_mgr.file_length(f).expect("get length");
    assert_eq!(length, 1024);

    for seek_offset in [100, 0] {
        let mut expected_buffer = [0u8; 4];

        // fetch some data at offset seek_offset
        volume_mgr
            .file_seek_from_start(f, seek_offset)
            .expect("Seeking");
        volume_mgr.read(f, &mut expected_buffer).expect("read file");

        // modify first byte
        expected_buffer[0] ^= 0xff;

        // write only first byte, expecting the rest to not change
        volume_mgr
            .file_seek_from_start(f, seek_offset)
            .expect("Seeking");
        volume_mgr
            .write(f, &expected_buffer[0..1])
            .expect("file write");
        volume_mgr.flush_file(f).expect("file flush");

        // read and verify
        volume_mgr
            .file_seek_from_start(f, seek_offset)
            .expect("file seek");
        let mut read_buffer = [0xffu8, 0xff, 0xff, 0xff];
        volume_mgr.read(f, &mut read_buffer).expect("file read");
        assert_eq!(
            read_buffer, expected_buffer,
            "mismatch seek+write at offset {seek_offset} from start"
        );
    }

    volume_mgr.close_file(f).expect("close file");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
}
// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
