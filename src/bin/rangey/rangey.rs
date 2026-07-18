//! Ultra wideband ds-twr.

#![no_main]
#![no_std]

mod initiator;
mod responder;

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_nrf::config::Config;
use embassy_nrf::gpio::{Input, Output, Pull};
use embassy_nrf::{bind_interrupts, peripherals, spim};
use embassy_time::{Duration, Timer};
use panic_probe as _;
use qorvo_blinky::board::Board;
use qorvo_blinky::ema::Ema;
use qorvo_blinky::ultrawideband::ranging::message::{DecodedMessage, MessageType};
use qorvo_blinky::ultrawideband::transport::{
    EventLine, MAX_PHY_PACKET_SIZE, abort_receive, receive_frame,
};
use qorvo_blinky::ultrawideband::{self};

bind_interrupts!(struct Irqs {
    SPIM3 => spim::InterruptHandler<peripherals::SPI3>;
});

/// Settle time after an SW2 edge, to debounce the role switch.
const DEBOUNCE: Duration = Duration::from_millis(20);

/// The two activity LEDs (active-low), toggled per radio event.
struct Leds {
    /// Blue (D10), toggled on every receive.
    rx: Output<'static>,
    /// Green (D9), toggled on every transmit.
    tx: Output<'static>,
}

impl Leds {
    /// Blink the receive LED.
    fn on_rx(&mut self) {
        self.rx.toggle();
    }

    /// Blink the transmit LED.
    fn on_tx(&mut self) {
        self.tx.toggle();
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let _ = spawner;
    let peripherals = embassy_nrf::init(Config::default());
    let board = Board::new(peripherals);
    let spi = board.spim3_uwb.build(Irqs);
    let mut button = Input::new(board.button_sw2, Pull::Up);
    let uwb_irq = Input::new(board.uwb_irq, Pull::Down);
    let mut uwb_rst = board.uwb_rst;
    let mut event_line = EventLine::new(uwb_irq);
    let mut leds = Leds {
        rx: board.led_d10,
        tx: board.led_d09,
    };
    let mut led_role = board.led_d11;

    dw3110_hard_reset(&mut uwb_rst).await;

    let (mut dw, tx_antenna_delay) = match ultrawideband::bring_up(spi) {
        Ok(pair) => pair,
        Err(e) => {
            defmt::panic!("rangey: bring-up failed: {}", e);
        }
    };

    defmt::info!("rangey: ready on CH9/PRF64/6.8Mbps - listening (press SW2 to initiate)");

    let mut buffer = [0_u8; MAX_PHY_PACKET_SIZE];
    let mut distance_ema = Ema::default();
    loop {
        let mut dw_receiving = match dw.receive(ultrawideband::radio_config()) {
            Ok(dw_receive) => dw_receive,
            Err(e) => defmt::panic!("receiver initialization failed: {:?}", e),
        };

        match select(
            button.wait_for_low(),
            receive_frame(&mut dw_receiving, &mut event_line, &mut buffer),
        )
        .await
        {
            Either::First(()) => {
                let Ok(ready) = abort_receive(dw_receiving)
                    .map_err(|e| defmt::panic!("main: button abrt rx: {}", e));

                settle_button(&mut button).await;
                led_role.set_low();

                defmt::info!("main: SW2 pressed - initiating");
                match initiator::run_initiator(
                    ready,
                    &mut event_line,
                    &mut button,
                    &mut buffer,
                    &mut leds,
                    tx_antenna_delay,
                )
                .await
                {
                    Ok(dw_recovered) => dw = dw_recovered,
                    Err(e) => defmt::panic!("main: initiator error: {}", e),
                }

                led_role.set_high();
                settle_button(&mut button).await;
                defmt::info!("main: initiator returned to listening");
            }
            Either::Second(Ok((msg_length, poll_rx_time))) => {
                let Ok(dw_ready) = dw_receiving
                    .finish_receiving()
                    .map_err(|(_dw, e)| defmt::panic!("main: responder finish_receiving: {}", e));
                leds.on_rx();

                let poll_msg = buffer.get(..msg_length).and_then(MessageType::decode);
                match poll_msg {
                    Some(DecodedMessage::Poll { msg_id }) => {
                        match responder::respond_and_range(
                            dw_ready,
                            &mut event_line,
                            &mut buffer,
                            &mut leds,
                            &mut distance_ema,
                            msg_id,
                            poll_rx_time.value(),
                        )
                        .await
                        {
                            Ok(recovered) => dw = recovered,
                            Err(e) => defmt::panic!("main: responder respond_and_range: {}", e),
                        }
                    }
                    _ => dw = dw_ready,
                }
            }
            Either::Second(Err(e)) => defmt::panic!("main: abort_receive failed: {}", e),
        }
    }
}

async fn dw3110_hard_reset(uwb_rst: &mut Output<'_>) {
    uwb_rst.set_low();
    Timer::after_millis(2).await;
    uwb_rst.set_high();
    Timer::after_millis(2).await;
}

/// Wait for SW2 to be released, then debounce, so one press is one role toggle.
///
/// The role select races the button as a level (`wait_for_low`), which catches
/// a press even if its edge lands while the board is mid-exchange; releasing
/// before re-arming keeps that same press from immediately toggling back.
async fn settle_button(button: &mut Input<'static>) {
    button.wait_for_high().await;
    Timer::after(DEBOUNCE).await;
}

/// Hard fault handler, exits with an error status.
#[cortex_m_rt::exception]
unsafe fn HardFault(_frame: &cortex_m_rt::ExceptionFrame) -> ! {
    semihosting::process::exit(1);
}
