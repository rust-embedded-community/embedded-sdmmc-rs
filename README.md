# Embedded SD/MMC [![crates.io](https://img.shields.io/crates/v/embedded-sdmmc.svg)](https://crates.io/crates/embedded-sdmmc) [![Documentation](https://docs.rs/embedded-sdmmc/badge.svg)](https://docs.rs/embedded-sdmmc)

This crate is intended to allow you to read/write files on a FAT formatted SD
card on your Rust Embedded device, as easily as using the `SdFat` Arduino
library. It is written in pure-Rust, is `#![no_std]` and does not use `alloc`
or `collections` to keep the memory footprint low. In the first instance it is
designed for readability and simplicity over performance.

## Using the crate

You will need something that implements the `BlockDevice` trait, which can read and write the 512-byte blocks (or sectors) from your card. If you were to implement this over USB Mass Storage, there's no reason this crate couldn't work with a USB Thumb Drive, but we only supply a `BlockDevice` suitable for reading SD and SDHC cards over SPI.

```rust
let mut spi_dev = embedded_sdmmc::SdMmcSpi::new(embedded_sdmmc::SdMmcSpi::new(sdmmc_spi, sdmmc_cs), time_source);
write!(uart, "Init SD card...").unwrap();
match spi_dev.acquire() {
    Ok(block) => {
        let mut cont = embedded_sdmmc::Controller::new(block, time_source);
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

### Open directories and files

By default the `Controller` will initialize with a maximum number of `4` open directories and files. This can be customized by specifying the `MAX_DIR` and `MAX_FILES` generic consts of the `Controller`:

```rust
// Create a controller with a maximum of 6 open directories and 12 open files
let mut cont: Controller<
    embedded_sdmmc::BlockSpi<DummySpi, DummyCsPin>,
    DummyTimeSource,
    6,
    12,
> = Controller::new_with_limits(block, time_source);
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
