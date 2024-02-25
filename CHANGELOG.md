# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog] and this project adheres to [Semantic Versioning].

## [Unreleased]

### Changed

- Updated to `heapless` ^0.8

## [Version 0.7.0] - 2024-02-04

### Changed

- __Breaking Change__: `Volume`, `Directory` and `File` are now smart! They hold references to the thing they were made from, and will clean themselves up when dropped. The trade-off is you can can't open multiple volumes, directories or files at the same time.
- __Breaking Change__: Renamed the old types to `RawVolume`, `RawDirectory` and `RawFile`
- __Breaking Change__: Renamed `Error::FileNotFound` to `Error::NotFound`
- Fixed long-standing bug that caused an integer overflow when a FAT32 directory was longer than one cluster ([#74])
- You can now open directories multiple times without error
- Updated to [embedded-hal] 1.0

### Added

- `RawVolume`, `RawDirectory` and `RawFile` types (like the old `Volume`, `Directory` and `File` types)
- New method `make_dir_in_dir`
- Empty strings and `"."` convert to `ShortFileName::this_dir()`
- New API `change_dir` which changes a directory to point to some child directory (or the parent) without opening a new directory.
- Updated 'shell' example to support `mkdir`, `tree` and relative/absolute paths

### Removed

* None

[#74]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/issues/74
[embedded-hal]: https://crates.io/crates/embedded-hal

## [Version 0.6.0] - 2023-10-20

### Changed

- Writing to a file no longer flushes file metadata to the Directory Entry.
  Instead closing a file now flushes file metadata to the Directory Entry.
  Requires mutable access to the Volume ([#94]).
- Files now have the correct length when modified, not appended ([#72]).
- Calling `SdCard::get_card_type` will now perform card initialisation ([#87] and [#90]).
- Removed warning about unused arguments.
- Types are now documented at the top level ([#86]).
- Renamed `Cluster` to `ClusterId` and stopped you adding two together

[#72]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/issues/72
[#86]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/issues/86
[#87]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/issues/87
[#90]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/issues/90
[#94]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/issues/94

### Added

- New examples, `append_file`, `create_file`, `delete_file`, `list_dir`, `shell`
- New test cases `tests/directories.rs`, `tests/read_file.rs`

### Removed

- __Breaking Change__: `Controller` alias for `VolumeManager` removed.
- __Breaking Change__: `VolumeManager::open_dir_entry` removed, as it was unsafe to the user to randomly pick a starting cluster.
- Old examples `create_test`, `test_mount`, `write_test`, `delete_test`

## [Version 0.5.0] - 2023-05-20

### Changed

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

## [Version 0.4.0] - 2023-01-18

### Changed

- Optionally use [defmt] s/defmt) for logging.
    Controlled by `defmt-log` feature flag.
- __Breaking Change__: Use SPI blocking traits instead to ease SPI peripheral sharing.
  See: <https://github.com/rust-embedded-community/embedded-sdmmc-rs/issues/28>
- Added `Controller::has_open_handles` and `Controller::free` methods.
- __Breaking Change__: Changed interface to enforce correct SD state at compile time.
- __Breaking Change__: Added custom error type for `File` operations.
- Fix `env_logger` pulling in the `std` feature in `log` in library builds.
- Raise the minimum supported Rust version to 1.56.0.
- Code tidy-ups and more documentation.
- Add `MAX_DIRS` and `MAX_FILES` generics to `Controller` to allow an arbitrary numbers of concurrent open directories and files.
- Add new constructor method `Controller::new_with_limits(block_device: D, timesource: T) -> Controller<D, T, MAX_DIRS, MAX_FILES>`
  to create a `Controller` with custom limits.

## [Version 0.3.0] - 2019-12-16

### Changed

- Updated to `v2` embedded-hal traits.
- Added open support for all modes.
- Added write support for files.
- Added `Info_Sector` tracking for FAT32.
- Change directory iteration to look in all the directory's clusters.
- Added `write_test` and `create_test`.
- De-duplicated FAT16 and FAT32 code (<https://github.com/thejpster/embedded-sdmmc-rs/issues/10>)

## [Version 0.2.1] - 2019-02-19

### Changed

- Added `readme=README.md` to `Cargo.toml`

## [Version 0.2.0] - 2019-01-24

### Changed

- Reduce delay waiting for response. Big speed improvements.

## [Version 0.1.0] - 2018-12-23

### Changed

- Can read blocks from an SD Card using an `embedded_hal::SPI` device and a
  `embedded_hal::OutputPin` for Chip Select.
- Can read partition tables and open a FAT32 or FAT16 formatted partition.
- Can open and iterate the root directory of a FAT16 formatted partition.

[Keep a Changelog]: http://keepachangelog.com/en/1.0.0/
[Semantic Versioning]: http://semver.org/spec/v2.0.0.html
[Unreleased]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/compare/v0.7.0...develop
[Version 0.7.0]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/compare/v0.7.0...v0.6.0
[Version 0.6.0]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/compare/v0.6.0...v0.5.0
[Version 0.5.0]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/compare/v0.5.0...v0.4.0
[Version 0.4.0]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/compare/v0.4.0...v0.3.0
[Version 0.3.0]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/compare/v0.3.0...v0.2.1
[Version 0.2.1]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/compare/v0.2.1...v0.2.0
[Version 0.2.0]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/compare/v0.2.0...v0.1.1
[Version 0.1.1]: https://github.com/rust-embedded-community/embedded-sdmmc-rs/releases/tag/v0.1.1
