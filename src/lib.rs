//! # embedded-sdmmc
//!
//! > An SD/MMC Library written in Embedded Rust
//!
//! This crate is intended to allow you to read/write files on a FAT formatted
//! SD card on your Rust Embedded device, as easily as using the `SdFat` Arduino
//! library. It is written in pure-Rust, is `#![no_std]` and does not use
//! `alloc` or `collections` to keep the memory footprint low. In the first
//! instance it is designed for readability and simplicity over performance.
//!
//! This crate supports both blocking and asynchronous usage via their respective
//! modules. The APIs within the modules are identical except that the asynchronous
//! module uses `async fn` where applicable. The `blocking` module uses traits from
//! the embedded_hal & embedded_io crates whereas the `asynchronous module uses
//! traits from the embedded_hal_async and embedded_io_async crates.
//!
//! ## Using the crate
//!
//! You will need something that implements the `BlockDevice` trait, which can
//! read and write the 512-byte blocks (or sectors) from your card. If you were
//! to implement this over USB Mass Storage, there's no reason this crate
//! couldn't work with a USB Thumb Drive, but we only supply a `BlockDevice`
//! suitable for reading SD and SDHC cards over SPI.
//!
//! ```rust
//! use embedded_sdmmc::blocking::{Error, Mode, SdCard, SdCardError, TimeSource, VolumeIdx, VolumeManager};
//!
//! fn example<S, D, T>(spi: S, delay: D, ts: T) -> Result<(), Error<SdCardError>>
//! where
//!     S: embedded_hal::spi::SpiDevice,
//!     D: embedded_hal::delay::DelayNs,
//!     T: TimeSource,
//! {
//!     let sdcard = SdCard::new(spi, delay);
//!     println!("Card size is {} bytes", sdcard.num_bytes()?);
//!     let volume_mgr = VolumeManager::new(sdcard, ts);
//!     let volume0 = volume_mgr.open_volume(VolumeIdx(0))?;
//!     println!("Volume 0: {:?}", volume0);
//!     let root_dir = volume0.open_root_dir()?;
//!     let mut my_file = root_dir.open_file_in_dir("MY_FILE.TXT", Mode::ReadOnly)?;
//!     while !my_file.is_eof() {
//!         let mut buffer = [0u8; 32];
//!         let num_read = my_file.read(&mut buffer)?;
//!         for b in &buffer[0..num_read] {
//!             print!("{}", *b as char);
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! For writing files:
//!
//! ```rust
//! use embedded_sdmmc::blocking::{BlockDevice, Directory, Error, Mode, TimeSource};
//! fn write_file<D: BlockDevice, T: TimeSource, const DIRS: usize, const FILES: usize, const VOLUMES: usize>(
//!     root_dir: &mut Directory<D, T, DIRS, FILES, VOLUMES>,
//! ) -> Result<(), Error<D::Error>>
//! {
//!     let my_other_file = root_dir.open_file_in_dir("MY_DATA.CSV", Mode::ReadWriteCreateOrAppend)?;
//!     my_other_file.write(b"Timestamp,Signal,Value\n")?;
//!     my_other_file.write(b"2025-01-01T00:00:00Z,TEMP,25.0\n")?;
//!     my_other_file.write(b"2025-01-01T00:00:01Z,TEMP,25.1\n")?;
//!     my_other_file.write(b"2025-01-01T00:00:02Z,TEMP,25.2\n")?;
//!     // Don't forget to flush the file so that the directory entry is updated
//!     my_other_file.flush()?;
//!     Ok(())
//! }
//! ```
//!
//! ## Features
//!
//! * `log`: Enabled by default. Generates log messages using the `log` crate.
//! * `defmt-log`: By turning off the default features and enabling the
//!   `defmt-log` feature you can configure this crate to log messages over defmt
//!   instead.
//!
//! You cannot enable both the `log` feature and the `defmt-log` feature.

#![cfg_attr(not(test), no_std)]

#[cfg(test)]
#[macro_use]
extern crate hex_literal;

#[macro_use]
mod structure;

mod common;

/// Blocking implementation of this crate. Uses traits from embedded_hal & embedded_io crates.
#[path = "."]
pub mod blocking {
    use bisync::synchronous::*;
    use embedded_hal::{delay::DelayNs, spi::SpiDevice};
    use embedded_io::{ErrorType, Read, Seek, SeekFrom, Write};
    mod inner;
    pub use inner::*;
}

/// Async implementation of this crate. Uses traits from embedded_hal_async & embedded_io_async crates.
#[path = "."]
pub mod asynchronous {
    use bisync::asynchronous::*;
    use embedded_hal_async::{delay::DelayNs, spi::SpiDevice};
    use embedded_io_async::{ErrorType, Read, Seek, SeekFrom, Write};
    mod inner;
    pub use inner::*;
}

#[cfg(all(feature = "defmt-log", feature = "log"))]
compile_error!("Cannot enable both log and defmt-log");

#[cfg(feature = "log")]
use log::{debug, trace, warn};

#[cfg(feature = "defmt-log")]
use defmt::{debug, trace, warn};

#[cfg(all(not(feature = "defmt-log"), not(feature = "log")))]
#[macro_export]
/// Like log::debug! but does nothing at all
macro_rules! debug {
    ($($arg:tt)+) => {};
}

#[cfg(all(not(feature = "defmt-log"), not(feature = "log")))]
#[macro_export]
/// Like log::trace! but does nothing at all
macro_rules! trace {
    ($($arg:tt)+) => {};
}

#[cfg(all(not(feature = "defmt-log"), not(feature = "log")))]
#[macro_export]
/// Like log::warn! but does nothing at all
macro_rules! warn {
    ($($arg:tt)+) => {};
}
