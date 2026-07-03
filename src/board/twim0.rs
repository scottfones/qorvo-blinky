//! TWIM0 components and builder.
//!
//! Devices:
//!     LIS2DH12 accelerometer (reg 0x19).

use embassy_nrf::interrupt::typelevel::{Binding, TWISPI0};
use embassy_nrf::twim::{self, Twim};
use embassy_nrf::{Peri, peripherals};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use static_cell::StaticCell;

/// Shared I2C0 bus.
pub type Twim0Bus = Mutex<NoopRawMutex, Twim<'static>>;

/// Components for TWIM0.
pub struct Twim0Parts {
    /// SCL clock pin (P1.04).
    pub scl: Peri<'static, peripherals::P1_04>,
    /// SDA data pin (P0.24).
    pub sda: Peri<'static, peripherals::P0_24>,
    /// TWISPI0 (TWIM0) peripheral instance.
    pub twim: Peri<'static, peripherals::TWISPI0>,
}

impl Twim0Parts {
    /// Construct the TWIM0 device.
    #[must_use]
    pub fn build(
        self,
        irq: impl Binding<TWISPI0, twim::InterruptHandler<peripherals::TWISPI0>> + 'static,
        buffer: &'static mut [u8],
    ) -> &'static Twim0Bus {
        static BUS: StaticCell<Twim0Bus> = StaticCell::new();

        let mut config = twim::Config::default();
        config.sda_pullup = true;
        config.scl_pullup = true;
        let twim = Twim::new(self.twim, irq, self.sda, self.scl, config, buffer);

        BUS.init(Mutex::new(twim))
    }
}
