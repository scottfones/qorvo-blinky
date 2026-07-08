//! On-device tests for the accelerometer (LIS2DH12).

#![no_main]
#![no_std]

use defmt_rtt as _;
use embassy_nrf::{bind_interrupts, peripherals, twim};

bind_interrupts!(struct Irqs {
    TWISPI0 => twim::InterruptHandler<peripherals::TWISPI0>;
});

#[cfg(test)]
#[embedded_test::tests]
mod tests {
    use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
    use embassy_nrf::config::Config;
    use embassy_nrf::gpio::{Input, Pull};
    use embedded_hal_async::i2c::I2c;
    use qorvo_blinky::accel;
    use qorvo_blinky::board::Board;
    use static_cell::ConstStaticCell;

    use super::Irqs;

    /// Construct the board before each test.
    #[init]
    fn init() -> Board {
        let p = embassy_nrf::init(Config::default());
        Board::new(p)
    }

    /// Reads the LIS2DH12 `WHO_AM_I` (0x0F) over I2C and confirms its identity.
    #[test]
    async fn reads_lis2dh12_device_id(board: Board) {
        static RAM_BUFFER: ConstStaticCell<[u8; 4]> = ConstStaticCell::new([0; 4]);

        let twim0_bus = board.twim0.build(Irqs, RAM_BUFFER.take());
        let mut i2c_dev = I2cDevice::new(twim0_bus);
        let mut accel_buffer = [0_u8; 1];

        defmt::unwrap!(
            i2c_dev.write_read(0x19, &[0x0F], &mut accel_buffer).await,
            "LIS2DH12 WHO_AM_I read failed"
        );
        defmt::info!("LIS2DH12 accelerometer ID: {=[?;1]}", accel_buffer);
        defmt::assert_eq!(accel_buffer, [0x33], "unexpected accelerometer id");
    }

    /// Reads the LIS2DH12 by polling and reports orientation.
    #[test]
    async fn reads_lis2dh12_by_polling(board: Board) {
        static RAM_BUFFER: ConstStaticCell<[u8; 4]> = ConstStaticCell::new([0; 4]);

        let twim0_bus = board.twim0.build(Irqs, RAM_BUFFER.take());
        let mut i2c_dev = I2cDevice::new(twim0_bus);

        defmt::unwrap!(i2c_dev.write(accel::ADDR, &[accel::CTRL_REG1, 0x57]).await);
        defmt::unwrap!(i2c_dev.write(accel::ADDR, &[accel::CTRL_REG4, 0x88]).await);

        let mut accel_status = [0_u8; 1];
        loop {
            defmt::unwrap!(
                i2c_dev
                    .write_read(accel::ADDR, &[accel::STATUS_REG], &mut accel_status)
                    .await
            );
            if let [flag] = accel_status
                && flag & accel::STATUS_ZYXDA != 0
            {
                break;
            }
        }

        read_acceleration(&mut i2c_dev, "polled").await;
    }

    /// Reads the LIS2DH12 by interrupt and reports orientation.
    #[test]
    async fn reads_lis2dh12_by_interrupt(board: Board) {
        static RAM_BUFFER: ConstStaticCell<[u8; 4]> = ConstStaticCell::new([0; 4]);

        let twim0_bus = board.twim0.build(Irqs, RAM_BUFFER.take());
        let mut i2c_dev = I2cDevice::new(twim0_bus);
        let mut accel_drdy = Input::new(board.accel_drdy, Pull::None);

        defmt::unwrap!(i2c_dev.write(accel::ADDR, &[accel::CTRL_REG1, 0x57]).await);
        defmt::unwrap!(i2c_dev.write(accel::ADDR, &[accel::CTRL_REG4, 0x80]).await);
        defmt::unwrap!(
            i2c_dev
                .write(accel::ADDR, &[accel::CTRL_REG3, accel::CTRL_REG3_I1_ZYXDA])
                .await
        );

        accel_drdy.wait_for_high().await;

        read_acceleration(&mut i2c_dev, "interrupt").await;
    }

    /// Read one X/Y/Z sample and asserts ~1 g magnitude.
    async fn read_acceleration<I>(i2c_dev: &mut I, label: &str)
    where
        I: I2c,
        I::Error: defmt::Format,
    {
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

        let x = accel::FullScale::G2.milli_g(x_raw);
        let y = accel::FullScale::G2.milli_g(y_raw);
        let z = accel::FullScale::G2.milli_g(z_raw);
        defmt::info!(
            "{=str}  raw x={=i16:#06x} y={=i16:#06x} z={=i16:#06x}  mg x={=i32} y={=i32} z={=i32}",
            label,
            x_raw,
            y_raw,
            z_raw,
            x,
            y,
            z
        );

        let magnitude_sq = x * x + y * y + z * z;
        defmt::info!("magnitude_sq: {=i32}", magnitude_sq);
        defmt::assert!(
            (700 * 700..=1300 * 1300).contains(&magnitude_sq),
            "acceleration magnitude not ~1 g at rest"
        );
    }
}
