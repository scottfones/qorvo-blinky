//! Board support for the Qorvo DWM3001CDK.
//!
//! User LED (active-low): D9 = P0.04 (green), D10 = P0.05 (blue),
//!                       D11 = P0.22 (red)  , D12 = P0.14 (red).
//!
//! Button SW2 (`BT_WAKE_UP`) = P0.02 (active-low, pull-up).
//!
//! DW3110 over SPIM3: SCK = P0.03, MISO = P0.29, MOSI = P0.08, CS = P1.06.
//! DW3110 IRQ = P1.02 (active-high, pull-down; via Uberi `custom_board.h`).
//! DW3110 RST = P0.25 (active-low, no pull-up; via Uberi `custom_board.h`)
//!
//! LIS2DH12 accelerometer over I2C0: SDA = P0.24, SCL = P1.04, IRQ = P0.16.

pub mod spim3_uwb;
pub mod twim0;

use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::{Peri, Peripherals, peripherals};

/// Constructed board resources.
pub struct Board {
    /// LIS2DH12 data-ready interrupt (INT1, P0.16, active-high).
    pub accel_drdy: Peri<'static, peripherals::P0_16>,
    /// SW2 button (`BT_WAKE_UP`, P0.02, active-low, pull-up).
    pub button_sw2: Peri<'static, peripherals::P0_02>,
    /// User LED D9 (green, P0.04, active-low).
    pub led_d09: Output<'static>,
    /// User LED D10 (blue, P0.05, active-low).
    pub led_d10: Output<'static>,
    /// User LED D11 (red, P0.22, active-low).
    pub led_d11: Output<'static>,
    /// User LED D12 (red, P0.14, active-low).
    pub led_d12: Output<'static>,
    /// DW3110 SPI resources.
    pub spim3_uwb: spim3_uwb::Spim3UwbParts,
    /// TWIM0 resources.
    pub twim0: twim0::Twim0Parts,
    /// DW3110 IRQ (P1.02, active-high, pull-down).
    pub uwb_irq: Peri<'static, peripherals::P1_02>,
    /// DW3110 Reset (P0.25, active-low, no pull-up).
    pub uwb_rst: Output<'static>,
}

impl Board {
    /// Configure the board's peripherals from the embassy singletons.
    #[must_use]
    pub fn new(p: Peripherals) -> Self {
        let accel_drdy = p.P0_16;

        let button_sw2 = p.P0_02;

        let led_d09 = Output::new(p.P0_04, Level::High, OutputDrive::Standard);
        let led_d10 = Output::new(p.P0_05, Level::High, OutputDrive::Standard);
        let led_d11 = Output::new(p.P0_22, Level::High, OutputDrive::Standard);
        let led_d12 = Output::new(p.P0_14, Level::High, OutputDrive::Standard);

        let spim3_uwb = spim3_uwb::Spim3UwbParts {
            cs: p.P1_06,
            miso: p.P0_29,
            mosi: p.P0_08,
            sck: p.P0_03,
            spim: p.SPI3,
        };

        let twim0 = twim0::Twim0Parts {
            scl: p.P1_04,
            sda: p.P0_24,
            twim: p.TWISPI0,
        };

        let uwb_irq = p.P1_02;
        let uwb_rst = Output::new(p.P0_25, Level::High, OutputDrive::Standard0Disconnect1);

        Self {
            accel_drdy,
            button_sw2,
            led_d09,
            led_d10,
            led_d11,
            led_d12,
            spim3_uwb,
            twim0,
            uwb_irq,
            uwb_rst,
        }
    }
}
