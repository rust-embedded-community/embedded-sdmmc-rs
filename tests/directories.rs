//! Directory related tests

use embedded_sdmmc::{Mode, ShortFileName};

mod utils;

#[derive(Debug, Clone)]
struct ExpectedDirEntry {
    name: String,
    mtime: String,
    ctime: String,
    size: u32,
    is_dir: bool,
}

impl PartialEq<embedded_sdmmc::DirEntry> for ExpectedDirEntry {
    fn eq(&self, other: &embedded_sdmmc::DirEntry) -> bool {
        if other.name.to_string() != self.name {
            return false;
        }
        if format!("{}", other.mtime) != self.mtime {
            return false;
        }
        if format!("{}", other.ctime) != self.ctime {
            return false;
        }
        if other.size != self.size {
            return false;
        }
        if other.attributes.is_directory() != self.is_dir {
            return false;
        }
        true
    }
}

#[test]
fn fat16_root_directory_listing() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut volume_mgr = embedded_sdmmc::VolumeManager::new(disk, time_source);

    let fat16_volume = volume_mgr
        .open_raw_volume(embedded_sdmmc::VolumeIdx(0))
        .expect("open volume 0");
    let root_dir = volume_mgr
        .open_root_dir(fat16_volume)
        .expect("open root dir");

    let expected = [
        ExpectedDirEntry {
            name: String::from("README.TXT"),
            mtime: String::from("2018-12-09 19:22:34"),
            ctime: String::from("2018-12-09 19:22:34"),
            size: 258,
            is_dir: false,
        },
        ExpectedDirEntry {
            name: String::from("EMPTY.DAT"),
            mtime: String::from("2018-12-09 19:21:16"),
            ctime: String::from("2018-12-09 19:21:16"),
            size: 0,
            is_dir: false,
        },
        ExpectedDirEntry {
            name: String::from("TEST"),
            mtime: String::from("2018-12-09 19:23:16"),
            ctime: String::from("2018-12-09 19:23:16"),
            size: 0,
            is_dir: true,
        },
        ExpectedDirEntry {
            name: String::from("64MB.DAT"),
            mtime: String::from("2018-12-09 19:21:38"),
            ctime: String::from("2018-12-09 19:21:38"),
            size: 64 * 1024 * 1024,
            is_dir: false,
        },
    ];

    let mut listing = Vec::new();

    volume_mgr
        .iterate_dir(root_dir, |d| {
            listing.push(d.clone());
        })
        .expect("iterate directory");

    assert_eq!(expected.len(), listing.len());
    for (expected_entry, given_entry) in expected.iter().zip(listing.iter()) {
        assert_eq!(
            expected_entry, given_entry,
            "{:#?} does not match {:#?}",
            given_entry, expected_entry
        );
    }
}

#[test]
fn fat16_sub_directory_listing() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut volume_mgr = embedded_sdmmc::VolumeManager::new(disk, time_source);

    let fat16_volume = volume_mgr
        .open_raw_volume(embedded_sdmmc::VolumeIdx(0))
        .expect("open volume 0");
    let root_dir = volume_mgr
        .open_root_dir(fat16_volume)
        .expect("open root dir");
    let test_dir = volume_mgr
        .open_dir(root_dir, "TEST")
        .expect("open test dir");

    let expected = [
        ExpectedDirEntry {
            name: String::from("."),
            mtime: String::from("2018-12-09 19:21:02"),
            ctime: String::from("2018-12-09 19:21:02"),
            size: 0,
            is_dir: true,
        },
        ExpectedDirEntry {
            name: String::from(".."),
            mtime: String::from("2018-12-09 19:21:02"),
            ctime: String::from("2018-12-09 19:21:02"),
            size: 0,
            is_dir: true,
        },
        ExpectedDirEntry {
            name: String::from("TEST.DAT"),
            mtime: String::from("2018-12-09 19:22:12"),
            ctime: String::from("2018-12-09 19:22:12"),
            size: 3500,
            is_dir: false,
        },
    ];

    let mut listing = Vec::new();
    let mut count = 0;

    volume_mgr
        .iterate_dir(test_dir, |d| {
            if count == 0 {
                assert!(d.name == ShortFileName::this_dir());
            } else if count == 1 {
                assert!(d.name == ShortFileName::parent_dir());
            }
            count += 1;
            listing.push(d.clone());
        })
        .expect("iterate directory");

    assert_eq!(expected.len(), listing.len());
    for (expected_entry, given_entry) in expected.iter().zip(listing.iter()) {
        assert_eq!(
            expected_entry, given_entry,
            "{:#?} does not match {:#?}",
            given_entry, expected_entry
        );
    }
}

#[test]
fn fat32_root_directory_listing() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut volume_mgr = embedded_sdmmc::VolumeManager::new(disk, time_source);

    let fat32_volume = volume_mgr
        .open_raw_volume(embedded_sdmmc::VolumeIdx(1))
        .expect("open volume 1");
    let root_dir = volume_mgr
        .open_root_dir(fat32_volume)
        .expect("open root dir");

    let expected = [
        ExpectedDirEntry {
            name: String::from("64MB.DAT"),
            mtime: String::from("2018-12-09 19:22:56"),
            ctime: String::from("2018-12-09 19:22:56"),
            size: 64 * 1024 * 1024,
            is_dir: false,
        },
        ExpectedDirEntry {
            name: String::from("EMPTY.DAT"),
            mtime: String::from("2018-12-09 19:22:56"),
            ctime: String::from("2018-12-09 19:22:56"),
            size: 0,
            is_dir: false,
        },
        ExpectedDirEntry {
            name: String::from("README.TXT"),
            mtime: String::from("2023-09-21 09:48:06"),
            ctime: String::from("2018-12-09 19:22:56"),
            size: 258,
            is_dir: false,
        },
        ExpectedDirEntry {
            name: String::from("TEST"),
            mtime: String::from("2018-12-09 19:23:20"),
            ctime: String::from("2018-12-09 19:23:20"),
            size: 0,
            is_dir: true,
        },
    ];

    let mut listing = Vec::new();

    volume_mgr
        .iterate_dir(root_dir, |d| {
            listing.push(d.clone());
        })
        .expect("iterate directory");

    assert_eq!(expected.len(), listing.len());
    for (expected_entry, given_entry) in expected.iter().zip(listing.iter()) {
        assert_eq!(
            expected_entry, given_entry,
            "{:#?} does not match {:#?}",
            given_entry, expected_entry
        );
    }
}

#[test]
fn open_dir_twice() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut volume_mgr = embedded_sdmmc::VolumeManager::new(disk, time_source);

    let fat32_volume = volume_mgr
        .open_raw_volume(embedded_sdmmc::VolumeIdx(1))
        .expect("open volume 1");

    let root_dir = volume_mgr
        .open_root_dir(fat32_volume)
        .expect("open root dir");

    let root_dir2 = volume_mgr
        .open_root_dir(fat32_volume)
        .expect("open it again");

    assert!(matches!(
        volume_mgr.open_dir(root_dir, "README.TXT"),
        Err(embedded_sdmmc::Error::OpenedFileAsDir)
    ));

    let test_dir = volume_mgr
        .open_dir(root_dir, "TEST")
        .expect("open test dir");

    let test_dir2 = volume_mgr.open_dir(root_dir, "TEST").unwrap();

    volume_mgr.close_dir(root_dir).expect("close root dir");
    volume_mgr.close_dir(test_dir).expect("close test dir");
    volume_mgr.close_dir(test_dir2).expect("close test dir");
    volume_mgr.close_dir(root_dir2).expect("close test dir");

    assert!(matches!(
        volume_mgr.close_dir(test_dir),
        Err(embedded_sdmmc::Error::BadHandle)
    ));
}

#[test]
fn open_too_many_dirs() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut volume_mgr: embedded_sdmmc::VolumeManager<
        utils::RamDisk<Vec<u8>>,
        utils::TestTimeSource,
        1,
        4,
        2,
    > = embedded_sdmmc::VolumeManager::new_with_limits(disk, time_source, 0x1000_0000);

    let fat32_volume = volume_mgr
        .open_raw_volume(embedded_sdmmc::VolumeIdx(1))
        .expect("open volume 1");
    let root_dir = volume_mgr
        .open_root_dir(fat32_volume)
        .expect("open root dir");

    assert!(matches!(
        volume_mgr.open_dir(root_dir, "TEST"),
        Err(embedded_sdmmc::Error::TooManyOpenDirs)
    ));
}

#[test]
fn find_dir_entry() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut volume_mgr = embedded_sdmmc::VolumeManager::new(disk, time_source);

    let fat32_volume = volume_mgr
        .open_raw_volume(embedded_sdmmc::VolumeIdx(1))
        .expect("open volume 1");

    let root_dir = volume_mgr
        .open_root_dir(fat32_volume)
        .expect("open root dir");

    let dir_entry = volume_mgr
        .find_directory_entry(root_dir, "README.TXT")
        .expect("Find directory entry");
    assert!(dir_entry.attributes.is_archive());
    assert!(!dir_entry.attributes.is_directory());
    assert!(!dir_entry.attributes.is_hidden());
    assert!(!dir_entry.attributes.is_lfn());
    assert!(!dir_entry.attributes.is_system());
    assert!(!dir_entry.attributes.is_volume());

    assert!(matches!(
        volume_mgr.find_directory_entry(root_dir, "README.TXS"),
        Err(embedded_sdmmc::Error::NotFound)
    ));
}

#[test]
fn delete_file() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut volume_mgr = embedded_sdmmc::VolumeManager::new(disk, time_source);

    let fat32_volume = volume_mgr
        .open_raw_volume(embedded_sdmmc::VolumeIdx(1))
        .expect("open volume 1");

    let root_dir = volume_mgr
        .open_root_dir(fat32_volume)
        .expect("open root dir");

    let file = volume_mgr
        .open_file_in_dir(root_dir, "README.TXT", Mode::ReadOnly)
        .unwrap();

    assert!(matches!(
        volume_mgr.delete_file_in_dir(root_dir, "README.TXT"),
        Err(embedded_sdmmc::Error::FileAlreadyOpen)
    ));

    assert!(matches!(
        volume_mgr.delete_file_in_dir(root_dir, "README2.TXT"),
        Err(embedded_sdmmc::Error::NotFound)
    ));

    volume_mgr.close_file(file).unwrap();

    volume_mgr
        .delete_file_in_dir(root_dir, "README.TXT")
        .unwrap();

    assert!(matches!(
        volume_mgr.delete_file_in_dir(root_dir, "README.TXT"),
        Err(embedded_sdmmc::Error::NotFound)
    ));

    assert!(matches!(
        volume_mgr.open_file_in_dir(root_dir, "README.TXT", Mode::ReadOnly),
        Err(embedded_sdmmc::Error::NotFound)
    ));
}

#[test]
fn make_directory() {
    let time_source = utils::make_time_source();
    let disk = utils::make_block_device(utils::DISK_SOURCE).unwrap();
    let mut volume_mgr = embedded_sdmmc::VolumeManager::new(disk, time_source);

    let fat32_volume = volume_mgr
        .open_raw_volume(embedded_sdmmc::VolumeIdx(1))
        .expect("open volume 1");

    let root_dir = volume_mgr
        .open_root_dir(fat32_volume)
        .expect("open root dir");

    let test_dir_name = ShortFileName::create_from_str("12345678.ABC").unwrap();
    let test_file_name = ShortFileName::create_from_str("ABC.TXT").unwrap();

    volume_mgr
        .make_dir_in_dir(root_dir, &test_dir_name)
        .unwrap();

    let new_dir = volume_mgr.open_dir(root_dir, &test_dir_name).unwrap();

    let mut has_this = false;
    let mut has_parent = false;
    volume_mgr
        .iterate_dir(new_dir, |item| {
            if item.name == ShortFileName::parent_dir() {
                has_parent = true;
                assert!(item.attributes.is_directory());
                assert_eq!(item.size, 0);
                assert_eq!(item.mtime.to_string(), utils::get_time_source_string());
                assert_eq!(item.ctime.to_string(), utils::get_time_source_string());
            } else if item.name == ShortFileName::this_dir() {
                has_this = true;
                assert!(item.attributes.is_directory());
                assert_eq!(item.size, 0);
                assert_eq!(item.mtime.to_string(), utils::get_time_source_string());
                assert_eq!(item.ctime.to_string(), utils::get_time_source_string());
            } else {
                panic!("Unexpected item in new dir");
            }
        })
        .unwrap();
    assert!(has_this);
    assert!(has_parent);

    let new_file = volume_mgr
        .open_file_in_dir(
            new_dir,
            &test_file_name,
            embedded_sdmmc::Mode::ReadWriteCreate,
        )
        .expect("open new file");
    volume_mgr
        .write(new_file, b"Hello")
        .expect("write to new file");
    volume_mgr.close_file(new_file).expect("close new file");

    let mut has_this = false;
    let mut has_parent = false;
    let mut has_new_file = false;
    volume_mgr
        .iterate_dir(new_dir, |item| {
            if item.name == ShortFileName::parent_dir() {
                has_parent = true;
                assert!(item.attributes.is_directory());
                assert_eq!(item.size, 0);
                assert_eq!(item.mtime.to_string(), utils::get_time_source_string());
                assert_eq!(item.ctime.to_string(), utils::get_time_source_string());
            } else if item.name == ShortFileName::this_dir() {
                has_this = true;
                assert!(item.attributes.is_directory());
                assert_eq!(item.size, 0);
                assert_eq!(item.mtime.to_string(), utils::get_time_source_string());
                assert_eq!(item.ctime.to_string(), utils::get_time_source_string());
            } else if item.name == test_file_name {
                has_new_file = true;
                // We wrote "Hello" to it
                assert_eq!(item.size, 5);
                assert!(!item.attributes.is_directory());
                assert_eq!(item.mtime.to_string(), utils::get_time_source_string());
                assert_eq!(item.ctime.to_string(), utils::get_time_source_string());
            } else {
                panic!("Unexpected item in new dir");
            }
        })
        .unwrap();
    assert!(has_this);
    assert!(has_parent);
    assert!(has_new_file);

    // Close the root dir and look again
    volume_mgr.close_dir(root_dir).expect("close root");
    volume_mgr.close_dir(new_dir).expect("close new_dir");
    let root_dir = volume_mgr
        .open_root_dir(fat32_volume)
        .expect("open root dir");
    // Check we can't make it again now it exists
    assert!(volume_mgr
        .make_dir_in_dir(root_dir, &test_dir_name)
        .is_err());
    let new_dir = volume_mgr
        .open_dir(root_dir, &test_dir_name)
        .expect("find new dir");
    let new_file = volume_mgr
        .open_file_in_dir(new_dir, &test_file_name, embedded_sdmmc::Mode::ReadOnly)
        .expect("re-open new file");
    volume_mgr.close_dir(root_dir).expect("close root");
    volume_mgr.close_dir(new_dir).expect("close new dir");
    volume_mgr.close_file(new_file).expect("close file");
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
