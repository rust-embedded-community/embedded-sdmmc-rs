# Embedded SD/MMC

This crate is intended to allow you to read/write files on a FAT formatted SD
card on your Rust Embedded device, as easily as using the `SdFat` Arduino
library. It is written in pure-Rust, is `#![no_std]` and does not use `alloc`
or `collections` to keep the memory footprint low. In the first instance it is
designed for readability and simplicity over performance.

## Using the crate

You will need something that implements the `BlockDevice` trait, which can read and write the 512-byte blocks (or sectors) from your card. If you were to implement this over USB Mass Storage, there's no reason this crate couldn't work with a USB Thumb Drive, but we only supply a `BlockDevice` suitable for reading SD and SDHC cards over SPI.

```rust
let mut cont = embedded_sdmmc::Controller::new(embedded_sdmmc::SdMmcSpi::new(sdmmc_spi, sdmmc_cs));
write!(uart, "Init SD card...").unwrap();
match cont.device().init() {
    Ok(_) => {
        write!(uart, "OK!\nCard size...").unwrap();
        match cont.device().card_size_bytes() {
            Ok(size) => writeln!(uart, "{}", size).unwrap(),
            Err(e) => writeln!(uart, "Err: {:?}", e).unwrap(),
        }
        write!(uart, "Volume 0...").unwrap();
        match cont.get_volume(0) {
            Ok(v) => writeln!(uart, "{:?}", v).unwrap(),
            Err(e) => writeln!(uart, "Err: {:?}", e).unwrap(),
        }
    }
    Err(e) => writeln!(uart, "{:?}!", e).unwrap(),
}
```

## Supported features

* Open files read-only from an open directory
* Read data from open files
* Close files
* Iterate root directory
* Iterate sub-directories

## Todo List (PRs welcome!)

* Open non-root dirs
* Iterate non-root dirs
* Open files for append
* Append to files
* Create new dirs
* Create new files
* Delete files
* Delete (empty) directories
* Handle MS-DOS `/path/foo/bar.txt` style paths.

## Changelog

### Unreleased changes (will be 0.3.0)

* No changes

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
