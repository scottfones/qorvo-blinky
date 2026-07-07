//! Logs accelerometer data.

#![no_main]
#![no_std]

use defmt_rtt as _;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_nrf::config::Config;
use embassy_nrf::gpio::{Input, Pull};
use embassy_nrf::peripherals::P0_16;
use embassy_nrf::twim::Twim;
use embassy_nrf::{Peri, bind_interrupts, peripherals, twim};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embedded_hal_async::i2c::I2c;
use panic_probe as _;
use qorvo_blinky::accel;
use qorvo_blinky::board::Board;
use static_cell::ConstStaticCell;

static RAM_BUFFER: ConstStaticCell<[u8; 4]> = ConstStaticCell::new([0; 4]);

/// The accelerometer measurement configuration: 10 Hz, +/-2 g, 12-bit.
const ACCEL_CONFIG: accel::Config = accel::Config {
    data_rate: accel::OutputDataRate::Hz10,
    full_scale: accel::FullScale::G2,
    resolution: accel::Resolution::Bits12,
};

/// The shared-bus I2C handle this binary uses to reach the accelerometer.
type AccelI2c = I2cDevice<'static, NoopRawMutex, Twim<'static>>;

bind_interrupts!(struct Irqs {
    TWISPI0 => twim::InterruptHandler<peripherals::TWISPI0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_nrf::init(Config::default());
    let board = Board::new(p);
    let twim0_bus = board.twim0.build(Irqs, RAM_BUFFER.take());
    let i2c_dev = I2cDevice::new(twim0_bus);

    defmt::info!("DWM3001CDK orienty");
    match accel_task(board.accel_drdy, i2c_dev) {
        Ok(token) => spawner.spawn(token),
        Err(e) => defmt::error!("Acceleration task failed to spawn: {:?}", e),
    }
}

#[embassy_executor::task]
async fn accel_task(accel_drdy: Peri<'static, P0_16>, mut i2c_dev: AccelI2c) {
    defmt::unwrap!(
        i2c_dev
            .write(accel::ADDR, &[accel::CTRL_REG1, ACCEL_CONFIG.ctrl_reg1()])
            .await
    );
    defmt::unwrap!(
        i2c_dev
            .write(accel::ADDR, &[accel::CTRL_REG4, ACCEL_CONFIG.ctrl_reg4()])
            .await
    );
    defmt::unwrap!(
        i2c_dev
            .write(accel::ADDR, &[accel::CTRL_REG3, accel::CTRL_REG3_I1_ZYXDA])
            .await
    );

    let mut accel_drdy = Input::new(accel_drdy, Pull::None);
    loop {
        accel_drdy.wait_for_high().await;
        read_acceleration(&mut i2c_dev).await;
    }
}

/// Read one X/Y/Z sample and log it in milli-g.
async fn read_acceleration(i2c_dev: &mut AccelI2c) {
    let mut accel_sample = [0_u8; 6];
    defmt::unwrap!(
        i2c_dev
            .write_read(
                accel::ADDR,
                &[accel::AUTO_INCREMENT | accel::OUT_X_L],
                &mut accel_sample,
            )
            .await
    );
    let [x_low, x_high, y_low, y_high, z_low, z_high] = accel_sample;
    let x_raw = i16::from_le_bytes([x_low, x_high]);
    let y_raw = i16::from_le_bytes([y_low, y_high]);
    let z_raw = i16::from_le_bytes([z_low, z_high]);

    let x = ACCEL_CONFIG.milli_g(x_raw);
    let y = ACCEL_CONFIG.milli_g(y_raw);
    let z = ACCEL_CONFIG.milli_g(z_raw);
    defmt::info!("milli-g x:{=i32} y:{=i32} z:{=i32}", x, y, z);
}

/// Hard fault handler, exits with an error status.
#[cortex_m_rt::exception]
unsafe fn HardFault(_frame: &cortex_m_rt::ExceptionFrame) -> ! {
    semihosting::process::exit(1);
}
