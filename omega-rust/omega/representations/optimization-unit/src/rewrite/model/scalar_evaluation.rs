use super::super::canonical_encoding::{
    encode_definition_site, encode_integer_value, encode_scalar_type,
};
use super::super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarEvaluationWitness {
    Unary {
        operand_fact: ScalarConstantFactIdentity,
    },
    Binary {
        left_fact: ScalarConstantFactIdentity,
        right_fact: ScalarConstantFactIdentity,
    },
    ProofCertifiedUnary {
        operand_fact: ScalarConstantFactIdentity,
        obligation_fact: AcceptedObligationFactIdentity,
    },
    ProofCertifiedBinary {
        left_fact: ScalarConstantFactIdentity,
        right_fact: ScalarConstantFactIdentity,
        obligation_fact: AcceptedObligationFactIdentity,
    },
    RangeAgainstConstant {
        range_fact: ValueRangeFactIdentity,
        constant_fact: ScalarConstantFactIdentity,
    },
    RangeAgainstRange {
        left_range_fact: ValueRangeFactIdentity,
        right_range_fact: ValueRangeFactIdentity,
    },
}

impl ScalarEvaluationWitness {
    pub const fn unary_operand(self) -> Option<ScalarConstantFactIdentity> {
        match self {
            Self::Unary { operand_fact } | Self::ProofCertifiedUnary { operand_fact, .. } => {
                Some(operand_fact)
            }
            Self::Binary { .. }
            | Self::ProofCertifiedBinary { .. }
            | Self::RangeAgainstConstant { .. }
            | Self::RangeAgainstRange { .. } => None,
        }
    }

    pub const fn binary_operands(
        self,
    ) -> Option<(ScalarConstantFactIdentity, ScalarConstantFactIdentity)> {
        match self {
            Self::Binary {
                left_fact,
                right_fact,
            }
            | Self::ProofCertifiedBinary {
                left_fact,
                right_fact,
                ..
            } => Some((left_fact, right_fact)),
            Self::Unary { .. }
            | Self::ProofCertifiedUnary { .. }
            | Self::RangeAgainstConstant { .. }
            | Self::RangeAgainstRange { .. } => None,
        }
    }

    pub const fn obligation_fact(self) -> Option<AcceptedObligationFactIdentity> {
        match self {
            Self::ProofCertifiedUnary {
                obligation_fact, ..
            }
            | Self::ProofCertifiedBinary {
                obligation_fact, ..
            } => Some(obligation_fact),
            Self::Unary { .. }
            | Self::Binary { .. }
            | Self::RangeAgainstConstant { .. }
            | Self::RangeAgainstRange { .. } => None,
        }
    }

    pub const fn range_against_constant(
        self,
    ) -> Option<(ValueRangeFactIdentity, ScalarConstantFactIdentity)> {
        match self {
            Self::RangeAgainstConstant {
                range_fact,
                constant_fact,
            } => Some((range_fact, constant_fact)),
            Self::Unary { .. }
            | Self::Binary { .. }
            | Self::ProofCertifiedUnary { .. }
            | Self::ProofCertifiedBinary { .. }
            | Self::RangeAgainstRange { .. } => None,
        }
    }

    pub const fn range_against_range(
        self,
    ) -> Option<(ValueRangeFactIdentity, ValueRangeFactIdentity)> {
        match self {
            Self::RangeAgainstRange {
                left_range_fact,
                right_range_fact,
            } => Some((left_range_fact, right_range_fact)),
            Self::Unary { .. }
            | Self::Binary { .. }
            | Self::ProofCertifiedUnary { .. }
            | Self::ProofCertifiedBinary { .. }
            | Self::RangeAgainstConstant { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarConstantValue {
    Boolean(bool),
    Integer(IntegerValue),
}

/// Bind one literal scalar fact to the exact immutable input and definition it
/// describes. The optimizer and independent validator may share this encoding,
/// but each must reconstruct its inputs independently.
pub fn literal_scalar_constant_fact_identity(
    input: OptimizationUnitIdentity,
    machine: MachineId,
    definition: ValueDefinition,
    constant: ScalarConstantValue,
    support: OperationId,
) -> Option<ScalarConstantFactIdentity> {
    match (definition.scalar_type, constant) {
        (ScalarType::Boolean, ScalarConstantValue::Boolean(_))
        | (ScalarType::Integer(_), ScalarConstantValue::Integer(_)) => {}
        _ => return None,
    }
    let mut canonical = Vec::new();
    canonical.extend_from_slice(b"omega.psi-literal-scalar-constant-fact.v1\0");
    canonical.extend_from_slice(&input.bytes());
    canonical.extend_from_slice(&machine.get().to_le_bytes());
    canonical.extend_from_slice(&definition.value.get().to_le_bytes());
    encode_scalar_type(&mut canonical, definition.scalar_type);
    encode_definition_site(&mut canonical, definition.site);
    match constant {
        ScalarConstantValue::Boolean(value) => {
            canonical.push(1);
            canonical.push(u8::from(value));
        }
        ScalarConstantValue::Integer(value) => {
            canonical.push(2);
            encode_integer_value(&mut canonical, value);
        }
    }
    canonical.extend_from_slice(&support.get().to_le_bytes());
    Some(ScalarConstantFactIdentity::from_canonical_bytes(&canonical))
}

/// Compatibility name retained while integer-only rules migrate to the shared
/// scalar candidate vocabulary.
pub type IntegerEvaluationWitness = ScalarEvaluationWitness;
