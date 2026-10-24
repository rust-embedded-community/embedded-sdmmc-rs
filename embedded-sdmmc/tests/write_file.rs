//! File opening related tests

use embedded_sdmmc::{Block, BlockDevice, BlockIdx, Mode};

use utils::TestVolumeManager;

mod utils;

#[test]
fn append_file() {
    let mut volume_mgr = TestVolumeManager::new();
    let volume = volume_mgr.open_fat16_volume();
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
    let mut volume_mgr = TestVolumeManager::new();
    let volume = volume_mgr.open_fat16_volume();
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
    let mut volume_mgr = TestVolumeManager::new();
    let volume = volume_mgr.open_fat16_volume();
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
    // The FAT16 partition is followed by the FAT32 partition, whose boot
    // sector is at block 264192.
    const NEXT_PARTITION: BlockIdx = BlockIdx(264192);

    let volume_mgr = TestVolumeManager::new();
    let disk = volume_mgr.unmount();

    let mut before = [Block::new()];
    disk.read(&mut before, NEXT_PARTITION).unwrap();
    let mut volume_mgr = TestVolumeManager::mount(disk);

    let volume = volume_mgr.open_fat16_volume();
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
    let disk = volume_mgr.unmount();
    disk.read(&mut after, NEXT_PARTITION).unwrap();
    assert!(
        after[0].contents == before[0].contents,
        "the next partition's boot sector was overwritten"
    );
}

#[test]
fn full_volume_uses_its_last_cluster() {
    // The FAT32 partition's FSInfo sector, with its free cluster count at
    // byte 488 (accurate on the test disk)
    const FSINFO: BlockIdx = BlockIdx(264192 + 1);
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut block = [Block::new()];
    disk.read(&mut block, FSINFO).unwrap();
    let free = u32::from_le_bytes(block[0].contents[488..492].try_into().unwrap());
    let mut volume_mgr = TestVolumeManager::mount(disk);
    let volume = volume_mgr.open_fat32_volume();
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
    let disk = volume_mgr.unmount();
    disk.read(&mut block, FSINFO).unwrap();
    assert_eq!(block[0].contents[488..492], 0u32.to_le_bytes());
}

#[test]
fn delete_frees_clusters_fat16() {
    // About 64 MiB is free on the FAT16 partition
    let mut volume_mgr = TestVolumeManager::new();
    let volume = volume_mgr.open_fat16_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let data = vec![0xCC; 1 * 1024 * 1024];
    for round in 0..80 {
        let f = volume_mgr
            .open_file_in_dir(root_dir, "LEAK.DAT", Mode::ReadWriteCreateOrTruncate)
            .expect("open file");
        if let Err(e) = volume_mgr.write(f, &data) {
            panic!("write in round {round}: {e:?}");
        }
        volume_mgr.close_file(f).expect("close file");
        volume_mgr
            .delete_entry_in_dir(root_dir, "LEAK.DAT")
            .expect("delete file");
    }
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
}

#[test]
fn delete_frees_clusters_fat32() {
    let volume_mgr = TestVolumeManager::new();
    let disk = volume_mgr.unmount();
    let before = saved_free_count(&disk);
    // About 318 MiB is free on the FAT32 partition
    let mut volume_mgr = TestVolumeManager::mount(disk);
    let volume = volume_mgr.open_fat16_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let data = vec![0xCC; 1 * 1024 * 1024];
    for round in 0..80 {
        let f = volume_mgr
            .open_file_in_dir(root_dir, "LEAK.DAT", Mode::ReadWriteCreateOrTruncate)
            .expect("open file");
        if let Err(e) = volume_mgr.write(f, &data) {
            panic!("write in round {round}: {e:?}");
        }
        volume_mgr.close_file(f).expect("close file");
        volume_mgr
            .delete_entry_in_dir(root_dir, "LEAK.DAT")
            .expect("delete file");
    }
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();
    // Every cluster allocated was freed and counted again
    assert_eq!(saved_free_count(&disk), before);
}

#[test]
fn delete_stops_at_a_link_outside_the_volume() {
    // Let's mount the test disk and find where 64MB.DAT is
    let mut volume_mgr = TestVolumeManager::new();
    let volume = volume_mgr.open_fat16_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let entry = volume_mgr
        .find_directory_entry(root_dir, "64MB.DAT")
        .expect("find entry");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();

    // Read cluster chain for file, using its directory entry
    let mut block = [Block::new()];
    disk.read(&mut block, entry.entry_block).unwrap();
    let e = &block[0].contents[entry.entry_offset as usize..];
    // this is the first entry in the cluster chain for 64MB.DAT
    let first = u32::from(u16::from_le_bytes([e[26], e[27]]));

    // Link it to cluster 1, which is a reserved cluster and not a data cluster
    let (fat_block, at) = fat16_entry(&disk, first);
    disk.read(&mut block, fat_block).unwrap();
    block[0].contents[at..at + 2].copy_from_slice(&1u16.to_le_bytes());
    disk.write(&block, fat_block).unwrap();
    let reserved = fat16_reserved_entries(&disk);

    // Mount the corrupt disk and check the file is not deleteable
    let mut volume_mgr = TestVolumeManager::mount(disk);
    let volume = volume_mgr.open_fat16_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let result = volume_mgr.delete_entry_in_dir(root_dir, "64MB.DAT");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();

    // check we detected the corruption
    assert!(matches!(result, Err(embedded_sdmmc::Error::FormatError(_))));

    // check these did not change
    assert_eq!(fat16_reserved_entries(&disk), reserved);

    // We should have freed the first entry though
    let (fat_block, at) = fat16_entry(&disk, first);
    disk.read(&mut block, fat_block).unwrap();
    assert_eq!(block[0].contents[at..at + 2], [0, 0]);
}

#[test]
fn delete_rejects_a_first_cluster_outside_the_volume() {
    // Let's mount the test disk and find where 64MB.DAT is
    let mut volume_mgr = TestVolumeManager::new();
    let volume = volume_mgr.open_fat16_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let entry = volume_mgr
        .find_directory_entry(root_dir, "64MB.DAT")
        .expect("find entry");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();

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

    // Delete the file
    let mut volume_mgr = TestVolumeManager::mount(disk);
    let volume = volume_mgr.open_fat16_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let result = volume_mgr.delete_entry_in_dir(root_dir, "64MB.DAT");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();

    assert!(matches!(result, Err(embedded_sdmmc::Error::FormatError(_))));
    // Was it refused before anything changed: the entry is still there, and the?
    // padding entry was not written
    disk.read(&mut block, entry.entry_block).unwrap();
    assert_ne!(block[0].contents[entry.entry_offset as usize], 0xE5);
    disk.read(&mut block, fat_block).unwrap();
    assert_eq!(block[0].contents[at..at + 2], [0xFF, 0xFF]);
}

#[test]
fn truncate_counts_every_freed_cluster() {
    let mut volume_mgr = TestVolumeManager::new();
    let volume = volume_mgr.open_fat32_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let f = volume_mgr
        .open_file_in_dir(root_dir, "TRUNC.DAT", Mode::ReadWriteCreateOrTruncate)
        .expect("open file");
    volume_mgr
        .write(f, &vec![0xCC; 1024 * 1024])
        .expect("file write");
    volume_mgr.close_file(f).expect("close file");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();
    let before = saved_free_count(&disk);

    let mut volume_mgr = TestVolumeManager::mount(disk);
    let volume = volume_mgr.open_fat32_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let f = volume_mgr
        .open_file_in_dir(root_dir, "TRUNC.DAT", Mode::ReadWriteTruncate)
        .expect("open file");
    volume_mgr.close_file(f).expect("close file");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    // 1 MiB is 256 clusters of 4 KiB; a truncated file keeps its first one
    let disk = volume_mgr.unmount();
    assert_eq!(saved_free_count(&disk), before + 255);
}

#[test]
fn truncate_stops_at_a_link_outside_the_volume() {
    // Let's mount the test disk and find where 64MB.DAT is
    let mut volume_mgr = TestVolumeManager::new();
    let volume = volume_mgr.open_fat16_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let entry = volume_mgr
        .find_directory_entry(root_dir, "64MB.DAT")
        .expect("find entry");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();

    let mut block = [Block::new()];
    disk.read(&mut block, entry.entry_block).unwrap();
    let e = &block[0].contents[entry.entry_offset as usize..];
    let first = u32::from(u16::from_le_bytes([e[26], e[27]]));
    // The second cluster links to cluster 0
    let (fat_block, at) = fat16_entry(&disk, first);
    disk.read(&mut block, fat_block).unwrap();
    let second = u32::from(u16::from_le_bytes([
        block[0].contents[at],
        block[0].contents[at + 1],
    ]));
    let (fat_block, at) = fat16_entry(&disk, second);
    disk.read(&mut block, fat_block).unwrap();
    block[0].contents[at..at + 2].copy_from_slice(&0u16.to_le_bytes());
    disk.write(&block, fat_block).unwrap();
    let reserved = fat16_reserved_entries(&disk);

    // Truncate the file
    let mut volume_mgr = TestVolumeManager::mount(disk);
    let volume = volume_mgr.open_fat16_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let result = volume_mgr.open_file_in_dir(root_dir, "64MB.DAT", Mode::ReadWriteTruncate);
    assert!(matches!(result, Err(embedded_sdmmc::Error::FormatError(_))));
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();

    assert_eq!(fat16_reserved_entries(&disk), reserved);
    // The file is left empty, its first cluster the end of its chain
    disk.read(&mut block, entry.entry_block).unwrap();
    let e = &block[0].contents[entry.entry_offset as usize..];
    assert_eq!(u32::from_le_bytes([e[28], e[29], e[30], e[31]]), 0);
    let (fat_block, at) = fat16_entry(&disk, first);
    disk.read(&mut block, fat_block).unwrap();
    assert!(u16::from_le_bytes([block[0].contents[at], block[0].contents[at + 1]]) >= 0xFFF8);
}

#[test]
fn truncate_rejects_a_first_cluster_outside_the_volume() {
    // Let's mount the test disk and find where 64MB.DAT is
    let mut volume_mgr = TestVolumeManager::new();
    let volume = volume_mgr.open_fat16_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let entry = volume_mgr
        .find_directory_entry(root_dir, "64MB.DAT")
        .expect("find entry");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();

    // Point the entry at 0xFFF0, past the last cluster (65400), whose FAT
    // entry is in the padding at the end of the FAT's last block. Mark that
    // entry, to see whether it is written.
    let mut block = [Block::new()];
    disk.read(&mut block, entry.entry_block).unwrap();
    let at = entry.entry_offset as usize + 26;
    block[0].contents[at..at + 2].copy_from_slice(&0xFFF0u16.to_le_bytes());
    disk.write(&block, entry.entry_block).unwrap();
    let (fat_block, at) = fat16_entry(&disk, 0xFFF0);
    disk.read(&mut block, fat_block).unwrap();
    block[0].contents[at..at + 2].copy_from_slice(&0x1234u16.to_le_bytes());
    disk.write(&block, fat_block).unwrap();

    // Truncate the file
    let mut volume_mgr = TestVolumeManager::mount(disk);
    let volume = volume_mgr.open_fat16_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let result = volume_mgr.open_file_in_dir(root_dir, "64MB.DAT", Mode::ReadWriteTruncate);
    assert!(matches!(result, Err(embedded_sdmmc::Error::FormatError(_))));
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();

    // Was it refused before anything changed?
    assert_eq!(
        fat16_entry_size_and_cluster(&disk, &entry),
        (64 * 1024 * 1024, 0xFFF0)
    );
    disk.read(&mut block, fat_block).unwrap();
    assert_eq!(block[0].contents[at..at + 2], 0x1234u16.to_le_bytes());
}

#[test]
fn truncate_rejects_a_first_link_outside_the_volume() {
    // Let's mount the test disk and find where 64MB.DAT is
    let mut volume_mgr = TestVolumeManager::new();
    let volume = volume_mgr.open_fat16_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let entry = volume_mgr
        .find_directory_entry(root_dir, "64MB.DAT")
        .expect("find entry");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();

    let (_, first) = fat16_entry_size_and_cluster(&disk, &entry);
    // Link the first cluster to cluster 1, which is not a data cluster
    let mut block = [Block::new()];
    let (fat_block, at) = fat16_entry(&disk, u32::from(first));
    disk.read(&mut block, fat_block).unwrap();
    block[0].contents[at..at + 2].copy_from_slice(&1u16.to_le_bytes());
    disk.write(&block, fat_block).unwrap();
    let reserved = fat16_reserved_entries(&disk);

    // remount the disk and open the file with truncation
    let mut volume_mgr = TestVolumeManager::mount(disk);
    let volume = volume_mgr.open_fat16_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let result = volume_mgr
        .open_file_in_dir(root_dir, "64MB.DAT", Mode::ReadWriteTruncate)
        .map(|f| volume_mgr.close_file(f).expect("close file"));
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();

    assert!(matches!(result, Err(embedded_sdmmc::Error::FormatError(_))));
    // Was it refused before anything changed?
    assert_eq!(fat16_reserved_entries(&disk), reserved);
    assert_eq!(
        fat16_entry_size_and_cluster(&disk, &entry),
        (64 * 1024 * 1024, first)
    );
    disk.read(&mut block, fat_block).unwrap();
    assert_eq!(block[0].contents[at..at + 2], 1u16.to_le_bytes());
}

#[test]
fn free_count_too_low_becomes_unknown() {
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    force_override_free_count(&disk, 1);
    let mut volume_mgr = TestVolumeManager::mount(disk);
    let volume = volume_mgr.open_fat32_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let f = volume_mgr
        .open_file_in_dir(root_dir, "LOW.DAT", Mode::ReadWriteCreateOrTruncate)
        .expect("open file");
    volume_mgr
        .write(f, &vec![0xCC; 1024 * 1024])
        .expect("file write");
    volume_mgr.close_file(f).expect("close file");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    // 0xFFFF_FFFF is the "unknown" value of the spec
    let disk = volume_mgr.unmount();
    assert_eq!(saved_free_count(&disk), 0xFFFF_FFFF);
}

#[test]
fn free_count_too_high_becomes_unknown() {
    let mut volume_mgr = TestVolumeManager::new();
    let volume = volume_mgr.open_fat32_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let f = volume_mgr
        .open_file_in_dir(root_dir, "HIGH.DAT", Mode::ReadWriteCreateOrTruncate)
        .expect("open file");
    volume_mgr
        .write(f, &vec![0xCC; 1024 * 1024])
        .expect("file write");
    volume_mgr.close_file(f).expect("close file");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();
    // The partition's cluster count, from its boot sector
    let mut boot = [Block::new()];
    disk.read(&mut boot, BlockIdx(264192)).unwrap();
    let b = &boot[0].contents;
    let total = u32::from_le_bytes(b[32..36].try_into().unwrap());
    let reserved = u32::from(u16::from_le_bytes([b[14], b[15]]));
    let fat_size = u32::from_le_bytes(b[36..40].try_into().unwrap());
    let clusters = (total - reserved - u32::from(b[16]) * fat_size) / u32::from(b[13]);
    // Claim every cluster is free, though the file still has some
    force_override_free_count(&disk, clusters);

    let mut volume_mgr = TestVolumeManager::mount(disk);
    let volume = volume_mgr.open_fat32_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    volume_mgr
        .delete_entry_in_dir(root_dir, "HIGH.DAT")
        .expect("delete file");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();
    assert_eq!(saved_free_count(&disk), 0xFFFF_FFFF);
}

#[test]
fn free_count_above_the_cluster_count_becomes_unknown() {
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    // The partition's cluster count, from its boot sector
    let mut boot = [Block::new()];
    disk.read(&mut boot, BlockIdx(264192)).unwrap();
    let b = &boot[0].contents;
    let total = u32::from_le_bytes(b[32..36].try_into().unwrap());
    let reserved = u32::from(u16::from_le_bytes([b[14], b[15]]));
    let fat_size = u32::from_le_bytes(b[36..40].try_into().unwrap());
    let clusters = (total - reserved - u32::from(b[16]) * fat_size) / u32::from(b[13]);
    force_override_free_count(&disk, clusters + 1000);
    let mut volume_mgr = TestVolumeManager::mount(disk);
    let volume = volume_mgr.open_fat32_volume();
    let root_dir = volume_mgr.open_root_dir(volume).expect("open root dir");
    let f = volume_mgr
        .open_file_in_dir(root_dir, "HIGH.DAT", Mode::ReadWriteCreateOrTruncate)
        .expect("open file");
    volume_mgr
        .write(f, &vec![0xCC; 64 * 1024])
        .expect("file write");
    volume_mgr.close_file(f).expect("close file");
    volume_mgr.close_dir(root_dir).expect("close dir");
    volume_mgr.close_volume(volume).expect("close volume");
    let disk = volume_mgr.unmount();
    assert_eq!(saved_free_count(&disk), 0xFFFF_FFFF);
}

/// The FAT32 partition's free cluster count, as saved on the disk
pub fn saved_free_count(disk: &utils::RamDisk<Vec<u8>>) -> u32 {
    let mut block = [Block::new()];
    disk.read(&mut block, utils::FAT32_FSINFO).unwrap();
    u32::from_le_bytes(block[0].contents[488..492].try_into().unwrap())
}

/// Read an entry from a FAT16 File Allocation Table
///
/// Assumes 1K clusters
fn fat16_entry(disk: &utils::RamDisk<Vec<u8>>, cluster: u32) -> (embedded_sdmmc::BlockIdx, usize) {
    let mut boot = [Block::new()];
    disk.read(&mut boot, BlockIdx(utils::FAT16_START)).unwrap();
    let reserved = u32::from(u16::from_le_bytes([
        boot[0].contents[14],
        boot[0].contents[15],
    ]));
    let fat_start = utils::FAT16_START + reserved;
    (
        BlockIdx(fat_start + cluster * 2 / 512),
        (cluster * 2 % 512) as usize,
    )
}

/// The FAT16 partition's FAT[0] and FAT[1] (the media descriptor and the
/// end-of-chain marker)
fn fat16_reserved_entries(disk: &utils::RamDisk<Vec<u8>>) -> [u8; 4] {
    let (fat_block, _) = fat16_entry(disk, 0);
    let mut block = [Block::new()];
    disk.read(&mut block, fat_block).unwrap();
    block[0].contents[0..4].try_into().unwrap()
}

/// Get the size and first cluster in a FAT16 directory entry
fn fat16_entry_size_and_cluster(
    disk: &utils::RamDisk<Vec<u8>>,
    entry: &embedded_sdmmc::DirEntry,
) -> (u32, u16) {
    let mut block = [Block::new()];
    disk.read(&mut block, entry.entry_block).unwrap();
    let e = &block[0].contents[entry.entry_offset as usize..];
    (
        u32::from_le_bytes([e[28], e[29], e[30], e[31]]),
        u16::from_le_bytes([e[26], e[27]]),
    )
}

/// Force the FAT32 partition's saved free cluster count to be a different value
///
/// Useful for testing underflow and overflow on the free cluster count
fn force_override_free_count(disk: &utils::RamDisk<Vec<u8>>, count: u32) {
    let mut block = [Block::new()];
    disk.read(&mut block, utils::FAT32_FSINFO).unwrap();
    block[0].contents[488..492].copy_from_slice(&count.to_le_bytes());
    disk.write(&block, utils::FAT32_FSINFO).unwrap();
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
