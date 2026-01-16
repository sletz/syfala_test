use alloc::boxed::Box;
use core::{fmt, num};
use serde::{Deserialize, Serialize};

/// Always in little endian, for now
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
pub enum SampleType {
    U8,
    U16,
    U24,
    U32,
    U64,
    I8,
    I16,
    I24,
    I32,
    I64,
    IEEF32,
    IEEF64,
}

impl SampleType {
    #[inline(always)]
    pub fn is_signed(self) -> bool {
        use SampleType::*;
        [I8, I16, I24, I32, IEEF32, IEEF64].contains(&self)
    }

    #[inline(always)]
    pub fn is_float(self) -> bool {
        use SampleType::*;
        [IEEF32, IEEF64].contains(&self)
    }

    #[inline(always)]
    pub const fn sample_size(self) -> num::NonZeroU8 {
        use SampleType::*;
        let res = match self {
            U8 | I8 => 1,
            U16 | I16 => 2,
            U24 | I24 => 3,
            U32 | I32 | IEEF32 => 4,
            U64 | I64 | IEEF64 => 8,
        };

        num::NonZeroU8::new(res).unwrap()
    }
}

// TODO Add sample formats to configs
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
#[serde(try_from = "f64")]
pub struct SampleRate(f64);

impl SampleRate {
    #[inline(always)]
    pub const fn get(&self) -> &f64 {
        &self.0
    }

    #[inline(always)]
    pub const fn new(val: f64) -> Option<Self> {
        if let num::FpCategory::Normal = val.classify()
            && val.is_sign_positive()
        {
            return Some(Self(val));
        }
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct SampleRateError;

impl fmt::Display for SampleRateError {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Sample rate must be normal and positive")
    }
}

impl TryFrom<f64> for SampleRate {
    type Error = SampleRateError;

    #[inline(always)]
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(SampleRateError)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct ChannelCount(pub num::NonZeroU32);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct BufferSize(pub num::NonZeroU32); // Important: in _frames_

/// Represents an audio stream configuration.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Format {
    pub sample_rate: SampleRate,
    pub channel_count: ChannelCount,
    pub buffer_size: BufferSize,
    pub sample_type: SampleType,
}

impl Default for Format {
    #[inline(always)]
    fn default() -> Self {
        Self::standard()
    }
}

impl Format {
    #[inline(always)]
    pub const fn standard() -> Format {
        Format {
            sample_rate: SampleRate::new(48e3).unwrap(),
            channel_count: ChannelCount(num::NonZeroU32::new(1).unwrap()),
            buffer_size: BufferSize(num::NonZeroU32::new(64).unwrap()),
            sample_type: SampleType::IEEF32,
        }
    }

    /// This is the same as [`self.n_channels`](Self::n_channels)` *
    /// `[`self.buffer_size_frames`](Self::buffer_size_frames)
    #[inline(always)]
    pub fn chunk_size_samples(&self) -> Option<num::NonZeroU32> {
        self.buffer_size.0.checked_mul(self.channel_count.0)
    }

    #[inline(always)]
    pub fn chunk_size_bytes(&self) -> Option<num::NonZeroU32> {
        self.chunk_size_samples()
            .and_then(|n| n.checked_mul(num::NonZeroU32::new(4).unwrap()))
    }
}

#[derive(Debug, Default, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    pub inputs: Box<[Format]>,
    pub outputs: Box<[Format]>,
}
