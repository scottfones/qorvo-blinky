//! Register maps, flags and logic for the LIS2DH12 accelerometer.

/// I2C device address (SA0 tied high).
pub const ADDR: u8 = 0x19;
/// `CTRL_REG1`: output data rate, low-power enable, per-axis enables.
pub const CTRL_REG1: u8 = 0x20;
/// `CTRL_REG3`: interrupt-source routing to the INT1 pin.
pub const CTRL_REG3: u8 = 0x22;
/// `CTRL_REG4`: block data update, full-scale, high-resolution.
pub const CTRL_REG4: u8 = 0x23;
/// `STATUS_REG`: new-data and overrun flags.
pub const STATUS_REG: u8 = 0x27;
/// `OUT_X_L`: first of the six output bytes (0x28..=0x2D).
pub const OUT_X_L: u8 = 0x28;

/// Auto-increment flag for multi-byte reads.
pub const AUTO_INCREMENT: u8 = 0b1000_0000;
/// `CTRL_REG3` bit 4: route the data-ready signal to the INT1 pin.
pub const CTRL_REG3_I1_ZYXDA: u8 = 0b0001_0000;
/// `STATUS_REG` bit 3: a new X, Y, and Z sample is available.
pub const STATUS_ZYXDA: u8 = 0b0000_1000;

/// Convert one raw axis word to milli-g at +/-2 g full-scale.
#[must_use]
pub fn milli_g(raw: i16) -> i32 {
    i32::from(raw) / 16
}
