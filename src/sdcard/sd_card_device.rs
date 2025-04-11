//! SD card device trait and provided implementations.

use core::cell::RefCell;

use embedded_hal::{
    digital::OutputPin,
    spi::{Operation, SpiBus},
};

/// Trait for SD cards connected via SPI.
pub trait SdCardDevice {
    /// Perform a transaction against the device.
    ///
    /// This is similar to [`embedded_hal::spi::SpiDevice::transaction`], except that this sends
    /// a dummy `0xFF` byte to the device after deasserting the CS pin but before unlocking the
    /// bus.
    fn transaction(
        &mut self,
        operations: &mut [Operation<'_, u8>],
    ) -> Result<(), SdCardDeviceError>;

    /// Send 80 clock pulses to the device with CS deasserted.
    fn send_clock_pulses(&mut self) -> Result<(), SdCardDeviceError>;

    /// Do a read within a transaction.
    ///
    /// This is a convenience method equivalent to `device.transaction(&mut [Operation::Read(buf)])`.
    ///
    /// See also: [`SdCardDevice::transaction`], [`embedded_hal::spi::SpiBus::read`]
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> Result<(), SdCardDeviceError> {
        self.transaction(&mut [Operation::Read(buf)])
    }

    /// Do a write within a transaction.
    ///
    /// This is a convenience method equivalent to `device.transaction(&mut [Operation::Write(buf)])`.
    ///
    /// See also: [`SdCardDevice::transaction`], [`embedded_hal::spi::SpiBus::write`]
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result<(), SdCardDeviceError> {
        self.transaction(&mut [Operation::Write(buf)])
    }

    /// Do a transfer within a transaction.
    ///
    /// This is a convenience method equivalent to `device.transaction(&mut [Operation::Transfer(read, write)]`.
    ///
    /// See also: [`SdCardDevice::transaction`], [`embedded_hal::spi::SpiBus::transfer`]
    #[inline]
    fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), SdCardDeviceError> {
        self.transaction(&mut [Operation::Transfer(read, write)])
    }

    /// Do an in-place transfer within a transaction.
    ///
    /// This is a convenience method equivalent to `device.transaction(&mut [Operation::TransferInPlace(buf)]`.
    ///
    /// See also: [`SdCardDevice::transaction`], [`embedded_hal::spi::SpiBus::transfer_in_place`]
    #[inline]
    fn transfer_in_place(&mut self, buf: &mut [u8]) -> Result<(), SdCardDeviceError> {
        self.transaction(&mut [Operation::TransferInPlace(buf)])
    }
}

/// Errors that can occur when using the [`SdCardDevice`].
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "defmt-log", derive(defmt::Format))]
#[non_exhaustive]
pub enum SdCardDeviceError {
    /// An operation on the inner SPI bus failed.
    Spi,
    /// Setting the value of the Chip Select (CS) pin failed.
    Cs,
}

impl<BUS, CS> SdCardDevice for (&RefCell<BUS>, CS)
where
    BUS: SpiBus,
    CS: OutputPin,
{
    fn transaction(
        &mut self,
        operations: &mut [Operation<'_, u8>],
    ) -> Result<(), SdCardDeviceError> {
        let (bus, cs) = self;
        let mut bus = bus.borrow_mut();
        bus_transaction(&mut *bus, cs, operations)
    }

    fn send_clock_pulses(&mut self) -> Result<(), SdCardDeviceError> {
        let (bus, cs) = self;
        let mut bus = bus.borrow_mut();
        send_clock_pulses(&mut *bus, cs)
    }
}

#[cfg(feature = "embassy-sync-06")]
impl<CS, BUS, M> SdCardDevice for (&embassy_sync_06::blocking_mutex::Mutex<M, RefCell<BUS>>, CS)
where
    CS: OutputPin,
    BUS: SpiBus,
    M: embassy_sync_06::blocking_mutex::raw::RawMutex,
{
    fn transaction(
        &mut self,
        operations: &mut [Operation<'_, u8>],
    ) -> Result<(), SdCardDeviceError> {
        let (bus, cs) = self;
        bus.lock(|bus| {
            let mut bus = bus.borrow_mut();
            bus_transaction(&mut *bus, cs, operations)
        })
    }

    fn send_clock_pulses(&mut self) -> Result<(), SdCardDeviceError> {
        let (bus, cs) = self;
        bus.lock(|bus| {
            let mut bus = bus.borrow_mut();
            send_clock_pulses(&mut *bus, cs)
        })
    }
}

// `ExclusiveDevice` represents exclusive access to the bus so there's no need to send the dummy
// byte after deasserting the CS pin. We can delegate the implementation to the `embedded_hal` trait.
#[cfg(feature = "embedded-hal-bus-03")]
impl<CS, BUS, D> SdCardDevice for embedded_hal_bus_03::spi::ExclusiveDevice<BUS, CS, D>
where
    BUS: SpiBus,
    CS: OutputPin,
    D: embedded_hal::delay::DelayNs,
{
    fn transaction(
        &mut self,
        operations: &mut [Operation<'_, u8>],
    ) -> Result<(), SdCardDeviceError> {
        <Self as embedded_hal::spi::SpiDevice>::transaction(self, operations)
            .map_err(|_| SdCardDeviceError::Spi)
    }

    fn send_clock_pulses(&mut self) -> Result<(), SdCardDeviceError> {
        let bus = self.bus_mut();

        // There's no way to access the CS pin here so we can't set it high. Most likely it is already high so this is probably fine(?)

        let send_res = bus.write(&[0xFF; 10]);

        // On failure, it's important to still flush.
        let flush_res = bus.flush().map_err(|_| SdCardDeviceError::Spi);

        send_res.map_err(|_| SdCardDeviceError::Spi)?;
        flush_res.map_err(|_| SdCardDeviceError::Spi)?;
        Ok(())
    }
}

/// Perform a transaction against the device. This sends a dummy `0xFF` byte to the device after
/// deasserting the CS pin but before unlocking the bus.
fn bus_transaction<BUS, CS>(
    bus: &mut BUS,
    cs: &mut CS,
    operations: &mut [Operation<'_, u8>],
) -> Result<(), SdCardDeviceError>
where
    BUS: SpiBus,
    CS: OutputPin,
{
    cs.set_low().map_err(|_| SdCardDeviceError::Cs)?;

    let op_res = operations.iter_mut().try_for_each(|op| match op {
        Operation::Read(buf) => bus.read(buf),
        Operation::Write(buf) => bus.write(buf),
        Operation::Transfer(read, write) => bus.transfer(read, write),
        Operation::TransferInPlace(buf) => bus.transfer_in_place(buf),
        Operation::DelayNs(_) => {
            // We don't use delays in SPI transations in this crate so it fine to panic here.
            panic!("Tried to use a delay in a SPI transaction. This is a bug in embedded-sdmmc.")
        }
    });

    // On failure, it's important to still flush and deassert CS.
    let flush_res = bus.flush();
    let cs_res = cs.set_high();

    op_res.map_err(|_| SdCardDeviceError::Spi)?;
    flush_res.map_err(|_| SdCardDeviceError::Spi)?;
    cs_res.map_err(|_| SdCardDeviceError::Cs)?;

    // Write the dummy byte
    let dummy_res = bus.write(&[0xFF]);
    let flush_res = bus.flush();

    dummy_res.map_err(|_| SdCardDeviceError::Spi)?;
    flush_res.map_err(|_| SdCardDeviceError::Spi)?;

    Ok(())
}

/// Send 80 clock pulses to the device with CS deasserted. This is needed to initialize the SD card.
fn send_clock_pulses<BUS, CS>(bus: &mut BUS, cs: &mut CS) -> Result<(), SdCardDeviceError>
where
    BUS: SpiBus,
    CS: OutputPin,
{
    cs.set_high().map_err(|_| SdCardDeviceError::Cs)?;
    let send_res = bus.write(&[0xFF; 10]);

    // On failure, it's important to still flush.
    let flush_res = bus.flush().map_err(|_| SdCardDeviceError::Spi);

    send_res.map_err(|_| SdCardDeviceError::Spi)?;
    flush_res.map_err(|_| SdCardDeviceError::Spi)?;

    Ok(())
}
