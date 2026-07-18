use dw3000_ng::hl::SendTime;
use dw3000_ng::time::Instant as DwInstant;
use dw3000_ng::{DW3000, Ready};
use embassy_time::Duration;
use panic_probe as _;
use qorvo_blinky::board::spim3_uwb::Spim3Uwb;
use qorvo_blinky::ema::Ema;
use qorvo_blinky::ultrawideband::ranging::math::{
    REPLY_DELAY_DTU, delayed_tx_time, distance_mm, dtu_elapsed, time_of_flight_dtu,
};
use qorvo_blinky::ultrawideband::ranging::message::{DecodedMessage, MessageType};
use qorvo_blinky::ultrawideband::transport::{
    EventLine, abort_receive, receive_frame, wait_tx_done,
};
use qorvo_blinky::ultrawideband::{self, DwResult};

use crate::Leds;

/// Deadline for the initiator's final receive on the responder.
const FINAL_TIMEOUT: Duration = Duration::from_millis(50);

/// Reflect one initiator poll: send a delayed response, receive the final, and
/// compute the range and its EMA (the responder holds all six timestamps).
pub async fn respond_and_range(
    dw: DW3000<Spim3Uwb, Ready>,
    event_line: &mut EventLine,
    buffer: &mut [u8; 127],
    leds: &mut Leds,
    distance_ema: &mut Ema,
    msg_id: u8,
    poll_rx_time_u64: u64,
) -> DwResult<DW3000<Spim3Uwb, Ready>> {
    // Response (delayed from the poll's receive instant).
    let response_tx_programmed = delayed_tx_time(poll_rx_time_u64, REPLY_DELAY_DTU);
    let response_frame = MessageType::encode_response(msg_id);

    let Some(response_tx_time) = DwInstant::new(response_tx_programmed) else {
        defmt::error!("responder: delayed rx instant out of range");
        return Ok(dw);
    };

    let mut dw_sending = dw.send_raw(
        &response_frame,
        SendTime::Delayed(response_tx_time),
        &ultrawideband::radio_config(),
    )?;

    let response_tx_time_u64 = wait_tx_done(&mut dw_sending, event_line).await?.value();

    let dw = dw_sending.finish_sending().map_err(|(_dw, e)| {
        defmt::error!("responder: response finish_sending: {}", e);
        e
    })?;
    leds.on_tx();

    // Final (receive)
    let mut dw_receiving = dw.receive(ultrawideband::radio_config()).map_err(|e| {
        defmt::error!("responder: final receive: {}", e);
        e
    })?;

    match embassy_time::with_timeout(
        FINAL_TIMEOUT,
        receive_frame(&mut dw_receiving, event_line, buffer),
    )
    .await
    {
        Ok(Ok((length, final_rx_time))) => {
            let dw_ready = dw_receiving.finish_receiving().map_err(|(_dw, e)| {
                defmt::error!("responder: final finish_receiving: {}", e);
                e
            })?;

            leds.on_rx();

            let final_rx_time_u64 = final_rx_time.value();
            if let Some(DecodedMessage::Final {
                msg_id: final_id,
                time_tx_poll: time_tx_poll_u64,
                time_rx_response: time_rx_response_u64,
                time_tx_final: time_tx_final_u64,
            }) = buffer.get(..length).and_then(MessageType::decode)
            {
                if final_id == msg_id {
                    let time_round1 = dtu_elapsed(time_rx_response_u64, time_tx_poll_u64);
                    let time_reply1 = dtu_elapsed(response_tx_time_u64, poll_rx_time_u64);
                    let time_round2 = dtu_elapsed(final_rx_time_u64, response_tx_time_u64);
                    let time_reply2 = dtu_elapsed(time_tx_final_u64, time_rx_response_u64);
                    let time_of_flight =
                        time_of_flight_dtu(time_round1, time_reply1, time_round2, time_reply2);
                    let millimeters = distance_mm(time_of_flight);
                    let ema_mm = distance_ema.sample(millimeters);
                    defmt::info!(
                        "seq={=u8} dist_mm={=i64} ema_mm={=i64}",
                        final_id,
                        millimeters,
                        ema_mm
                    );
                } else {
                    defmt::warn!(
                        " final seq mismatch (got {=u8}, want {=u8})",
                        final_id,
                        msg_id
                    );
                }
            } else {
                defmt::warn!("final decode failed");
            }
            Ok(dw_ready)
        }
        Ok(Err(e)) => {
            defmt::warn!("responder: final rx: {}", e);
            abort_receive(dw_receiving)
        }
        Err(embassy_time::TimeoutError) => {
            defmt::warn!("responder: final timed out");
            abort_receive(dw_receiving)
        }
    }
}
