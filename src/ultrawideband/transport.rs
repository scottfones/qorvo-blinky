//! Ultra wideband transport types and logic.

use dw3000_ng::time::Instant as DwInstant;
use dw3000_ng::{DW3000, Ready, Sending, SingleBufferReceiving};
use embassy_futures::select::select;
use embassy_nrf::gpio::Input;
use embassy_time::{Duration, Timer};

use crate::board::spim3_uwb::Spim3Uwb;
use crate::ultrawideband::DwResult;

/// Max PHY packet size (including FCS).
pub const MAX_PHY_PACKET_SIZE: usize = 127;

/// FCS packet segment length.
const FCS_LENGTH: usize = 2;

/// Limit for one IRQ wait period.
const IRQ_RESCUE_INTERVAL: Duration = Duration::from_millis(20);

pub struct EventLine {
    pin: Input<'static>,
}

impl EventLine {
    #[must_use]
    pub const fn new(pin: Input<'static>) -> Self {
        Self { pin }
    }

    /// Wait for interrupt or rescue interval to re-poll.
    async fn wait(&mut self) {
        select(self.pin.wait_for_high(), Timer::after(IRQ_RESCUE_INTERVAL)).await;
    }
}

/// Cancel an in-flight receive and return the device to `Ready`.
///
/// # Errors
/// Returns `Err` on SPI transport failure:
/// - W1C for `sys_status` fails
/// - `finish_receiving`
pub fn abort_receive(
    mut dw_receiving: DW3000<Spim3Uwb, SingleBufferReceiving>,
) -> DwResult<DW3000<Spim3Uwb, Ready>> {
    dw_receiving.ll().sys_status().write(|w| {
        w.rxprd(1)
            .rxsfdd(1)
            .ciadone(1)
            .rxphd(1)
            .rxphe(1)
            .rxfr(1)
            .rxfcg(1)
            .rxfce(1)
            .rxfsl(1)
            .rxfto(1)
            .ciaerr(1)
            .rxovrr(1)
            .rxpto(1)
            .rxsto(1)
            .cperr(1)
            .arfe(1)
    })?;

    match dw_receiving.finish_receiving() {
        Ok(ready) => Ok(ready),
        Err((_dw, e)) => Err(e),
    }
}

/// Poll receive until it completes.
///
/// # Errors
/// Returns `Err` if:
/// - SPI transport failure
/// - Latched receive error
pub async fn receive_frame(
    dw: &mut DW3000<Spim3Uwb, SingleBufferReceiving>,
    event_line: &mut EventLine,
    buffer: &mut [u8; MAX_PHY_PACKET_SIZE],
) -> DwResult<(usize, DwInstant)> {
    loop {
        match dw.r_wait_buf(buffer) {
            Ok((length, rx_instant, _quality)) => {
                return Ok((length.saturating_sub(FCS_LENGTH), rx_instant));
            }
            Err(nb::Error::WouldBlock) => event_line.wait().await,
            Err(nb::Error::Other(e)) => return Err(e),
        }
    }
}

/// Attempt to send until transmission completes.
///
/// # Errors
/// Returns `Err` if waiting fails or would block
pub async fn wait_tx_done(
    dw: &mut DW3000<Spim3Uwb, Sending>,
    event_line: &mut EventLine,
) -> DwResult<DwInstant> {
    loop {
        match dw.s_wait() {
            Ok(tx_instant) => return Ok(tx_instant),
            Err(nb::Error::WouldBlock) => event_line.wait().await,
            Err(nb::Error::Other(e)) => return Err(e),
        }
    }
}
