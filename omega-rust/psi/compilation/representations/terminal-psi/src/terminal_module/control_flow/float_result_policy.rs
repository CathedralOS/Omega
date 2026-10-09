//! Post-call policy consumes a raw result exactly once. Psi computes overflow
//! eligibility before that call from its original numeric inputs, in their own
//! carriers, including the selected division's nonzero-divisor exception.
//! Saturation preserves every raw payload except an eligible infinity clamp.

use semantic_vocabulary::{IeeeFloatFormat, ScalarType, ValueId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatResultPolicy {
    Trapping {
        raw_result: ValueId,
    },
    Saturating {
        raw_result: ValueId,
        eligible: ValueId,
    },
}

impl FloatResultPolicy {
    pub const fn raw_result(self) -> ValueId {
        match self {
            Self::Trapping { raw_result } | Self::Saturating { raw_result, .. } => raw_result,
        }
    }
    pub const fn can_trap(self) -> bool {
        matches!(self, Self::Trapping { .. })
    }
    pub fn operands(self) -> Vec<ValueId> {
        match self {
            Self::Trapping { raw_result } => vec![raw_result],
            Self::Saturating {
                raw_result,
                eligible,
            } => vec![eligible, raw_result],
        }
    }
    pub fn accepts_operand_types(
        self,
        format: IeeeFloatFormat,
        mut type_of: impl FnMut(ValueId) -> Option<ScalarType>,
    ) -> bool {
        type_of(self.raw_result()) == Some(ScalarType::IeeeFloat(format))
            && match self {
                Self::Trapping { .. } => true,
                Self::Saturating { eligible, .. } => type_of(eligible) == Some(ScalarType::Boolean),
            }
    }
    pub fn map_operands(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
        match self {
            Self::Trapping { raw_result } => *raw_result = map(*raw_result),
            Self::Saturating {
                raw_result,
                eligible,
            } => {
                *eligible = map(*eligible);
                *raw_result = map(*raw_result);
            }
        }
    }
}
