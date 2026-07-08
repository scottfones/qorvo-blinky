//! On-device tests for the nrf52833.

#![no_main]
#![no_std]

use defmt_rtt as _;

#[cfg(test)]
#[embedded_test::tests]
mod tests {
    use embassy_nrf::config::Config;
    use embassy_nrf::pac::FICR;
    use qorvo_blinky::board::Board;

    /// Construct the board before each test.
    #[init]
    fn init() -> Board {
        let p = embassy_nrf::init(Config::default());
        Board::new(p)
    }

    /// Reads the factory device id.
    #[test]
    fn reads_factory_device_id() {
        let id_low = FICR.deviceid(0).read();
        let id_high = FICR.deviceid(1).read();
        defmt::info!("FICR DEVICEID = {=u32:#010x}{=u32:08x}", id_high, id_low);
        defmt::assert!(id_low != 0 || id_high != 0, "FICR device id reads all zero");
    }
}
