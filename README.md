# Embedded SD/MMC [![crates.io](https://img.shields.io/crates/v/embedded-sdmmc.svg)](https://crates.io/crates/embedded-sdmmc) [![Documentation](https://docs.rs/embedded-sdmmc/badge.svg)](https://docs.rs/embedded-sdmmc)

This crate is intended to allow you to read/write files on a FAT formatted SD
card on your Rust Embedded device, as easily as using the `SdFat` Arduino
library. It is written in pure-Rust, is `#![no_std]` and does not use `alloc`
or `collections` to keep the memory footprint low. In the first instance it is
designed for readability and simplicity over performance.

## Using the crate

You will need something that implements the `BlockDevice` trait, which can read and write the 512-byte blocks (or sectors) from your card. If you were to implement this over USB Mass Storage, there's no reason this crate couldn't work with a USB Thumb Drive, but we only supply a `BlockDevice` suitable for reading SD and SDHC cards over SPI.

```rust
let mut cont = embedded_sdmmc::Controller::new(embedded_sdmmc::SdMmcSpi::new(sdmmc_spi, sdmmc_cs), time_source);
write!(uart, "Init SD card...").unwrap();
match cont.device().init() {
    Ok(_) => {
        write!(uart, "OK!\nCard size...").unwrap();
        match cont.device().card_size_bytes() {
            Ok(size) => writeln!(uart, "{}", size).unwrap(),
            Err(e) => writeln!(uart, "Err: {:?}", e).unwrap(),
        }
        write!(uart, "Volume 0...").unwrap();
        match cont.get_volume(embedded_sdmmc::VolumeIdx(0)) {
            Ok(v) => writeln!(uart, "{:?}", v).unwrap(),
            Err(e) => writeln!(uart, "Err: {:?}", e).unwrap(),
        }
    }
    Err(e) => writeln!(uart, "{:?}!", e).unwrap(),
}
```

## Supported features

* Open files in all supported methods from an open directory
* Read data from open files
* Write data to open files
* Close files
* Iterate root directory
* Iterate sub-directories

## Todo List (PRs welcome!)

* Create new dirs
* Delete files
* Delete (empty) directories
* Handle MS-DOS `/path/foo/bar.txt` style paths.

## Changelog

### Unreleased changes (will be 0.3.1)

* Code tidy-ups and more documentation.

### Version 0.3.0

* Updated to `v2` embedded-hal traits.
* Added open support for all modes.
* Added write support for files.
* Added `Info_Sector` tracking for FAT32.
* Change directory iteration to look in all the directory's clusters.
* Added `write_test` and `create_test`.
* De-duplicated FAT16 and FAT32 code (https://github.com/thejpster/embedded-sdmmc-rs/issues/10)

### Version 0.2.1

* Added `readme=README.md` to `Cargo.toml`

### Version 0.2.0

* Reduce delay waiting for response. Big speed improvements.

### Version 0.1.0

* Can read blocks from an SD Card using an `embedded_hal::SPI` device and a
  `embedded_hal::OutputPin` for Chip Select.
* Can read partition tables and open a FAT32 or FAT16 formatted partition.
* Can open and iterate the root directory of a FAT16 formatted partition.

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
