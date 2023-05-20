# Embedded SD/MMC [![crates.io](https://img.shields.io/crates/v/embedded-sdmmc.svg)](https://crates.io/crates/embedded-sdmmc) [![Documentation](https://docs.rs/embedded-sdmmc/badge.svg)](https://docs.rs/embedded-sdmmc)

This crate is intended to allow you to read/write files on a FAT formatted SD
card on your Rust Embedded device, as easily as using the `SdFat` Arduino
library. It is written in pure-Rust, is `#![no_std]` and does not use `alloc`
or `collections` to keep the memory footprint low. In the first instance it is
designed for readability and simplicity over performance.

## Using the crate

You will need something that implements the `BlockDevice` trait, which can read and write the 512-byte blocks (or sectors) from your card. If you were to implement this over USB Mass Storage, there's no reason this crate couldn't work with a USB Thumb Drive, but we only supply a `BlockDevice` suitable for reading SD and SDHC cards over SPI.

```rust
// Build an SD Card interface out of an SPI device, a chip-select pin and a delay object
let sdcard = embedded_sdmmc::SdCard::new(sdmmc_spi, sdmmc_cs, delay);
// Get the card size (this also triggers card initialisation because it's not been done yet)
println!("Card size is {} bytes", sdcard.num_bytes()?);
// Now let's look for volumes (also known as partitions) on our block device.
// To do this we need a Volume Manager. It will take ownership of the block device.
let mut volume_mgr = embedded_sdmmc::VolumeManager::new(sdcard, time_source);
// Try and access Volume 0 (i.e. the first partition).
// The volume object holds information about the filesystem on that volume.
// It doesn't hold a reference to the Volume Manager and so must be passed back
// to every Volume Manager API call. This makes it easier to handle multiple
// volumes in parallel.
let mut volume0 = volume_mgr.get_volume(embedded_sdmmc::VolumeIdx(0))?;
println!("Volume 0: {:?}", volume0);
// Open the root directory (passing in the volume we're using).
let root_dir = volume_mgr.open_root_dir(&volume0)?;
// Open a file called "MY_FILE.TXT" in the root directory
let mut my_file = volume_mgr.open_file_in_dir(
    &mut volume0,
    &root_dir,
    "MY_FILE.TXT",
    embedded_sdmmc::Mode::ReadOnly,
)?;
// Print the contents of the file
while !my_file.eof() {
    let mut buffer = [0u8; 32];
    let num_read = volume_mgr.read(&volume0, &mut my_file, &mut buffer)?;
    for b in &buffer[0..num_read] {
        print!("{}", *b as char);
    }
}
volume_mgr.close_file(&volume0, my_file)?;
volume_mgr.close_dir(&volume0, root_dir);
```

### Open directories and files

By default the `VolumeManager` will initialize with a maximum number of `4` open directories and files. This can be customized by specifying the `MAX_DIR` and `MAX_FILES` generic consts of the `VolumeManager`:

```rust
// Create a volume manager with a maximum of 6 open directories and 12 open files
let mut cont: VolumeManager<_, _, 6, 12> = VolumeManager::new_with_limits(block, time_source);
```

## Supported features

* Open files in all supported methods from an open directory
* Open an arbitrary number of directories and files
* Read data from open files
* Write data to open files
* Close files
* Delete files
* Iterate root directory
* Iterate sub-directories
* Log over defmt or the common log interface (feature flags).

## Todo List (PRs welcome!)

* Create new dirs
* Delete (empty) directories
* Handle MS-DOS `/path/foo/bar.txt` style paths.

## Changelog

The changelog has moved to [CHANGELOG.md](/CHANGELOG.md)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)

- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
