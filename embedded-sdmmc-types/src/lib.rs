//! # Embedded SD/MMC Types Library
//!
//! This library contains some common types and abstractions required for implementing
//! support for `embedded-sdmmc` inside your hardware abstraction layer library.
//!
//! This also allows HAL library to only depend on this library instead of the higher-level
//! `embedded-sdmmc` which might have more frequent breaking changes.
//!
//! Adding `embedded-sdmmc` support for you SD card structure only involves implementing
//! the [BlockDevice] trait for your SD card structure. Once you have an initialized SD card
//! driver structure, implementing this trait is usually relatively easy.
//!
//! The [sdcard] module provides various data structures which are useful for implementing
//! the SD card initialization sequence on a system with a dedicated SD card controller.
//!
//! The [`sd_card_init` example](https://github.com/rust-embedded-community/embedded-sdmmc-rs/blob/develop/examples/sd_card_init.rs)
//! inside the `embedded-sdmmc` library provides an example of how the implementation of both
//! the initialization sequence and the [BlockDevice] implementation could look like.
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]

pub mod blockdevice;
pub mod sdcard;

pub use blockdevice::{Block, BlockCount, BlockDevice, BlockIdx};

#[cfg(test)]
mod tests {}
