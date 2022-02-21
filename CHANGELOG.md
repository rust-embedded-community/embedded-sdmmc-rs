# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]
- Optionally use [defmt](https://github.com/knurling-rs/defmt) for logging.
    Controlled by `defmt-log` feature flag.

### Changes
- Code tidy-ups and more documentation.
- [breaking-change] Use SPI blocking traits instead to ease SPI peripheral sharing.
  See: https://github.com/rust-embedded-community/embedded-sdmmc-rs/issues/28
- Added `Controller::has_open_handles` and `Controller::free` methods.
- [breaking-change] Changed interface to enforce correct SD state at compile time.

[Unreleased]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/compare/v0.3.0...develop

## [Version 0.3.0](https://github.com/rust-embedded-community/embedded-sdmmc-rs/releases/tag/v0.3.0)

### Changes

* Updated to `v2` embedded-hal traits.
* Added open support for all modes.
* Added write support for files.
* Added `Info_Sector` tracking for FAT32.
* Change directory iteration to look in all the directory's clusters.
* Added `write_test` and `create_test`.
* De-duplicated FAT16 and FAT32 code (https://github.com/thejpster/embedded-sdmmc-rs/issues/10)

## [Version 0.2.1](https://github.com/rust-embedded-community/embedded-sdmmc-rs/releases/tag/v0.2.1)

### Changes

* Added `readme=README.md` to `Cargo.toml`

## [Version 0.2.0](https://github.com/rust-embedded-community/embedded-sdmmc-rs/releases/tag/v0.2.0)

### Changes

* Reduce delay waiting for response. Big speed improvements.

## [Version 0.1.0](https://github.com/rust-embedded-community/embedded-sdmmc-rs/releases/tag/v0.1.1)

### Changes

* Can read blocks from an SD Card using an `embedded_hal::SPI` device and a
  `embedded_hal::OutputPin` for Chip Select.
* Can read partition tables and open a FAT32 or FAT16 formatted partition.
* Can open and iterate the root directory of a FAT16 formatted partition.

