//! Source-handle-free checked plans for proof-only float meaning projection.
//!
//! These rows retain only dense plan-local identities, the landed source
//! format, and the closed projection-catalog operation. They are not runtime
//! values and do not authorize float evaluation or native lowering.

use psi_numerics::float_projection::FloatProjectionOperation;
use psi_typed_trees::types::PrimitiveType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedProofValueId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedFloatProjectionInputId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedProofOnlyValueType {
    FloatMeaning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedProofValueDeclaration {
    pub id: CheckedProofValueId,
    pub value_type: CheckedProofOnlyValueType,
}

/// One checked float-projection input coordinate. Its identity is local to the
/// retained proof plan and contains no source symbol, expression handle, or
/// runtime bits. Binding it to a landed runtime value is a later producer step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedFloatProjectionInput {
    pub id: CheckedFloatProjectionInputId,
    pub primitive: PrimitiveType,
}

/// Exact proof-only projection selected from the shared closed catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedFloatMeaningProjection {
    pub result: CheckedProofValueDeclaration,
    pub source: CheckedFloatProjectionInput,
    pub operation: FloatProjectionOperation,
}

impl CheckedFloatMeaningProjection {
    /// Independently replay the checked source/result shapes before this row
    /// crosses into Terminal Psi.
    pub fn validate(self) -> Result<(), CheckedFloatMeaningProjectionError> {
        if self.result.value_type != CheckedProofOnlyValueType::FloatMeaning {
            return Err(CheckedFloatMeaningProjectionError::ResultTypeMismatch);
        }
        let expected = match self.operation {
            FloatProjectionOperation::Meaning32 => PrimitiveType::F32,
            FloatProjectionOperation::Meaning64 => PrimitiveType::F64,
        };
        if self.source.primitive != expected {
            return Err(CheckedFloatMeaningProjectionError::SourceFormatMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedFloatMeaningProjectionError {
    ResultTypeMismatch,
    SourceFormatMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projection() -> CheckedFloatMeaningProjection {
        CheckedFloatMeaningProjection {
            result: CheckedProofValueDeclaration {
                id: CheckedProofValueId(3),
                value_type: CheckedProofOnlyValueType::FloatMeaning,
            },
            source: CheckedFloatProjectionInput {
                id: CheckedFloatProjectionInputId(7),
                primitive: PrimitiveType::F32,
            },
            operation: FloatProjectionOperation::Meaning32,
        }
    }

    #[test]
    fn checked_projection_replays_exact_format_without_source_handles() {
        let plan = projection();
        assert_eq!(plan.validate(), Ok(()));
        assert_eq!(plan.result.id, CheckedProofValueId(3));
        assert_eq!(plan.source.id, CheckedFloatProjectionInputId(7));
    }

    #[test]
    fn checked_projection_rejects_cross_format_substitution() {
        let mut plan = projection();
        plan.operation = FloatProjectionOperation::Meaning64;
        assert_eq!(
            plan.validate(),
            Err(CheckedFloatMeaningProjectionError::SourceFormatMismatch)
        );
    }
}
