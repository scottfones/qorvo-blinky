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
    use embassy_nrf::pac::FICR;
    use embedded_hal_async::i2c::I2c;
    use qorvo_blinky::board::Board;
    use static_cell::ConstStaticCell;

    use super::Irqs;

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
    async fn reads_dw3110_device_id() -> Result<(), &'static str> {
        let p = embassy_nrf::init(Config::default());
        let board = Board::new(p);

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
    async fn reads_lis2dh12_device_id() -> Result<(), &'static str> {
        static RAM_BUFFER: ConstStaticCell<[u8; 4]> = ConstStaticCell::new([0; 4]);

        let p = embassy_nrf::init(Config::default());
        let board = Board::new(p);

        let twim0_bus = board.twim0.build(Irqs, RAM_BUFFER.take());

        let mut accel = I2cDevice::new(twim0_bus);
        let mut accel_buffer = [0_u8; 1];

        if let Err(e) = accel.write_read(0x19, &[0x0F], &mut accel_buffer).await {
            defmt::error!("LIS2DH12 accelerometer ID read failed: {:?}", e);
            return Err("LIS2DH12 accelerometer ID read failed.");
        }

        defmt::info!("LIS2DH12 accelerometer ID: {=[?;1]}", accel_buffer);

        if accel_buffer != [0x33] {
            return Err("Unexpected accelerometer ID");
        }
        Ok(())
    }
}
