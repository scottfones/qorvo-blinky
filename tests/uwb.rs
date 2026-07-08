//! On-device tests for ultra wideband (DW3110).

#![no_main]
#![no_std]

use defmt_rtt as _;
use embassy_nrf::{bind_interrupts, peripherals, spim};

bind_interrupts!(struct Irqs {
    SPIM3 => spim::InterruptHandler<peripherals::SPI3>;
});

#[cfg(test)]
#[embedded_test::tests]
mod tests {
    use dw3000_ng::hl::SendTime;
    use dw3000_ng::{DW3000, Ready, Uninitialized};
    use embassy_nrf::config::Config;
    use embassy_nrf::gpio::Output;
    use embassy_time::{Delay, Duration, Instant, Timer};
    use qorvo_blinky::board::Board;
    use qorvo_blinky::board::spim3_uwb::Spim3Uwb;

    use super::Irqs;

    /// Construct the board before each test.
    #[init]
    fn init() -> Board {
        let p = embassy_nrf::init(Config::default());
        Board::new(p)
    }

    /// Reads the device id over SPI.
    #[test]
    fn read_device_id(board: Board) {
        let mut dw3000 = DW3000::new(board.spim3_uwb.build(Irqs));

        let device_id = unwrap_or_panic(dw3000.ll().dev_id().read(), "DEV_ID read");
        let ridtag = device_id.ridtag();
        defmt::info!("DW3110 RIDTAG = {=u16:#06x}", ridtag);
        defmt::assert_eq!(ridtag, 0xDECA, "not a DW3000-family device");
    }

    /// Reads the channel 5 antenna delays from OTP 0x1A.
    #[test]
    async fn read_otp_ch5_delay(board: Board) {
        let mut dw3000 = soft_reset_init(DW3000::new(board.spim3_uwb.build(Irqs))).await;

        let otp_1a = read_otp_word(&mut dw3000, 0x1A);
        let (receive_delay, transmit_delay) = split_antenna_delay(otp_1a);
        defmt::info!(
            "DW3110 OTP 0x1A CH5 antenna delay = {=u32:#010x} (rx={=u16} tx={=u16})",
            otp_1a,
            receive_delay,
            transmit_delay
        );
    }

    /// Reads the DW3110 channel 9 antenna delays from OTP 0x1C.
    #[test]
    async fn read_otp_ch9_delay(board: Board) {
        let mut dw3000 = soft_reset_init(DW3000::new(board.spim3_uwb.build(Irqs))).await;

        let otp_1c = read_otp_word(&mut dw3000, 0x1C);
        let (receive_delay, transmit_delay) = split_antenna_delay(otp_1c);
        defmt::info!(
            "DW3110 OTP 0x1C CH9 antenna delay = {=u32:#010x} (rx={=u16} tx={=u16})",
            otp_1c,
            receive_delay,
            transmit_delay
        );
        defmt::assert!(
            otp_1c != 0 && otp_1c != 0xFFFF_FFFF,
            "CH9 antenna delay OTP 0x1C is blank"
        );
        defmt::assert!(receive_delay != 0, "CH9 antenna rx delay is 0");
        defmt::assert!(transmit_delay != 0, "CH9 antenna tx delay is 0");
    }

    /// Reads the DW3110 platform id, cal revision, and OTP revision.
    #[test]
    async fn read_otp_platform_id(board: Board) {
        let mut dw3000 = soft_reset_init(DW3000::new(board.spim3_uwb.build(Irqs))).await;

        let otp_1f = read_otp_word(&mut dw3000, 0x1F);
        let [id_high, id_low, cal_revision, otp_revision] = otp_1f.to_be_bytes();
        let platform_id = u16::from_be_bytes([id_high, id_low]);
        defmt::info!(
            "DW3110 OTP 0x1F platform_id={=u16} cal_rev={=u8} otp_rev={=u8}",
            platform_id,
            cal_revision,
            otp_revision
        );
    }

    /// Confirms the `sys_time` counter advances once the chip is Ready.
    #[test]
    async fn read_sys_time(board: Board) {
        let dw3000 = soft_reset_init(DW3000::new(board.spim3_uwb.build(Irqs))).await;

        let mut dw3000 = unwrap_or_panic(
            dw3000.config(dw3000_ng::Config::default(), Delay),
            "config/PLL lock",
        );

        let t0 = read_unlatched_sys_time(&mut dw3000);
        Timer::after_millis(5).await;
        let t1 = read_unlatched_sys_time(&mut dw3000);

        let delta = t1.wrapping_sub(t0);
        defmt::info!(
            "DW3110 sys_time t0={=u32:#010x} t1={=u32:#010x} delta={=u32}",
            t0,
            t1,
            delta
        );
        defmt::assert_ne!(delta, 0, "device-time did not advance - clock not running");
    }

    /// Confirms a transmission yields TXFRS with a non-zero timestamp.
    #[test]
    async fn transmit_a_frame(board: Board) {
        let dw3000_rst = board.uwb_rst;
        let dw3000 = hard_reset_init(DW3000::new(board.spim3_uwb.build(Irqs)), dw3000_rst).await;

        let dw3000 = unwrap_or_panic(
            dw3000.config(dw3000_ng::Config::default(), Delay),
            "config/PLL lock",
        );

        let payload = [0xDE, 0xCA, 0xAA, 0x55];
        let mut sending = unwrap_or_panic(
            dw3000.send_raw(&payload, SendTime::Now, &dw3000_ng::Config::default()),
            "send_raw",
        );

        let deadline = Instant::now() + Duration::from_millis(100);
        let tx_time = loop {
            match sending.s_wait() {
                Ok(instant) => break instant,
                Err(nb::Error::WouldBlock) => {
                    defmt::assert!(Instant::now() < deadline, "TXFRS never posted");
                    Timer::after_micros(50).await;
                }
                Err(nb::Error::Other(error)) => {
                    defmt::panic!("transmit error: {}", defmt::Debug2Format(&error));
                }
            }
        };

        defmt::info!("DW3110 tx_time = {=u64:#012x}", tx_time.value());
        defmt::assert_ne!(tx_time.value(), 0, "transmit timestamp is zero");
    }

    /// Read system time after clearing the read latch.
    ///
    /// DW3000 User Manual Reference:
    /// - 8.2.2.7: Sub-register 0x00:1C – System time counter
    fn read_unlatched_sys_time(dw3000: &mut DW3000<Spim3Uwb, Ready>) -> u32 {
        // Unlatch via SPI write transaction
        unwrap_or_panic(
            dw3000.ll().sys_status().write(|w| w),
            "SYS_TIME unlatch write",
        );
        unwrap_or_panic(dw3000.sys_time(), "sys_time read")
    }

    /// Read an OTP word.
    fn read_otp_word(dw3000: &mut DW3000<Spim3Uwb, Uninitialized>, address: u16) -> u32 {
        unwrap_or_panic(dw3000.read_otp(address), "OTP read")
    }

    /// Split an OTP antenna-delay into rx and tx halves.
    const fn split_antenna_delay(word: u32) -> (u16, u16) {
        let [rx_upper, rx_lower, tx_upper, tx_lower] = word.to_be_bytes();
        let rx_delay = u16::from_be_bytes([rx_upper, rx_lower]);
        let tx_delay = u16::from_be_bytes([tx_upper, tx_lower]);
        (rx_delay, tx_delay)
    }

    async fn hard_reset_init(
        dw3000: DW3000<Spim3Uwb, Uninitialized>,
        mut dw3000_rst: Output<'static>,
    ) -> DW3000<Spim3Uwb, Uninitialized> {
        // trigger reset
        dw3000_rst.set_low();
        // trigger wait
        Timer::after_millis(2).await;
        // release wait
        dw3000_rst.set_high();
        // reset wait
        Timer::after_millis(2).await;

        unwrap_or_panic(dw3000.init(), "init")
    }
    /// Soft-reset and then init the DW3110.
    ///
    /// DW3000 User Manual References:
    /// - 8.2.15.1: Sub-register 0x11:00 - Soft reset
    /// - 8.2.15.2: Sub-register 0x11:04 – Clock control
    async fn soft_reset_init(
        mut dw3000: DW3000<Spim3Uwb, Uninitialized>,
    ) -> DW3000<Spim3Uwb, Uninitialized> {
        // First step in soft reset procedure from 7.2.15.1
        // Set `SYS_CLK` to 01
        unwrap_or_panic(
            dw3000.ll().clk_ctrl().modify(|_, w| w.sys_clk(0b01)),
            "clk_ctrl write",
        );
        // Second step in soft reset procedure from the 8.2.15.1
        // Set bits 8 through 0 to zero to force a reset
        unwrap_or_panic(
            dw3000.ll().soft_rst().write(|w| {
                w.arm_rst(0)
                    .prgn_rst(0)
                    .cia_rst(0)
                    .bist_rst(0)
                    .rx_rst(0)
                    .tx_rst(0)
                    .hif_rst(0)
                    .pmsc_rst(0)
                    .gpio_rst(0)
            }),
            "soft_rst assert",
        );
        // Third step in soft reset procedure from 8.2.15.1
        // Set bits 8 through 0 to one for  normal operation
        unwrap_or_panic(
            dw3000.ll().soft_rst().write(|w| {
                w.arm_rst(1)
                    .prgn_rst(1)
                    .cia_rst(1)
                    .bist_rst(1)
                    .rx_rst(1)
                    .tx_rst(1)
                    .hif_rst(1)
                    .pmsc_rst(1)
                    .gpio_rst(1)
            }),
            "soft_rst release",
        );

        // reset settle
        Timer::after_millis(2).await;

        unwrap_or_panic(dw3000.init(), "init")
    }

    /// Unwrap a driver `Result` or panic with the error context.
    ///
    /// The `dw3000-ng` crate errors are `Debug` but not `defmt::Format` without
    /// `defmt` feature.  The feature is left disabled to prevent introduction
    /// of older `defmt` version.
    #[track_caller]
    fn unwrap_or_panic<T, E>(result: Result<T, E>, context: &str) -> T
    where
        E: core::fmt::Debug,
    {
        result.unwrap_or_else(|error| {
            defmt::panic!("{=str}: {}", context, defmt::Debug2Format(&error))
        })
    }
}
