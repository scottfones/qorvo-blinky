//! Blinks user LEDs.

#![no_main]
#![no_std]

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::config::Config;
use embassy_time::Timer;
use panic_probe as _;
use qorvo_blinky::board::Board;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let _ = spawner;
    let p = embassy_nrf::init(Config::default());
    let devices = Board::new(p);
    let mut led_09 = devices.led_d09;
    let mut led_10 = devices.led_d10;
    let mut led_11 = devices.led_d11;
    let mut led_12 = devices.led_d12;

    defmt::info!("DWM3001CDK blinky");
    loop {
        led_09.set_low(); // active-low: LOW = ON
        led_12.set_high(); // OFF
        Timer::after_millis(1000).await;
        led_09.set_high(); // OFF
        led_10.set_low(); // active-low: LOW = ON
        Timer::after_millis(1000).await;
        led_10.set_high(); // OFF
        led_11.set_low(); // active-low: LOW = ON
        Timer::after_millis(1000).await;
        led_11.set_high(); // OFF
        led_12.set_low(); // active-low: LOW = ON
        Timer::after_millis(1000).await;
    }
}

/// Hard fault handler, exits with an error status.
#[cortex_m_rt::exception]
unsafe fn HardFault(_frame: &cortex_m_rt::ExceptionFrame) -> ! {
    semihosting::process::exit(1);
}
