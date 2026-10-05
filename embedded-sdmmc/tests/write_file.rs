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

#[test]
fn full_volume_stays_inside_its_partition() {
    use embedded_sdmmc::{Block, BlockDevice, BlockIdx};
    // The FAT16 partition is followed by the FAT32 partition, whose boot
    // sector is at block 264192.
    const NEXT_PARTITION: BlockIdx = BlockIdx(264192);
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut before = [Block::new()];
    disk.read(&mut before, NEXT_PARTITION).unwrap();
    let volume_mgr: VolumeManager<utils::RamDisk<Vec<u8>>, utils::TestTimeSource, 4, 2, 1> =
        VolumeManager::new_with_limits(disk, time_source, 0xAA00_0000);
    let volume = volume_mgr
        .open_raw_volume(VolumeIdx(0))
        .expect("open volume");
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let f = volume_mgr
        .open_file_in_dir(root_dir, "FULL.DAT", Mode::ReadWriteCreateOrTruncate)
        .expect("open file");
    // About 64 MiB is free: write until the volume is full
    let chunk = vec![0xCC; 1024 * 1024];
    let mut written = 0;
    while volume_mgr.write(f, &chunk).is_ok() {
        written += 1;
        assert!(written < 100, "the volume never filled");
    }
    assert!(matches!(
        volume_mgr.write(f, &chunk),
        Err(embedded_sdmmc::Error::DiskFull)
    ));
    volume_mgr.close_file(f).expect("close file");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let mut after = [Block::new()];
    volume_mgr
        .free()
        .0
        .read(&mut after, NEXT_PARTITION)
        .unwrap();
    assert!(
        after[0].contents == before[0].contents,
        "the next partition's boot sector was overwritten"
    );
}

#[test]
fn full_volume_uses_its_last_cluster() {
    use embedded_sdmmc::{Block, BlockDevice, BlockIdx};
    // The FAT32 partition's FSInfo sector, with its free cluster count at
    // byte 488 (accurate on the test disk)
    const FSINFO: BlockIdx = BlockIdx(264192 + 1);
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut block = [Block::new()];
    disk.read(&mut block, FSINFO).unwrap();
    let free = u32::from_le_bytes(block[0].contents[488..492].try_into().unwrap());
    let volume_mgr: VolumeManager<utils::RamDisk<Vec<u8>>, utils::TestTimeSource, 4, 2, 1> =
        VolumeManager::new_with_limits(disk, utils::make_time_source(), 0xAA00_0000);
    let volume = volume_mgr
        .open_raw_volume(VolumeIdx(1))
        .expect("open volume");
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let f = volume_mgr
        .open_file_in_dir(root_dir, "FULL.DAT", Mode::ReadWriteCreateOrTruncate)
        .expect("open file");
    // One 4 KiB cluster per write, until the volume is full
    let cluster = vec![0xCC; 4096];
    let mut written = 0;
    while volume_mgr.write(f, &cluster).is_ok() {
        written += 1;
    }
    let length = volume_mgr.file_length(f).expect("file length");
    volume_mgr.close_file(f).expect("close file");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    // Every free cluster was used, and none is counted as free
    assert_eq!(written, free);
    assert_eq!(length, free * 4096);
    let disk = volume_mgr.free().0;
    disk.read(&mut block, FSINFO).unwrap();
    assert_eq!(block[0].contents[488..492], 0u32.to_le_bytes());
}

/// The FAT16 partition starts at block 2048.
const FAT16_START: u32 = 2048;
/// The FAT32 partition starts at block 264192; its FSInfo sector is the next
/// one, with the free cluster count at byte 488.
const FAT32_FSINFO: embedded_sdmmc::BlockIdx = embedded_sdmmc::BlockIdx(264192 + 1);

type TestVolumeManager = VolumeManager<utils::RamDisk<Vec<u8>>, utils::TestTimeSource, 4, 2, 1>;

fn make_volume_manager() -> TestVolumeManager {
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    VolumeManager::new_with_limits(disk, utils::make_time_source(), 0xAA00_0000)
}

/// The FAT32 partition's free cluster count, as saved on the disk
fn saved_free_count(disk: &utils::RamDisk<Vec<u8>>) -> u32 {
    use embedded_sdmmc::{Block, BlockDevice};
    let mut block = [Block::new()];
    disk.read(&mut block, FAT32_FSINFO).unwrap();
    u32::from_le_bytes(block[0].contents[488..492].try_into().unwrap())
}

/// The block holding the FAT16 partition's FAT entry for `cluster`, and the
/// entry's offset in it
fn fat16_entry(disk: &utils::RamDisk<Vec<u8>>, cluster: u32) -> (embedded_sdmmc::BlockIdx, usize) {
    use embedded_sdmmc::{Block, BlockDevice, BlockIdx};
    let mut boot = [Block::new()];
    disk.read(&mut boot, BlockIdx(FAT16_START)).unwrap();
    let reserved = u32::from(u16::from_le_bytes([
        boot[0].contents[14],
        boot[0].contents[15],
    ]));
    let fat_start = FAT16_START + reserved;
    (
        BlockIdx(fat_start + cluster * 2 / 512),
        (cluster * 2 % 512) as usize,
    )
}

/// Write a file of `len` bytes to the FAT16 partition's root directory and
/// close the volume. Returns the disk and the file's directory entry.
fn fat16_file(name: &str, len: usize) -> (utils::RamDisk<Vec<u8>>, embedded_sdmmc::DirEntry) {
    let volume_mgr = make_volume_manager();
    let volume = volume_mgr
        .open_raw_volume(VolumeIdx(0))
        .expect("open volume");
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let f = volume_mgr
        .open_file_in_dir(root_dir, name, Mode::ReadWriteCreateOrTruncate)
        .expect("open file");
    volume_mgr.write(f, &vec![0xCC; len]).expect("file write");
    volume_mgr.close_file(f).expect("close file");
    let entry = volume_mgr
        .find_directory_entry(root_dir, name)
        .expect("find entry");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    (volume_mgr.free().0, entry)
}

/// The FAT16 partition's FAT[0] and FAT[1] (the media descriptor and the
/// end-of-chain marker)
fn fat16_reserved_entries(disk: &utils::RamDisk<Vec<u8>>) -> [u8; 4] {
    use embedded_sdmmc::{Block, BlockDevice};
    let (fat_block, _) = fat16_entry(disk, 0);
    let mut block = [Block::new()];
    disk.read(&mut block, fat_block).unwrap();
    block[0].contents[0..4].try_into().unwrap()
}

/// Write and delete a file more times than the free space would hold:
/// fails with a full disk if delete leaves the file's clusters allocated.
/// Returns the disk once the volume is closed.
fn write_and_delete(
    volume_idx: VolumeIdx,
    file_mib: usize,
    rounds: usize,
) -> utils::RamDisk<Vec<u8>> {
    let volume_mgr = make_volume_manager();
    let volume = volume_mgr.open_raw_volume(volume_idx).expect("open volume");
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let data = vec![0xCC; file_mib * 1024 * 1024];
    for round in 0..rounds {
        let f = volume_mgr
            .open_file_in_dir(root_dir, "LEAK.DAT", Mode::ReadWriteCreateOrTruncate)
            .expect("open file");
        volume_mgr
            .write(f, &data)
            .unwrap_or_else(|e| panic!("write in round {round}: {e:?}"));
        volume_mgr.close_file(f).expect("close file");
        volume_mgr
            .delete_entry_in_dir(root_dir, "LEAK.DAT")
            .expect("delete file");
    }
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    volume_mgr.free().0
}

#[test]
fn delete_frees_clusters_fat16() {
    // About 64 MiB is free on the FAT16 partition
    write_and_delete(VolumeIdx(0), 1, 80);
}

#[test]
fn delete_frees_clusters_fat32() {
    let before = saved_free_count(&utils::make_block_device(utils::DISK_SOURCE).unwrap());
    // About 318 MiB is free on the FAT32 partition
    let disk = write_and_delete(VolumeIdx(1), 8, 41);
    // Every cluster allocated was freed and counted again
    assert_eq!(saved_free_count(&disk), before);
}

/// Delete `name` from the FAT16 partition's root directory of `disk`
fn fat16_delete(
    disk: utils::RamDisk<Vec<u8>>,
    name: &str,
) -> (
    Result<(), embedded_sdmmc::Error<utils::Error>>,
    utils::RamDisk<Vec<u8>>,
) {
    let volume_mgr: TestVolumeManager =
        VolumeManager::new_with_limits(disk, utils::make_time_source(), 0xAA00_0000);
    let volume = volume_mgr
        .open_raw_volume(VolumeIdx(0))
        .expect("open volume");
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let result = volume_mgr.delete_entry_in_dir(root_dir, name);
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    (result, volume_mgr.free().0)
}

#[test]
fn delete_stops_at_a_link_outside_the_volume() {
    use embedded_sdmmc::{Block, BlockDevice};
    let (disk, entry) = fat16_file("BROKEN.DAT", 64 * 1024);
    // The file's first cluster, from its directory entry
    let mut block = [Block::new()];
    disk.read(&mut block, entry.entry_block).unwrap();
    let e = &block[0].contents[entry.entry_offset as usize..];
    let first = u32::from(u16::from_le_bytes([e[26], e[27]]));
    // Link it to cluster 1, which is not a data cluster
    let (fat_block, at) = fat16_entry(&disk, first);
    disk.read(&mut block, fat_block).unwrap();
    block[0].contents[at..at + 2].copy_from_slice(&1u16.to_le_bytes());
    disk.write(&block, fat_block).unwrap();
    let reserved = fat16_reserved_entries(&disk);

    let (result, disk) = fat16_delete(disk, "BROKEN.DAT");
    assert!(matches!(result, Err(embedded_sdmmc::Error::FormatError(_))));
    assert_eq!(fat16_reserved_entries(&disk), reserved);
    // The cluster holding the bad link was the file's, so it is freed
    let (fat_block, at) = fat16_entry(&disk, first);
    disk.read(&mut block, fat_block).unwrap();
    assert_eq!(block[0].contents[at..at + 2], [0, 0]);
}

#[test]
fn delete_rejects_a_first_cluster_outside_the_volume() {
    use embedded_sdmmc::{Block, BlockDevice};
    let (disk, entry) = fat16_file("BROKEN.DAT", 64 * 1024);
    // The FAT16 partition has 65399 clusters (2 to 65400): point the entry
    // past them, at 0xFFF0, whose FAT entry is in the padding at the end of
    // the FAT's last block. Mark that entry, to see whether it is written.
    let mut block = [Block::new()];
    disk.read(&mut block, entry.entry_block).unwrap();
    let at = entry.entry_offset as usize + 26;
    block[0].contents[at..at + 2].copy_from_slice(&0xFFF0u16.to_le_bytes());
    disk.write(&block, entry.entry_block).unwrap();
    let (fat_block, at) = fat16_entry(&disk, 0xFFF0);
    disk.read(&mut block, fat_block).unwrap();
    block[0].contents[at..at + 2].copy_from_slice(&0xFFFFu16.to_le_bytes());
    disk.write(&block, fat_block).unwrap();

    let (result, disk) = fat16_delete(disk, "BROKEN.DAT");
    assert!(matches!(result, Err(embedded_sdmmc::Error::FormatError(_))));
    // Refused before anything changed: the entry is still there, and the
    // padding entry was not written
    disk.read(&mut block, entry.entry_block).unwrap();
    assert_ne!(block[0].contents[entry.entry_offset as usize], 0xE5);
    disk.read(&mut block, fat_block).unwrap();
    assert_eq!(block[0].contents[at..at + 2], [0xFF, 0xFF]);
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
