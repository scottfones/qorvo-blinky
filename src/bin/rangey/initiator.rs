use dw3000_ng::hl::SendTime;
use dw3000_ng::time::Instant as DwInstant;
use dw3000_ng::{DW3000, Ready};
use embassy_futures::select::{Either, select};
use embassy_nrf::gpio::Input;
use embassy_time::{Duration, Timer};
use panic_probe as _;
use qorvo_blinky::board::spim3_uwb::Spim3Uwb;
use qorvo_blinky::ultrawideband::ranging::math::{
    REPLY_DELAY_DTU, delayed_tx_time, predicted_tx_time,
};
use qorvo_blinky::ultrawideband::ranging::message::{DecodedMessage, MessageType};
use qorvo_blinky::ultrawideband::transport::{
    EventLine, abort_receive, receive_frame, wait_tx_done,
};
use qorvo_blinky::ultrawideband::{self, DwResult};

use crate::Leds;

/// Pace between initiator exchanges (about 20 Hz).
const INITIATOR_PACE: Duration = Duration::from_millis(40);

/// Deadline for the initiator's response receive.
const RESPONSE_TIMEOUT: Duration = Duration::from_millis(50);

/// Stream initiator exchanges until SW2 is pressed again, then return the
/// device so the caller can resume listening.
pub async fn run_initiator(
    mut dw: DW3000<Spim3Uwb, Ready>,
    event_line: &mut EventLine,
    button: &mut Input<'static>,
    buffer: &mut [u8; 127],
    leds: &mut Leds,
    tx_antenna_delay: u16,
) -> DwResult<DW3000<Spim3Uwb, Ready>> {
    let mut msg_id = 0_u8;
    loop {
        msg_id = msg_id.wrapping_add(1);
        dw = initiator_exchange(dw, event_line, buffer, leds, msg_id, tx_antenna_delay).await?;
        if matches!(
            select(button.wait_for_low(), Timer::after(INITIATOR_PACE)).await,
            Either::First(())
        ) {
            return Ok(dw);
        }
    }
}

pub async fn initiator_exchange(
    dw: DW3000<Spim3Uwb, Ready>,
    event_line: &mut EventLine,
    buffer: &mut [u8; 127],
    leds: &mut Leds,
    msg_id: u8,
    tx_antenna_delay: u16,
) -> DwResult<DW3000<Spim3Uwb, Ready>> {
    // Poll (immediate).
    let poll_frame = MessageType::encode_poll(msg_id);

    let mut dw_sending = dw.send_raw(&poll_frame, SendTime::Now, &ultrawideband::radio_config())?;

    let poll_tx_time_u64 = wait_tx_done(&mut dw_sending, event_line).await?.value();

    let dw = dw_sending.finish_sending().map_err(|(_dw, e)| {
        defmt::error!("initiator: poll finish_sending: {}", e);
        e
    })?;
    leds.on_tx();

    // Response (the responder sent it delayed).
    let mut dw_receiving = dw.receive(ultrawideband::radio_config())?;

    let (dw, response_rx_time_u64) = match embassy_time::with_timeout(
        RESPONSE_TIMEOUT,
        receive_frame(&mut dw_receiving, event_line, buffer),
    )
    .await
    {
        Ok(Ok((msg_length, response_rx_time))) => {
            let dw_ready = dw_receiving.finish_receiving().map_err(|(_dw, e)| {
                defmt::error!("initiator: response finish_receiving: {}", e);
                e
            })?;

            let matched = buffer
                .get(..msg_length)
                .and_then(MessageType::decode)
                .is_some_and(|response| {
                    if let DecodedMessage::Response { msg_id: rsp_id } = response {
                        rsp_id == msg_id
                    } else {
                        false
                    }
                });

            if !matched {
                defmt::warn!(
                    "initiator: response rx frame mismatch (len={=usize} first={=[u8]:02x})",
                    msg_length,
                    buffer.get(..msg_length.min(4)).unwrap_or(&[])
                );
                return Ok(dw_ready);
            }
            leds.on_rx();
            (dw_ready, response_rx_time.value())
        }
        Ok(Err(e)) => {
            defmt::warn!("initiator: response rx: {}", e);
            return abort_receive(dw_receiving);
        }
        Err(embassy_time::TimeoutError) => {
            defmt::warn!("initiator: response timed out");
            return abort_receive(dw_receiving);
        }
    };

    // Final (delayed, final transmit time must be known before it is sent).
    let final_tx_programmed = delayed_tx_time(response_rx_time_u64, REPLY_DELAY_DTU);
    let final_tx_predicted = predicted_tx_time(final_tx_programmed, u64::from(tx_antenna_delay));
    let final_frame = MessageType::encode_final(
        msg_id,
        poll_tx_time_u64,
        response_rx_time_u64,
        final_tx_predicted,
    );

    let Some(final_tx_time) = DwInstant::new(final_tx_programmed) else {
        defmt::error!("initiator: delayed tx instant out of range");
        return Ok(dw);
    };

    let mut dw_sending = dw
        .send_raw(
            &final_frame,
            SendTime::Delayed(final_tx_time),
            &ultrawideband::radio_config(),
        )
        .inspect_err(|e| {
            defmt::error!("initiator: final send_raw: {}", e);
        })?;

    match wait_tx_done(&mut dw_sending, event_line).await {
        Ok(final_tx_actual) => {
            leds.on_tx();
            if final_tx_actual.value() != final_tx_predicted {
                defmt::warn!(
                    "initiator: final tx prediction off (pred={=u64} actual={=u64})",
                    final_tx_predicted,
                    final_tx_actual.value()
                );
            }
        }
        Err(e) => defmt::warn!("initiator: final tx error: {}", e),
    }

    dw_sending.finish_sending().map_err(|(_dw, e)| {
        defmt::error!("initiator: final finish_sending: {}", e);
        e
    })
}
