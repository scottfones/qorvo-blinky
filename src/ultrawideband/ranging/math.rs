//! Ultra wideband ranging math.

/// Reply delay in DTU for  delayed sends (2ms).
#[expect(
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "DTU_RATE is within range and representable"
)]
pub const REPLY_DELAY_DTU: u64 = (0.002 * DTU_RATE as f64) as u64;

/// Modulus of the 40-bit device time counter.
const DTU_MODULUS: u64 = 1 << 40;

/// DTU rate in ticks per second.
const DTU_RATE: i128 = 63_897_600_000;

/// Speed of light in millimeters per second.
const SPEED_OF_LIGHT_MM_PER_SECOND: i128 = 299_792_458_000;

/// Wrapped transmission delay time.
#[must_use]
pub const fn delayed_tx_time(rx_time: u64, reply_delay: u64) -> u64 {
    (rx_time.wrapping_add(reply_delay) % DTU_MODULUS) & !0x1FF
}

/// Distance in millimeters for a one-way time of flight in DTU.
#[must_use]
pub fn distance_mm(time_of_flight_dtu: i64) -> i64 {
    let millimeters = i128::from(time_of_flight_dtu) * SPEED_OF_LIGHT_MM_PER_SECOND / DTU_RATE;
    i64::try_from(millimeters).unwrap_or(i64::MAX)
}

/// Wrap-safe elapsed ticks.
#[must_use]
pub const fn dtu_elapsed(later: u64, earlier: u64) -> u64 {
    later.wrapping_sub(earlier) % DTU_MODULUS
}

#[must_use]
pub const fn predicted_tx_time(programmed_time: u64, tx_antenna_delay: u64) -> u64 {
    programmed_time.wrapping_add(tx_antenna_delay) % DTU_MODULUS
}

/// One-way time of flight in DTU from the four DS-TWR durations.
///
/// The `i128` intermediates hold the products of 40-bit durations, which
/// exceed `u64`, and let the subtraction go negative without wrapping.
#[must_use]
pub fn time_of_flight_dtu(
    time_round1: u64,
    time_reply1: u64,
    time_round2: u64,
    time_reply2: u64,
) -> i64 {
    let numerator = i128::from(time_round1) * i128::from(time_round2)
        - i128::from(time_reply1) * i128::from(time_reply2);
    let denominator = i128::from(time_round1)
        + i128::from(time_reply1)
        + i128::from(time_round2)
        + i128::from(time_reply2);

    if denominator == 0 {
        return 0;
    }

    i64::try_from(numerator / denominator).unwrap_or(i64::MAX)
}
