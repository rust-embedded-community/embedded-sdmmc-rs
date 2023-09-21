//! Directory related tests

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
        .open_volume(embedded_sdmmc::VolumeIdx(0))
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
        .open_volume(embedded_sdmmc::VolumeIdx(0))
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

    volume_mgr
        .iterate_dir(test_dir, |d| {
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
        .open_volume(embedded_sdmmc::VolumeIdx(1))
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

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
