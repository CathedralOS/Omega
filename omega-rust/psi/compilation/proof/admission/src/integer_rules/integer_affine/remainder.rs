//! A runtime divisor, rather than a landed constant, bounds an unsigned
//! remainder. The witness retains either the exact expression or one canonical
//! result definition. Positivity is a separate child proof, never an axiom
//! inferred from the expression's presence. Formation and availability of a
//! cited executable definition remain the receiving verifier's obligations.

use super::witness_checking::CheckedIntegerEndpointStep;
use super::{
    CheckedIntegerAffineForm, IntegerAffineBoundConversionError, IntegerAffineWitnessError,
};
use semantic_vocabulary::{
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType,
};
use terminal_psi::IntegerAffineWitness;

#[cfg(test)]
mod tests;

pub(super) fn check_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    witness: &IntegerAffineWitness,
) -> Result<Option<CheckedIntegerAffineForm>, IntegerAffineWitnessError> {
    let (expression, definition) = if operands(&witness.target).is_some() {
        (&witness.target, None)
    } else if let [position] = witness.definition_axioms.as_slice()
        && let Some(Proposition::Equal(result, expression)) = semantic_axioms.get(*position)
        && result == &witness.target
        && matches!(result, ScalarTerm::Value { .. })
    {
        (expression, Some(*position))
    } else {
        return Ok(None);
    };
    let Some((integer_type, dividend, divisor)) = operands(expression) else {
        return Ok(None);
    };
    if divisor != &witness.root {
        // The older endpoint word roots at the dividend and lands a constant
        // divisor. It keeps its separate checked witness and mapping.
        return Ok(None);
    }
    if integer_type.carrier() != IntegerCarrier::Fixed
        || integer_type.sign() != IntegerSign::Unsigned
    {
        return Ok(None);
    }
    if !matches!(witness.root, ScalarTerm::Value { .. }) {
        return Err(IntegerAffineWitnessError::RootNotValue);
    }
    if !matches!(
        dividend,
        ScalarTerm::Value { .. } | ScalarTerm::Integer { .. }
    ) {
        return Err(IntegerAffineWitnessError::TargetMismatch);
    }
    match definition {
        None if !witness.definition_axioms.is_empty() || !witness.literal_axioms.is_empty() => {
            return Err(IntegerAffineWitnessError::LiteralAxiomCountMismatch);
        }
        Some(position) => {
            if witness.literal_axioms.as_slice() != [None] {
                return Err(IntegerAffineWitnessError::LiteralAxiomCountMismatch);
            }
            if witness.target == witness.root || &witness.target == dividend {
                return Err(IntegerAffineWitnessError::DefinitionShapeMismatch(position));
            }
        }
        None => {}
    }
    if witness.root.scalar_type() != ScalarType::Integer(integer_type)
        || witness.target.scalar_type() != ScalarType::Integer(integer_type)
    {
        return Err(IntegerAffineWitnessError::TargetTypeMismatch);
    }
    // Validate every declared operand, including embedded literals beyond the
    // signed host window. No i128 endpoint conversion is needed by this rule.
    context
        .validate(&Proposition::Equal(
            witness.root.clone(),
            expression.clone(),
        ))
        .map_err(IntegerAffineWitnessError::MalformedProposition)?;
    if let Some(position) = definition {
        context
            .validate(&semantic_axioms[position])
            .map_err(IntegerAffineWitnessError::MalformedProposition)?;
    }
    Ok(Some(CheckedIntegerAffineForm {
        root: witness.root.clone(),
        target: witness.target.clone(),
        integer_type,
        coefficient: 1,
        offset: 0,
        endpoint_steps: vec![CheckedIntegerEndpointStep::CorrelatedUnsignedRemainder],
    }))
}

pub(super) fn map_bound(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
) -> Result<Proposition, IntegerAffineBoundConversionError> {
    let (lower, divisor, expected) = match root_bound {
        Proposition::LessThan(lower, divisor) => (lower, divisor, 0),
        Proposition::LessOrEqual(lower, divisor) => (lower, divisor, 1),
        _ => return Err(IntegerAffineBoundConversionError::RootBoundMismatch),
    };
    if divisor != form.root()
        || !matches!(lower, ScalarTerm::Integer { scalar_type, value }
            if *scalar_type == form.integer_type() && *value == IntegerValue::Unsigned(expected))
    {
        return Err(IntegerAffineBoundConversionError::RootBoundMismatch);
    }
    Ok(Proposition::LessThan(
        form.target().clone(),
        form.root().clone(),
    ))
}

fn operands(expression: &ScalarTerm) -> Option<(IntegerType, &ScalarTerm, &ScalarTerm)> {
    match expression {
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => Some((*scalar_type, left, right)),
        _ => None,
    }
}
