//! DW3110 SPI components and builder.

use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::interrupt::typelevel::{Binding, SPIM3};
use embassy_nrf::spim::{self, Spim};
use embassy_nrf::{Peri, peripherals};
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};

/// SPIM3 device for the DW3110.
pub type Spim3Uwb = ExclusiveDevice<Spim<'static>, Output<'static>, NoDelay>;

/// The peripherals wired to the DW3110, before SPI construction.
pub struct Spim3UwbParts {
    /// Chip-select pin (P1.06, active-low).
    pub cs: Peri<'static, peripherals::P1_06>,
    /// MISO pin (P0.29).
    pub miso: Peri<'static, peripherals::P0_29>,
    /// MOSI pin (P0.08).
    pub mosi: Peri<'static, peripherals::P0_08>,
    /// SCK pin (P0.03).
    pub sck: Peri<'static, peripherals::P0_03>,
    /// SPIM3 peripheral instance.
    pub spim: Peri<'static, peripherals::SPI3>,
}

impl Spim3UwbParts {
    /// Construct the DW3110 SPIM3 device.
    #[must_use]
    pub fn build(
        self,
        irq: impl Binding<SPIM3, spim::InterruptHandler<peripherals::SPI3>> + 'static,
    ) -> Spim3Uwb {
        let mut config = spim::Config::default();
        config.frequency = spim::Frequency::M4; // < 7 MHz until clock is configured

        let bus = Spim::new(self.spim, irq, self.sck, self.miso, self.mosi, config);
        let chip_select = Output::new(self.cs, Level::High, OutputDrive::Standard);

        match ExclusiveDevice::new_no_delay(bus, chip_select) {
            Ok(spim3_uwb) => spim3_uwb,
            // `OutputPin::Error` is `Infallible`.
            Err(e) => match e {},
        }
    }
}
