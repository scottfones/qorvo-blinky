//! On-device integration tests.

#![no_main]
#![no_std]

use defmt_rtt as _;
use embassy_nrf::{bind_interrupts, peripherals, spim, twim};

bind_interrupts!(struct Irqs {
    SPIM3 => spim::InterruptHandler<peripherals::SPI3>;
    TWISPI0 => twim::InterruptHandler<peripherals::TWISPI0>;
});

#[cfg(test)]
#[embedded_test::tests]
mod tests {
    use dw3000_ng::DW3000;
    use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
    use embassy_nrf::config::Config;
    use embassy_nrf::gpio::{Input, Pull};
    use embassy_nrf::pac::FICR;
    use embedded_hal_async::i2c::I2c;
    use qorvo_blinky::accel;
    use qorvo_blinky::board::Board;
    use static_cell::ConstStaticCell;

    use super::Irqs;

    /// Construct the board fresh before each test and hand it over.
    #[init]
    fn init() -> Board {
        let p = embassy_nrf::init(Config::default());
        Board::new(p)
    }

    /// Reads the factory device id and confirms the silicon responds.
    #[test]
    fn reads_factory_device_id() {
        let id_low = FICR.deviceid(0).read();
        let id_high = FICR.deviceid(1).read();
        defmt::info!("FICR DEVICEID = {=u32:#010x}{=u32:08x}", id_high, id_low);
        assert!(id_low != 0 || id_high != 0);
    }

    /// Reads the DW3110 device id over SPI and confirms the DW3000 family tag.
    #[test]
    async fn reads_dw3110_device_id(board: Board) -> Result<(), &'static str> {
        let spim3_uwb = board.spim3_uwb.build(Irqs);
        let mut dw3000 = DW3000::new(spim3_uwb);

        let Ok(device_id) = dw3000.ll().dev_id().read().await else {
            return Err("DW3110 DEV_ID read failed");
        };

        let ridtag = device_id.ridtag();
        defmt::info!("DW3110 RIDTAG = {=u16:#06x}", ridtag);

        if ridtag != 0xDECA {
            return Err("unexpected RIDTAG: not a DW3000-family device");
        }
        Ok(())
    }

    /// Reads the LIS2DH12 `WHO_AM_I` (0x0F) over I2C and confirms the
    /// accelerometer identity.
    #[test]
    async fn reads_lis2dh12_device_id(board: Board) -> Result<(), &'static str> {
        static RAM_BUFFER: ConstStaticCell<[u8; 4]> = ConstStaticCell::new([0; 4]);

        let twim0_bus = board.twim0.build(Irqs, RAM_BUFFER.take());

        let mut i2c_dev = I2cDevice::new(twim0_bus);
        let mut accel_buffer = [0_u8; 1];

        if let Err(e) = i2c_dev.write_read(0x19, &[0x0F], &mut accel_buffer).await {
            defmt::error!("LIS2DH12 accelerometer ID read failed: {:?}", e);
            return Err("LIS2DH12 accelerometer ID read failed.");
        }

        defmt::info!("LIS2DH12 accelerometer ID: {=[?;1]}", accel_buffer);

        if accel_buffer != [0x33] {
            return Err("Unexpected accelerometer ID");
        }
        Ok(())
    }

    /// Reads the LIS2DH12 by polling and reports orientation.
    #[test]
    async fn reads_lis2dh12_by_polling(board: Board) -> Result<(), &'static str> {
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

        read_acceleration(&mut i2c_dev, "polled").await
    }

    /// Reads the LIS2DH12 by interrupt and reports orientation.
    #[test]
    async fn reads_lis2dh12_by_interrupt(board: Board) -> Result<(), &'static str> {
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

        read_acceleration(&mut i2c_dev, "interrupt").await
    }

    /// Read one X/Y/Z sample, log it under `label`, and confirm ~1 g at rest.
    async fn read_acceleration<I>(i2c_dev: &mut I, label: &str) -> Result<(), &'static str>
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

        let x = accel::milli_g(x_raw);
        let y = accel::milli_g(y_raw);
        let z = accel::milli_g(z_raw);
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
        if !(700 * 700..=1300 * 1300).contains(&magnitude_sq) {
            return Err("acceleration magnitude not ~1 g at rest");
        }
        Ok(())
    }
}
