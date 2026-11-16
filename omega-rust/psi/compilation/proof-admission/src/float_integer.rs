//! Decide the total integer-conversion relations, then check the one admitted
//! conversion rule against its actual premises. Exact eligibility never calls
//! the partial converter. IEEE equality can transport these semantic goals
//! only at a finite nonzero literal, whose meaning has one representation.
//! In particular, an equality with zero cannot discard a negative zero sign.

use numerics::bignum::BigInt;
use numerics::float_semantics::{
    FloatMeaning, FloatSemanticEvaluationError, FloatSemanticOperand, FloatSemanticOperation,
    FloatSemanticResult, FloatToIntegerError,
};
use semantic_vocabulary::{
    IeeeFloatComparisonKind, IeeeFloatValue, IntegerValue, Proposition, PropositionContext,
    PropositionError, ScalarTerm,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FloatIntegerError {
    Malformed(Box<PropositionError>),
    Semantic(FloatSemanticEvaluationError),
    Transport(crate::PredicateDenotationError),
    Unsupported(&'static str),
    NotEstablished,
}

fn meaning(value: IeeeFloatValue) -> FloatMeaning {
    match value {
        IeeeFloatValue::Binary32(bits) => FloatMeaning::from_f32(f32::from_bits(bits)),
        IeeeFloatValue::Binary64(bits) => FloatMeaning::from_f64(f64::from_bits(bits)),
    }
}

/// A closed leaf decision, shared by the explicit certificate primitive and
/// guard consumers. Unknown inputs remain unknown; only semantic conversion
/// failure makes a graph false. Resource/internal failures retain their error.
pub fn closed_float_integer_relation(
    proposition: &Proposition,
) -> Result<Option<bool>, FloatIntegerError> {
    closed_with_endpoint_budget(proposition, 4096)
}

fn closed_with_endpoint_budget(
    proposition: &Proposition,
    maximum_endpoint_bits: u64,
) -> Result<Option<bool>, FloatIntegerError> {
    proposition
        .validate()
        .map_err(|error| FloatIntegerError::Malformed(Box::new(error)))?;
    match proposition {
        Proposition::IeeeFloatIntegerExact { source, target } => {
            let ScalarTerm::IeeeFloatLiteral(source) = source else {
                return Ok(None);
            };
            let target = semantic_vocabulary::fixed_integer_type(*target)
                .map_err(FloatIntegerError::Unsupported)?;
            Ok(Some(match meaning(*source) {
                FloatMeaning::FiniteNonZero(value) => value
                    .to_integer_exact()
                    .is_some_and(|value| target.admits(&value)),
                FloatMeaning::Zero { negative } => !negative,
                FloatMeaning::NaN | FloatMeaning::Infinity { .. } => false,
            }))
        }
        Proposition::FloatIntegerConversion(clause) => {
            let ScalarTerm::IeeeFloatLiteral(source) = &clause.source else {
                return Ok(None);
            };
            let ScalarTerm::Integer { value, .. } = &clause.result else {
                return Ok(None);
            };
            let expected = match value {
                IntegerValue::Signed(value) => BigInt::from_i128(*value),
                IntegerValue::Unsigned(value) => BigInt::from_u128(*value),
            };
            let operation = FloatSemanticOperation::for_contract_identity(&clause.operation)
                .ok_or(FloatIntegerError::Unsupported(
                    "integer conversion has no authenticated catalog row",
                ))?;
            match operation
                .kernel_discharge(
                    &[FloatSemanticOperand::Meaning(meaning(*source))],
                    maximum_endpoint_bits,
                )
                .map(|discharge| discharge.result)
            {
                Ok(FloatSemanticResult::Integer(actual)) => Ok(Some(actual == expected)),
                Ok(_) => Err(FloatIntegerError::Unsupported(
                    "integer conversion catalog returned a different result family",
                )),
                Err(FloatSemanticEvaluationError::IntegerConversion(
                    FloatToIntegerError::NonFinite | FloatToIntegerError::OutOfRange,
                )) => Ok(Some(false)),
                Err(error) => Err(FloatIntegerError::Semantic(error)),
            }
        }
        _ => Ok(None),
    }
}

fn finite_literal_equality(equality: &Proposition) -> Option<(&ScalarTerm, IeeeFloatValue)> {
    let Proposition::ScalarIeeeFloatComparison {
        kind: IeeeFloatComparisonKind::Equal,
        format,
        left,
        right,
    } = equality
    else {
        return None;
    };
    let (subject, literal) = match (left, right) {
        (subject, ScalarTerm::IeeeFloatLiteral(value))
        | (ScalarTerm::IeeeFloatLiteral(value), subject)
            if matches!(meaning(*value), FloatMeaning::FiniteNonZero(_)) =>
        {
            (subject, *value)
        }
        _ => return None,
    };
    (literal.format() == *format
        && subject.scalar_type() == semantic_vocabulary::ScalarType::IeeeFloat(*format))
    .then_some((subject, literal))
}

/// Inventory a usable equality once. This does not prove its truth; the
/// conversion rule still checks the actual cited certificate and both goals.
pub fn float_integer_equality_subject(equality: &Proposition) -> Option<&ScalarTerm> {
    finite_literal_equality(equality).map(|(subject, _)| subject)
}

/// Compute the required second premise of the conversion rule. The first
/// premise is an independently checked IEEE equality at this same source.
/// This function performs no proof search and publishes no raw identity fact.
pub fn float_integer_conversion_substitution(
    context: &PropositionContext,
    goal: &Proposition,
    equality: &Proposition,
) -> Result<Proposition, FloatIntegerError> {
    if !matches!(
        goal,
        Proposition::IeeeFloatIntegerExact { .. } | Proposition::ScalarIeeeFloatComparison { .. }
    ) {
        return Err(FloatIntegerError::Unsupported(
            "integer conversion transport has a different goal family",
        ));
    }
    context
        .validate(equality)
        .map_err(|error| FloatIntegerError::Malformed(Box::new(error)))?;
    let (subject, literal) =
        finite_literal_equality(equality).ok_or(FloatIntegerError::Unsupported(
            "integer conversion needs an IEEE equality to a finite nonzero literal",
        ))?;
    // A finite nonzero IEEE value has a unique encoding in its permanent
    // format. Reuse exact scalar substitution inside this checked semantic
    // rule, without adding an unproved raw-equality node to its certificate.
    let equation = Proposition::Equal(subject.clone(), ScalarTerm::IeeeFloatLiteral(literal));
    let substituted = crate::check_value_equality_substitution(context, goal, [&equation])
        .map_err(FloatIntegerError::Transport)?;
    if substituted == *goal {
        return Err(FloatIntegerError::NotEstablished);
    }
    Ok(substituted)
}

/// The caller has checked every supplied child and its dependency closure.
/// Exact eligibility entails only its converter conditions; equality transport
/// requires a separate proof of the exact substituted goal.
pub fn check_float_integer_conversion(
    context: &PropositionContext,
    goal: &Proposition,
    premise: &Proposition,
    computed: Option<&Proposition>,
) -> Result<(), FloatIntegerError> {
    if computed.is_none() {
        context
            .validate(premise)
            .map_err(|error| FloatIntegerError::Malformed(Box::new(error)))?;
        context
            .validate(goal)
            .map_err(|error| FloatIntegerError::Malformed(Box::new(error)))?;
        let Proposition::IeeeFloatIntegerExact { source, target } = premise else {
            return Err(FloatIntegerError::NotEstablished);
        };
        let carrier = semantic_vocabulary::fixed_integer_type(*target)
            .map_err(FloatIntegerError::Unsupported)?;
        let requirements =
            semantic_vocabulary::float_integer_preconditions(source.clone(), carrier)
                .map_err(FloatIntegerError::Unsupported)?;
        return if requirements.contains(goal) {
            Ok(())
        } else {
            Err(FloatIntegerError::NotEstablished)
        };
    }
    let expected = float_integer_conversion_substitution(context, goal, premise)?;
    if Some(&expected) == computed {
        Ok(())
    } else {
        Err(FloatIntegerError::NotEstablished)
    }
}

#[cfg(test)]
#[path = "float_integer/tests.rs"]
mod tests;
