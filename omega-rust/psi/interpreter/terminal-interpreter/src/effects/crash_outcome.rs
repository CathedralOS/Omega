//! Admission of one externally reported boundary crash against evaluated arguments.

use proof_admission::{ClosedIntegerEvaluationError, ClosedIntegerEvaluator};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType};
use terminal_psi::{BoundaryMachineDeclaration, CrashCause, CrashRouteGuard};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryCrashOutcomeError {
    InvalidDeclaration,
    InvalidArguments,
    UndeclaredCause,
    GuardFalse,
    UnsupportedGuard,
    GuardEvaluationLimitExceeded,
}

/// Check that a published same-cause route permits an externally reported crash
/// for these exact, already evaluated scalar arguments.
pub fn validate_boundary_crash_outcome(
    boundary: &BoundaryMachineDeclaration,
    arguments: &[ScalarTerm],
    cause: CrashCause,
) -> Result<(), BoundaryCrashOutcomeError> {
    proof_admission::check_predicate_evaluation_size(
        boundary
            .crash_routes
            .iter()
            .flat_map(|bucket| &bucket.alternatives)
            .map(|guard| match guard {
                CrashRouteGuard::Truth => &Proposition::Truth,
                CrashRouteGuard::Predicate(predicate) => predicate.proposition(),
            }),
    )
    .map_err(|_| BoundaryCrashOutcomeError::GuardEvaluationLimitExceeded)?;
    validate_boundary_crash_routes(boundary)?;
    if arguments.len() != boundary.scalar_parameters.len()
        || arguments
            .iter()
            .zip(&boundary.scalar_parameters)
            .any(|(argument, expected)| {
                !matches!(
                    argument,
                    ScalarTerm::Boolean(_)
                        | ScalarTerm::Integer { .. }
                        | ScalarTerm::IeeeFloatLiteral(_)
                ) || argument.scalar_type() != *expected
                    || argument.validate().is_err()
            })
    {
        return Err(BoundaryCrashOutcomeError::InvalidArguments);
    }
    let bucket = boundary
        .crash_routes
        .iter()
        .find(|bucket| bucket.cause == cause)
        .ok_or(BoundaryCrashOutcomeError::UndeclaredCause)?;
    let parameters = boundary
        .scalar_contract_parameters()
        .ok_or(BoundaryCrashOutcomeError::InvalidArguments)?;
    let substitutions = parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (parameter.id, argument.clone()))
        .collect();
    let mut unsupported = false;
    let mut exhausted = false;
    let mut integers = ClosedIntegerEvaluator::default();
    for guard in &bucket.alternatives {
        let value = match guard {
            CrashRouteGuard::Truth => Ok(Some(true)),
            CrashRouteGuard::Predicate(predicate) => closed_guard(
                &terminal_semantics::substitution::substitute_proposition_values(
                    predicate.proposition(),
                    &substitutions,
                ),
                &mut integers,
            ),
        };
        match value {
            Ok(Some(true)) => return Ok(()),
            Ok(Some(false)) => {}
            Ok(None) => unsupported = true,
            Err(ClosedIntegerEvaluationError::ResourceLimitExceeded) => exhausted = true,
        }
    }
    Err(if exhausted {
        BoundaryCrashOutcomeError::GuardEvaluationLimitExceeded
    } else if unsupported {
        BoundaryCrashOutcomeError::UnsupportedGuard
    } else {
        BoundaryCrashOutcomeError::GuardFalse
    })
}

fn validate_boundary_crash_routes(
    boundary: &BoundaryMachineDeclaration,
) -> Result<(), BoundaryCrashOutcomeError> {
    if boundary
        .crash_routes
        .windows(2)
        .any(|pair| pair[0].cause >= pair[1].cause)
        || boundary.crash_routes.iter().any(|bucket| {
            bucket.alternatives.is_empty()
                || bucket
                    .alternatives
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || (bucket.alternatives.contains(&CrashRouteGuard::Truth)
                    && bucket.alternatives != [CrashRouteGuard::Truth])
                || bucket.alternatives.iter().any(|guard| {
                    matches!(guard, CrashRouteGuard::Predicate(predicate)
                        if predicate.proposition().validate().is_err())
                })
        })
    {
        return Err(BoundaryCrashOutcomeError::InvalidDeclaration);
    }
    if boundary.crash_routes.is_empty() {
        return Ok(());
    }
    let context = PropositionContext::from_value_types(
        boundary
            .scalar_contract_parameters()
            .ok_or(BoundaryCrashOutcomeError::InvalidDeclaration)?
            .iter()
            .map(|parameter| (parameter.id, parameter.scalar_type)),
    )
    .map_err(|_| BoundaryCrashOutcomeError::InvalidDeclaration)?;
    for predicate in boundary
        .crash_routes
        .iter()
        .flat_map(|bucket| &bucket.alternatives)
        .filter_map(|guard| match guard {
            CrashRouteGuard::Truth => None,
            CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
        })
    {
        if matches!(predicate, Proposition::Truth | Proposition::Falsehood)
            || !predicate.visit_value_ids(|_| {})
            || context.validate(predicate).is_err()
        {
            return Err(BoundaryCrashOutcomeError::InvalidDeclaration);
        }
    }
    Ok(())
}

fn closed_guard(
    proposition: &Proposition,
    integers: &mut ClosedIntegerEvaluator,
) -> Result<Option<bool>, ClosedIntegerEvaluationError> {
    Ok(match proposition {
        Proposition::Truth => Some(true),
        Proposition::Falsehood => Some(false),
        Proposition::ScalarIeeeFloatComparison {
            kind, left, right, ..
        } => {
            let (ScalarTerm::IeeeFloatLiteral(left), ScalarTerm::IeeeFloatLiteral(right)) =
                (left, right)
            else {
                return Ok(None);
            };
            use semantic_vocabulary::{
                IeeeFloatComparisonKind as Kind, IeeeFloatComparisonOperation as Operation,
            };
            let (operation, negate) = match kind {
                Kind::Equal => (Operation::Equal, false),
                Kind::NotEqual => (Operation::NotEqual, false),
                Kind::LessThan => (Operation::Less, false),
                Kind::LessOrEqual => (Operation::LessOrEqual, false),
                Kind::NotLessThan => (Operation::Less, true),
                Kind::NotLessOrEqual => (Operation::LessOrEqual, true),
            };
            Some(crate::scalar_operations::ieee_float_compare(operation, *left, *right) != negate)
        }
        Proposition::Equal(left, right) if left.scalar_type() == ScalarType::Boolean => left
            .boolean_value()
            .zip(right.boolean_value())
            .map(|(left, right)| left == right),
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            let Some(((left_type, left), (right_type, right))) =
                left.integer_value().zip(right.integer_value())
            else {
                return Ok(None);
            };
            if left_type != right_type {
                return Ok(None);
            }
            left_type
                .compare(left, right)
                .map(|ordering| match proposition {
                    Proposition::Equal(..) => ordering.is_eq(),
                    Proposition::LessThan(..) => ordering.is_lt(),
                    _ => !ordering.is_gt(),
                })
        }
        Proposition::IntegerMathEqual(left, right)
        | Proposition::IntegerMathLessThan(left, right)
        | Proposition::IntegerMathLessOrEqual(left, right) => {
            integers
                .compare(left, right)?
                .map(|ordering| match proposition {
                    Proposition::IntegerMathEqual(..) => ordering.is_eq(),
                    Proposition::IntegerMathLessThan(..) => ordering.is_lt(),
                    _ => !ordering.is_gt(),
                })
        }
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
            let conjunction = matches!(proposition, Proposition::Conjunction(_));
            let mut value = conjunction;
            for child in children {
                let Some(child) = closed_guard(child, integers)? else {
                    return Ok(None);
                };
                value = if conjunction {
                    value && child
                } else {
                    value || child
                };
            }
            Some(value)
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => match closed_guard(premise, integers)? {
            Some(false) => Some(true),
            Some(true) => closed_guard(conclusion, integers)?,
            None => None,
        },
        _ => None,
    })
}
