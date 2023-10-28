//! Reading related tests

use sha2::Digest;

mod utils;

static TEST_DAT_SHA256_SUM: &[u8] =
    b"\x59\xe3\x46\x8e\x3b\xef\x8b\xfe\x37\xe6\x0a\x82\x21\xa1\x89\x6e\x10\x5b\x80\xa6\x1a\x23\x63\x76\x12\xac\x8c\xd2\x4c\xa0\x4a\x75";

#[test]
fn read_file_512_blocks() {
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
        .expect("Open test dir");

    let test_file = volume_mgr
        .open_file_in_dir(test_dir, "TEST.DAT", embedded_sdmmc::Mode::ReadOnly)
        .expect("open test file");

    let mut contents = Vec::new();

    let mut partial = false;
    while !volume_mgr.file_eof(test_file).expect("check eof") {
        let mut buffer = [0u8; 512];
        let len = volume_mgr.read(test_file, &mut buffer).expect("read data");
        if len != buffer.len() {
            if partial {
                panic!("Two partial reads!");
            } else {
                partial = true;
            }
        }
        contents.extend(&buffer[0..len]);
    }

    let mut hasher = sha2::Sha256::new();
    hasher.update(contents);
    let hash = hasher.finalize();
    assert_eq!(&hash[..], TEST_DAT_SHA256_SUM);
}

#[test]
fn read_file_all() {
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
        .expect("Open test dir");

    let test_file = volume_mgr
        .open_file_in_dir(test_dir, "TEST.DAT", embedded_sdmmc::Mode::ReadOnly)
        .expect("open test file");

    let mut contents = vec![0u8; 4096];
    let len = volume_mgr
        .read(test_file, &mut contents)
        .expect("read data");
    if len != 3500 {
        panic!("Failed to read all of TEST.DAT");
    }

    let mut hasher = sha2::Sha256::new();
    hasher.update(&contents[0..3500]);
    let hash = hasher.finalize();
    assert_eq!(&hash[..], TEST_DAT_SHA256_SUM);
}

#[test]
fn read_file_prime_blocks() {
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
        .expect("Open test dir");

    let test_file = volume_mgr
        .open_file_in_dir(test_dir, "TEST.DAT", embedded_sdmmc::Mode::ReadOnly)
        .expect("open test file");

    let mut contents = Vec::new();

    let mut partial = false;
    while !volume_mgr.file_eof(test_file).expect("check eof") {
        // Exercise the alignment code by reading in chunks of 53 bytes
        let mut buffer = [0u8; 53];
        let len = volume_mgr.read(test_file, &mut buffer).expect("read data");
        if len != buffer.len() {
            if partial {
                panic!("Two partial reads!");
            } else {
                partial = true;
            }
        }
        contents.extend(&buffer[0..len]);
    }

    let mut hasher = sha2::Sha256::new();
    hasher.update(&contents[0..3500]);
    let hash = hasher.finalize();
    assert_eq!(&hash[..], TEST_DAT_SHA256_SUM);
}

#[test]
fn read_file_backwards() {
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
        .expect("Open test dir");

    let test_file = volume_mgr
        .open_file_in_dir(test_dir, "TEST.DAT", embedded_sdmmc::Mode::ReadOnly)
        .expect("open test file");

    let mut contents = std::collections::VecDeque::new();

    const CHUNK_SIZE: u32 = 100;
    let length = volume_mgr.file_length(test_file).expect("file length");
    let mut offset = length - CHUNK_SIZE;
    let mut read = 0;

    // We're going to read the file backwards in chunks of 100 bytes. This
    // checks we didn't make any assumptions about only going forwards.
    while read < length {
        volume_mgr
            .file_seek_from_start(test_file, offset)
            .expect("seek");
        let mut buffer = [0u8; CHUNK_SIZE as usize];
        let len = volume_mgr.read(test_file, &mut buffer).expect("read");
        assert_eq!(len, CHUNK_SIZE as usize);
        contents.push_front(buffer.to_vec());
        read += CHUNK_SIZE;
        if offset >= CHUNK_SIZE {
            offset -= CHUNK_SIZE;
        }
    }

    assert_eq!(read, length);
    assert_eq!(offset, 0);

    let flat: Vec<u8> = contents.iter().flatten().copied().collect();

    let mut hasher = sha2::Sha256::new();
    hasher.update(flat);
    let hash = hasher.finalize();
    assert_eq!(&hash[..], TEST_DAT_SHA256_SUM);
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
