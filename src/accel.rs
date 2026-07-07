//! Register maps, flags, and configuration for the LIS2DH12 accelerometer.

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
/// `OUT_X_L`: first of the six output bytes.
pub const OUT_X_L: u8 = 0x28;

/// Auto-increment flag for multi-byte reads.
pub const AUTO_INCREMENT: u8 = 0b1000_0000;
/// `CTRL_REG1` bit 3: low-power (8-bit) mode enable.
pub const CTRL_REG1_LPEN: u8 = 0b0000_1000;
/// `CTRL_REG1` bits [2:0] (enable x, y, and z output).
pub const CTRL_REG1_XYZ_EN: u8 = 0b0000_0111;
/// `CTRL_REG3` bit 4: route the data-ready signal to INT1 pin.
pub const CTRL_REG3_I1_ZYXDA: u8 = 0b0001_0000;
/// `CTRL_REG4` bit 7: block data update (do not split high and low bytes).
pub const CTRL_REG4_BDU: u8 = 0b1000_0000;
/// `CTRL_REG4` bit 3: high-resolution mode enable (12-bit).
pub const CTRL_REG4_HR: u8 = 0b0000_1000;
/// `STATUS_REG` bit 3: a new sample is ready.
pub const STATUS_ZYXDA: u8 = 0b0000_1000;

/// Full-scale acceleration range.
#[derive(Clone, Copy, Default)]
pub enum FullScale {
    /// +/- 2 g.
    #[default]
    G2,
    /// +/- 4 g.
    G4,
    /// +/- 8 g.
    G8,
    /// +/- 16 g.
    G16,
}

impl FullScale {
    /// The full-scale field, `CTRL_REG4` bits 4 and 5.
    #[must_use]
    pub const fn bits(self) -> u8 {
        match self {
            Self::G2 => 0b0000_0000,
            Self::G4 => 0b0001_0000,
            Self::G8 => 0b0010_0000,
            Self::G16 => 0b0011_0000,
        }
    }

    /// Convert a raw axis word to milli-g at the given range.
    #[must_use]
    pub fn milli_g(self, raw: i16) -> i32 {
        let raw = i32::from(raw);
        match self {
            Self::G2 => raw / 16,
            Self::G4 => raw / 8,
            Self::G8 => raw / 4,
            Self::G16 => raw * 3 / 4,
        }
    }
}

/// Output data rate (`CTRL_REG1` bits 4 through 7).
#[derive(Clone, Copy, Default)]
pub enum OutputDataRate {
    /// Power-down.
    PowerDown,
    /// 1 Hz.
    Hz1,
    /// 10 Hz.
    #[default]
    Hz10,
    /// 25 Hz.
    Hz25,
    /// 50 Hz.
    Hz50,
    /// 100 Hz.
    Hz100,
    /// 200 Hz.
    Hz200,
    /// 400 Hz.
    Hz400,
    /// 1344 Hz.
    Hz1344,
}

impl OutputDataRate {
    /// The data-rate field, `CTRL_REG1` bits 4 through 7.
    #[must_use]
    pub const fn bits(self) -> u8 {
        match self {
            Self::PowerDown => 0b0000_0000,
            Self::Hz1 => 0b0001_0000,
            Self::Hz10 => 0b0010_0000,
            Self::Hz25 => 0b0011_0000,
            Self::Hz50 => 0b0100_0000,
            Self::Hz100 => 0b0101_0000,
            Self::Hz200 => 0b0110_0000,
            Self::Hz400 => 0b0111_0000,
            Self::Hz1344 => 0b1001_0000,
        }
    }
}

/// Output resolution, spanning `CTRL_REG1` `LPen` and `CTRL_REG4` HR.
#[derive(Clone, Copy, Default)]
pub enum Resolution {
    /// Low-power mode, 8-bit output (`LPen` = 1, HR = 0).
    Bits8,
    /// Normal mode, 10-bit output (`LPen` = 0, HR = 0).
    #[default]
    Bits10,
    /// High-resolution mode, 12-bit output (`LPen` = 0, HR = 1).
    Bits12,
}

impl Resolution {
    /// The `CTRL_REG4` HR bit for the resolution.
    #[must_use]
    pub const fn hr_bit(self) -> u8 {
        match self {
            Self::Bits12 => CTRL_REG4_HR,
            Self::Bits8 | Self::Bits10 => 0,
        }
    }

    /// The `CTRL_REG1` `LPen` bit for the resolution.
    #[must_use]
    pub const fn lpen_bit(self) -> u8 {
        match self {
            Self::Bits8 => CTRL_REG1_LPEN,
            Self::Bits10 | Self::Bits12 => 0,
        }
    }
}

/// Accelerometer configuration that sets the control-register bytes.
#[derive(Clone, Copy, Default)]
pub struct Config {
    /// Output data rate.
    pub data_rate: OutputDataRate,
    /// Full-scale range.
    pub full_scale: FullScale,
    /// Output resolution.
    pub resolution: Resolution,
}

impl Config {
    /// The `CTRL_REG1` byte: `ORD` + `LPen` + all three axes enabled.
    #[must_use]
    pub const fn ctrl_reg1(self) -> u8 {
        self.data_rate.bits() | self.resolution.lpen_bit() | CTRL_REG1_XYZ_EN
    }

    /// The `CTRL_REG4` byte: block data + full-scale + HR bit.
    #[must_use]
    pub const fn ctrl_reg4(self) -> u8 {
        CTRL_REG4_BDU | self.full_scale.bits() | self.resolution.hr_bit()
    }

    /// Convert a raw axis word to milli-g.
    #[must_use]
    pub fn milli_g(self, raw: i16) -> i32 {
        self.full_scale.milli_g(raw)
    }
}
