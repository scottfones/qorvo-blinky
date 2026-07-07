//! Blinks user LEDs.

#![no_main]
#![no_std]

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::config::Config;
use embassy_nrf::gpio::Output;
use embassy_time::{Duration, Ticker};
use panic_probe as _;
use qorvo_blinky::board::Board;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_nrf::init(Config::default());
    let devices = Board::new(p);
    let led_09 = devices.led_d09;
    let led_10 = devices.led_d10;
    let led_11 = devices.led_d11;
    let led_12 = devices.led_d12;

    defmt::info!("DWM3001CDK blinky");
    match blinky_task(led_09, led_10, led_11, led_12) {
        Ok(token) => spawner.spawn(token),
        Err(e) => defmt::error!("Blinky task failed to spawn: {:?}", e),
    }
}

#[embassy_executor::task]
async fn blinky_task(
    mut led_09: Output<'static>,
    mut led_10: Output<'static>,
    mut led_11: Output<'static>,
    mut led_12: Output<'static>,
) {
    let mut ticker = Ticker::every(Duration::from_secs(1));
    loop {
        defmt::info!("Green");
        led_12.set_high(); // OFF
        led_09.set_low(); // active-low: LOW = ON
        ticker.next().await;
        defmt::info!("Blue");
        led_09.set_high(); // OFF
        led_10.set_low(); // active-low: LOW = ON
        ticker.next().await;
        defmt::info!("Red");
        led_10.set_high(); // OFF
        led_11.set_low(); // active-low: LOW = ON
        ticker.next().await;
        defmt::info!("Red");
        led_11.set_high(); // OFF
        led_12.set_low(); // active-low: LOW = ON
        ticker.next().await;
    }
}

/// Hard fault handler, exits with an error status.
#[cortex_m_rt::exception]
unsafe fn HardFault(_frame: &cortex_m_rt::ExceptionFrame) -> ! {
    semihosting::process::exit(1);
}
