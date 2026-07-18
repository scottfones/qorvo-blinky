//! Ultra wideband ranging and transport modules.

pub mod ranging;
pub mod transport;

use dw3000_ng::configs::{BitRate, PulseRepetitionFrequency, UwbChannel};
use dw3000_ng::{DW3000, Ready, Uninitialized};
use embassy_time::Delay;
use embedded_hal::delay::DelayNs;

use crate::board::spim3_uwb::Spim3Uwb;

const DEFAULT_ANTENNA_CHANNEL: UwbChannel = UwbChannel::Channel9;
const DEFAULT_ANTENNA_DELAY_DTU: u16 = 16385;

const OTP_ADDR_ANTENNA_DELAY_CH5: u16 = 0x01A;
const OTP_ADDR_ANTENNA_DELAY_CH9: u16 = 0x01C;

/// Radio driver error type.
pub type DwError = dw3000_ng::Error<Spim3Uwb>;

/// Radio driver result type.
pub type DwResult<T> = Result<T, DwError>;

/// Initialize the DW3110 device.
///
/// # Errors
/// Returns `Err` if:
/// - unable to read device id
/// - soft reset fails
/// - post-config check fails
pub fn bring_up(spim3_uwb: Spim3Uwb) -> DwResult<(DW3000<Spim3Uwb, Ready>, u16)> {
    let mut dw = DW3000::new(spim3_uwb);

    bring_up_id(&mut dw)?;
    soft_reset(&mut dw)?;

    let mut dw = dw
        .init()
        .inspect_err(|e| defmt::error!("bring-up: init failed: {}", e))?;

    let (rx_antenna_delay, tx_antenna_delay) = bring_up_antenna_delay(&mut dw);
    let mut dw = dw
        .config(radio_config(), Delay)
        .inspect_err(|e| defmt::error!("bring-up: config failed: {}", e))?;

    bring_up_is_clean(&mut dw, rx_antenna_delay, tx_antenna_delay)?;
    Ok((dw, tx_antenna_delay))
}

/// Set antenna delay via OTP read or default value.
///
/// # Errors
/// Returns `Err` if OTP read fails
#[must_use]
pub fn bring_up_antenna_delay(dw: &mut DW3000<Spim3Uwb, Uninitialized>) -> (u16, u16) {
    let ch_addr = match DEFAULT_ANTENNA_CHANNEL {
        UwbChannel::Channel5 => OTP_ADDR_ANTENNA_DELAY_CH5,
        UwbChannel::Channel9 => OTP_ADDR_ANTENNA_DELAY_CH9,
    };
    match dw.read_otp(ch_addr) {
        Ok(word) if word != 0 && word != 0xFFFF_FFFF => {
            let rx = u16::try_from(word >> 16).unwrap_or(u16::MAX);
            let tx = u16::try_from(word & 0xFFFF).unwrap_or(u16::MAX);
            defmt::info!(
                "uwb: CH9 antenna delay OTP 0x1C = {=u32:#010x} (rx={=u16} tx={=u16})",
                word,
                rx,
                tx
            );
            (rx, tx)
        }
        Ok(word) => {
            defmt::warn!(
                "uwb: OTP 0x1C blank ({=u32:#010x}); using default {=u16}",
                word,
                DEFAULT_ANTENNA_DELAY_DTU
            );
            (DEFAULT_ANTENNA_DELAY_DTU, DEFAULT_ANTENNA_DELAY_DTU)
        }
        Err(e) => {
            defmt::error!("bringnable to read OTP antenna delay: {}", e);
            defmt::warn!(
                "uwb: OTP 0x1C read failed; using default {=u16}",
                DEFAULT_ANTENNA_DELAY_DTU
            );
            (DEFAULT_ANTENNA_DELAY_DTU, DEFAULT_ANTENNA_DELAY_DTU)
        }
    }
}

/// Query device for id and status info.
///
/// # Errors
/// Returns `Err` if device reads fail
pub fn bring_up_id(dw: &mut DW3000<Spim3Uwb, Uninitialized>) -> DwResult<()> {
    let dev_id = dw
        .ll()
        .dev_id()
        .read()
        .inspect_err(|e| defmt::error!("bring-up: dev_id read failed: {}", e))?;
    let status = dw
        .ll()
        .sys_status()
        .read()
        .inspect_err(|e| defmt::error!("bring-up: sys_status read failed: {}", e))?;

    defmt::info!(
        "uwb: pre-init ridtag={=u16:#06x} model={=u8:#04x} rcinit={=u8} spirdy={=u8}",
        dev_id.ridtag(),
        dev_id.model(),
        status.rcinit(),
        status.spirdy()
    );
    Ok(())
}

/// Validate device has initialized cleanly.
///
/// # Errors
/// Returns `Err` if:
/// - setting `spirdy`, `tx`, or `rx` interrupts fails
pub fn bring_up_is_clean(
    dw: &mut DW3000<Spim3Uwb, Ready>,
    rx_antenna_delay: u16,
    tx_antenna_delay: u16,
) -> DwResult<()> {
    dw.disable_spirdy_interrupt()
        .inspect_err(|e| defmt::error!("bring-up: disable spirdy irq failed: {}", e))?;
    dw.enable_rx_interrupts()
        .inspect_err(|e| defmt::error!("bring-up: enable rx irq failed: {}", e))?;
    dw.enable_tx_interrupts()
        .inspect_err(|e| defmt::error!("bring-up: enable tx irq failed: {}", e))?;

    dw.ll()
        .sys_status()
        .write(|w| {
            w.spirdy(1)
                .txfrb(1)
                .txprs(1)
                .txphs(1)
                .txfrs(1)
                .rxprd(1)
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
        })
        .inspect_err(|e| defmt::error!("bring-up: spirdy W1C failed: {}", e))?;

    dw.set_antenna_delay(rx_antenna_delay, tx_antenna_delay)
        .inspect_err(|e| defmt::error!("bring-up: set antenna delay failed: {}", e))?;

    Ok(())
}

/// Set DW3110 config.
#[must_use]
pub fn radio_config() -> dw3000_ng::Config {
    dw3000_ng::Config {
        channel: DEFAULT_ANTENNA_CHANNEL,
        pulse_repetition_frequency: PulseRepetitionFrequency::Mhz64,
        bitrate: BitRate::Kbps6800,
        ..dw3000_ng::Config::default()
    }
}

/// Soft-reset via UM 8.2.15.1.
fn soft_reset(dw: &mut DW3000<Spim3Uwb, Uninitialized>) -> DwResult<()> {
    dw.ll()
        .clk_ctrl()
        .modify(|_, w| w.sys_clk(0b01))
        .inspect_err(|e| defmt::error!("bring-up: sys_clk write failed: {}", e))?;
    dw.ll()
        .soft_rst()
        .write(|w| {
            w.arm_rst(0)
                .prgn_rst(0)
                .cia_rst(0)
                .bist_rst(0)
                .rx_rst(0)
                .tx_rst(0)
                .hif_rst(0)
                .pmsc_rst(0)
                .gpio_rst(0)
        })
        .inspect_err(|e| defmt::error!("bring-up: arm_rst(0) failed: {}", e))?;
    dw.ll()
        .soft_rst()
        .write(|w| {
            w.arm_rst(1)
                .prgn_rst(1)
                .cia_rst(1)
                .bist_rst(1)
                .rx_rst(1)
                .tx_rst(1)
                .hif_rst(1)
                .pmsc_rst(1)
                .gpio_rst(1)
        })
        .inspect_err(|e| defmt::error!("bring-up: arm_rst(1) failed: {}", e))?;

    // reset settle
    DelayNs::delay_ms(&mut Delay, 2);
    Ok(())
}
