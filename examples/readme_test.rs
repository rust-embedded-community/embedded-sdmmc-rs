//! This is the code from the README.md file.
//!
//! We add enough stuff to make it compile, but it won't run because our fake
//! SPI doesn't do any replies.

#![allow(dead_code)]

use core::cell::RefCell;

use embedded_sdmmc::blocking::{Error, SdCardError, TimeSource, Timestamp};

pub struct DummyCsPin;

impl embedded_hal::digital::ErrorType for DummyCsPin {
    type Error = core::convert::Infallible;
}

impl embedded_hal::digital::OutputPin for DummyCsPin {
    #[inline(always)]
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    #[inline(always)]
    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct FakeSpiBus();

impl embedded_hal::spi::ErrorType for FakeSpiBus {
    type Error = core::convert::Infallible;
}

impl embedded_hal::spi::SpiBus<u8> for FakeSpiBus {
    fn read(&mut self, _: &mut [u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn write(&mut self, _: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn transfer(&mut self, _: &mut [u8], _: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn transfer_in_place(&mut self, _: &mut [u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct FakeCs();

impl embedded_hal::digital::ErrorType for FakeCs {
    type Error = core::convert::Infallible;
}

impl embedded_hal::digital::OutputPin for FakeCs {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct FakeDelayer();

impl embedded_hal::delay::DelayNs for FakeDelayer {
    fn delay_ns(&mut self, ns: u32) {
        std::thread::sleep(std::time::Duration::from_nanos(u64::from(ns)));
    }
}

struct FakeTimesource();

impl TimeSource for FakeTimesource {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

#[derive(Debug, Clone)]
enum MyError {
    Filesystem(Error<SdCardError>),
    Disk(SdCardError),
}

impl From<Error<SdCardError>> for MyError {
    fn from(value: Error<SdCardError>) -> MyError {
        MyError::Filesystem(value)
    }
}

impl From<SdCardError> for MyError {
    fn from(value: SdCardError) -> MyError {
        MyError::Disk(value)
    }
}

fn main() -> Result<(), MyError> {
    // BEGIN Fake stuff that will be replaced with real peripherals
    let spi_bus = RefCell::new(FakeSpiBus());
    let delay = FakeDelayer();
    let sdmmc_spi = embedded_hal_bus::spi::RefCellDevice::new(&spi_bus, DummyCsPin, delay).unwrap();
    let time_source = FakeTimesource();
    // END Fake stuff that will be replaced with real peripherals

    use embedded_sdmmc::blocking::{Mode, SdCard, VolumeIdx, VolumeManager};
    // Build an SD Card interface out of an SPI device, a chip-select pin and the delay object
    let sdcard = SdCard::new(sdmmc_spi, delay);
    // Get the card size (this also triggers card initialisation because it's not been done yet)
    println!("Card size is {} bytes", sdcard.num_bytes()?);
    // Now let's look for volumes (also known as partitions) on our block device.
    // To do this we need a Volume Manager. It will take ownership of the block device.
    let volume_mgr = VolumeManager::new(sdcard, time_source);
    // Try and access Volume 0 (i.e. the first partition).
    // The volume object holds information about the filesystem on that volume.
    let volume0 = volume_mgr.open_volume(VolumeIdx(0))?;
    println!("Volume 0: {:?}", volume0);
    // Open the root directory (mutably borrows from the volume).
    let root_dir = volume0.open_root_dir()?;
    // Open a file called "MY_FILE.TXT" in the root directory
    // This mutably borrows the directory.
    let my_file = root_dir.open_file_in_dir("MY_FILE.TXT", Mode::ReadOnly)?;
    // Print the contents of the file, assuming it's in ISO-8859-1 encoding
    while !my_file.is_eof() {
        let mut buffer = [0u8; 32];
        let num_read = my_file.read(&mut buffer)?;
        for b in &buffer[0..num_read] {
            print!("{}", *b as char);
        }
    }

    Ok(())
}

// ****************************************************************************
//
// End Of File
//
// ****************************************************************************
