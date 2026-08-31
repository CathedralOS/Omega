//! Source-handle-free checked plans for proof-only float meaning projection.
//!
//! Canonical rows retain unique plan-local source identities, landed formats,
//! and closed projection-catalog operations. Authored occurrences and spans
//! live in a separate provenance table. Neither table contains runtime values
//! or authorizes float evaluation or native lowering.

use psi_numerics::float_projection::FloatProjectionOperation;
use psi_typed_trees::types::PrimitiveType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedProofValueId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedProofPropositionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedFloatProjectionInputId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedFloatMeaningProjectionOccurrenceId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedProofOnlyValueType {
    FloatMeaning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedProofValueDeclaration {
    pub id: CheckedProofValueId,
    pub value_type: CheckedProofOnlyValueType,
}

/// One checked float-projection input coordinate. Equal validated source keys
/// share an ID assigned densely by first use. The retained row contains no
/// source symbol, expression handle, or runtime bits; binding it to a landed
/// artifact value remains a later producer step.
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

/// Diagnostic provenance for one authored projection call. Multiple
/// occurrences may name the same canonical proof value; the span is never
/// part of that value's semantic identity and is erased before Terminal Psi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedFloatMeaningProjectionOccurrence {
    pub id: CheckedFloatMeaningProjectionOccurrenceId,
    pub value: CheckedProofValueId,
    pub source_span: psi_source::SourceSpan,
}

/// One proof-only equality whose operands are exact results in the retained
/// float-projection table. It is not a runtime Boolean or a machine operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedFloatMeaningEqualityProposition {
    pub id: CheckedProofPropositionId,
    pub left: CheckedProofValueId,
    pub right: CheckedProofValueId,
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
