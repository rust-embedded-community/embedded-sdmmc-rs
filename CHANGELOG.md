# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased](https://github.com/rust-embedded-community/embedded-sdmmc-rs/compare/v0.5.0...develop)

### Changed

- None

### Added

- None

### Removed

- None

## [Version 0.5.0](https://github.com/rust-embedded-community/embedded-sdmmc-rs/releases/tag/v0.5.0) - 2023-05-20

### Changes

- __Breaking Change__: Renamed `Controller` to `VolumeManager`, to better describe what it does.
- __Breaking Change__: Renamed `SdMmcSpi` to `SdCard`
- __Breaking Change__: `AcquireOpts` now has `use_crc` (which makes it ask for CRCs to be enabled) instead of `require_crc` (which simply allowed the enable-CRC command to fail)
- __Breaking Change__: `SdCard::new` now requires an object that implements the embedded-hal `DelayUs` trait
- __Breaking Change__: Renamed `card_size_bytes` to `num_bytes`, to match `num_blocks`
- More robust card intialisation procedure, with added retries
- Supports building with neither `defmt` nor `log` logging

### Added

- Added `mark_card_as_init` method, if you know the card is initialised and want to skip the initialisation step

### Removed

- __Breaking Change__: Removed `BlockSpi` type - card initialisation now handled as an internal state variable

## [Version 0.4.0](https://github.com/rust-embedded-community/embedded-sdmmc-rs/releases/tag/v0.4.0) - 2023-01-18

### Changes
- Optionally use [defmt](https://github.com/knurling-rs/defmt) for logging.
    Controlled by `defmt-log` feature flag.
- __Breaking Change__: Use SPI blocking traits instead to ease SPI peripheral sharing.
  See: https://github.com/rust-embedded-community/embedded-sdmmc-rs/issues/28
- Added `Controller::has_open_handles` and `Controller::free` methods.
- __Breaking Change__: Changed interface to enforce correct SD state at compile time.
- __Breaking Change__: Added custom error type for `File` operations.
- Fix `env_logger` pulling in the `std` feature in `log` in library builds.
- Raise the minimum supported Rust version to 1.56.0.
- Code tidy-ups and more documentation.
- Add `MAX_DIRS` and `MAX_FILES` generics to `Controller` to allow an arbitrary numbers of concurrent open directories and files.
- Add new constructor method `Controller::new_with_limits(block_device: D, timesource: T) -> Controller<D, T, MAX_DIRS, MAX_FILES>`
  to create a `Controller` with custom limits.

## [Version 0.3.0](https://github.com/rust-embedded-community/embedded-sdmmc-rs/releases/tag/v0.3.0) - 2019-12-16

### Changes

* Updated to `v2` embedded-hal traits.
* Added open support for all modes.
* Added write support for files.
* Added `Info_Sector` tracking for FAT32.
* Change directory iteration to look in all the directory's clusters.
* Added `write_test` and `create_test`.
* De-duplicated FAT16 and FAT32 code (https://github.com/thejpster/embedded-sdmmc-rs/issues/10)

## [Version 0.2.1](https://github.com/rust-embedded-community/embedded-sdmmc-rs/releases/tag/v0.2.1) - 2019-02-19

### Changes

* Added `readme=README.md` to `Cargo.toml`

## [Version 0.2.0](https://github.com/rust-embedded-community/embedded-sdmmc-rs/releases/tag/v0.2.0) - 2019-01-24

### Changes

* Reduce delay waiting for response. Big speed improvements.

## [Version 0.1.0](https://github.com/rust-embedded-community/embedded-sdmmc-rs/releases/tag/v0.1.1) - 2018-12-23

### Changes

* Can read blocks from an SD Card using an `embedded_hal::SPI` device and a
  `embedded_hal::OutputPin` for Chip Select.
* Can read partition tables and open a FAT32 or FAT16 formatted partition.
* Can open and iterate the root directory of a FAT16 formatted partition.
