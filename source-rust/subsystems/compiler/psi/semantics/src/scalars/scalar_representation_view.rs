use semantic_vocabulary::{
    IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, ScalarType,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarRepresentationViewPair {
    FloatBits {
        format: IeeeFloatFormat,
        integer: IntegerType,
    },
    BitsFloat {
        integer: IntegerType,
        format: IeeeFloatFormat,
    },
    BooleanByte {
        integer: IntegerType,
    },
    ByteBoolean {
        integer: IntegerType,
    },
}

impl ScalarRepresentationViewPair {
    pub fn classify(source: ScalarType, target: ScalarType) -> Option<Self> {
        let fixed = |integer: IntegerType, bits: u16| {
            !integer.is_address() && integer.bits() == Some(bits)
        };
        match (source, target) {
            (ScalarType::IeeeFloat(format), ScalarType::Integer(integer))
                if fixed(integer, format_bits(format)) =>
            {
                Some(Self::FloatBits { format, integer })
            }
            (ScalarType::Integer(integer), ScalarType::IeeeFloat(format))
                if fixed(integer, format_bits(format)) =>
            {
                Some(Self::BitsFloat { integer, format })
            }
            (ScalarType::Boolean, ScalarType::Integer(integer)) if fixed(integer, 8) => {
                Some(Self::BooleanByte { integer })
            }
            (ScalarType::Integer(integer), ScalarType::Boolean) if fixed(integer, 8) => {
                Some(Self::ByteBoolean { integer })
            }
            _ => None,
        }
    }

    pub const fn requires_landing(self) -> bool {
        matches!(self, Self::ByteBoolean { .. })
    }

    pub fn view(self, value: ScalarViewValue) -> Option<ScalarViewValue> {
        match (self, value) {
            (Self::FloatBits { format, integer }, ScalarViewValue::IeeeFloat(float))
                if float.format() == format =>
            {
                let bits = match float {
                    IeeeFloatValue::Binary32(bits) => u64::from(bits),
                    IeeeFloatValue::Binary64(bits) => bits,
                };
                Some(ScalarViewValue::Integer(integer_from_bits(integer, bits)?))
            }
            (Self::BitsFloat { integer, format }, ScalarViewValue::Integer(value)) => {
                let bits = integer_bits(integer, value)?;
                Some(ScalarViewValue::IeeeFloat(match format {
                    IeeeFloatFormat::Binary32 => {
                        IeeeFloatValue::Binary32(u32::try_from(bits).ok()?)
                    }
                    IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(bits),
                }))
            }
            (Self::BooleanByte { integer }, ScalarViewValue::Boolean(value)) => Some(
                ScalarViewValue::Integer(integer_from_bits(integer, u64::from(value))?),
            ),
            (Self::ByteBoolean { integer }, ScalarViewValue::Integer(value)) => {
                match integer_bits(integer, value)? {
                    0 => Some(ScalarViewValue::Boolean(false)),
                    1 => Some(ScalarViewValue::Boolean(true)),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarViewValue {
    Boolean(bool),
    Integer(IntegerValue),
    IeeeFloat(IeeeFloatValue),
}

const fn format_bits(format: IeeeFloatFormat) -> u16 {
    match format {
        IeeeFloatFormat::Binary32 => 32,
        IeeeFloatFormat::Binary64 => 64,
    }
}

pub(crate) fn integer_bits(integer: IntegerType, value: IntegerValue) -> Option<u64> {
    let width = integer.bits()?;
    if !integer.admits(value) || width > 64 {
        return None;
    }
    let mask = if integer.bits() == Some(64) {
        u64::MAX
    } else {
        (1u64 << width) - 1
    };
    Some(match value {
        IntegerValue::Signed(value) => (value as u64) & mask,
        IntegerValue::Unsigned(value) => u64::try_from(value).ok()? & mask,
    })
}

pub(crate) fn integer_from_bits(integer: IntegerType, bits: u64) -> Option<IntegerValue> {
    let width = u32::from(integer.bits()?);
    if width > 64 {
        return None;
    }
    Some(match integer.sign() {
        IntegerSign::Unsigned => {
            let mask = if width == 64 {
                u64::MAX
            } else {
                (1u64 << width) - 1
            };
            IntegerValue::Unsigned(u128::from(bits & mask))
        }
        IntegerSign::Signed => {
            let shift = 64 - width;
            IntegerValue::Signed(i128::from(((bits << shift) as i64) >> shift))
        }
    })
}
