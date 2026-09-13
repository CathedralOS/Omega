//! Admission of a reported boundary crash, not prediction of an invocation.
//!
//! Arguments are already evaluated snapshots. Substitute the declaration's
//! exact scalar telescope, then decide its closed predicates. Mathematical
//! integers share the proof kernel's denotation; a fixed-width evaluator would
//! incorrectly wrap proof-only intermediate values. Bound input traversal before
//! validation/cloning and share arithmetic work across the whole outcome check.
//! Unsupported or exhausted evaluation cannot become a false route or authorize
//! a crash. This adds neither a normal-return guarantee nor a provider inference.

use super::*;
use proof_admission::{ClosedIntegerEvaluationError, ClosedIntegerEvaluator};
use terminal_psi::CrashCause;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryCrashOutcomeError {
    InvalidDeclaration(ModuleError),
    InvalidArguments,
    UndeclaredCause,
    GuardFalse,
    UnsupportedGuard,
    GuardEvaluationLimitExceeded,
}

impl std::fmt::Display for BoundaryCrashOutcomeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for BoundaryCrashOutcomeError {}

/// Check that a published same-cause route permits an externally reported crash
/// for these exact, already evaluated scalar arguments. Success does not demand
/// a crash and establishes neither normal completion nor disposition receipts.
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
    validate_boundary_crash_routes(boundary)
        .map_err(BoundaryCrashOutcomeError::InvalidDeclaration)?;
    if arguments.len() != boundary.scalar_parameters.len()
        || arguments
            .iter()
            .zip(&boundary.scalar_parameters)
            .any(|(argument, expected)| {
                !matches!(
                    argument,
                    ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. }
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
    // Formal IDs belong to the scalar telescope, not caller IDs or the mixed
    // structural argument positions. Substitution is simultaneous.
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
                &substitute_proposition_values(predicate.proposition(), &substitutions),
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

fn closed_guard(
    proposition: &Proposition,
    integers: &mut ClosedIntegerEvaluator,
) -> Result<Option<bool>, ClosedIntegerEvaluationError> {
    Ok(match proposition {
        Proposition::Truth => Some(true),
        Proposition::Falsehood => Some(false),
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
        // Structural/opaque predicates need their own observation support;
        // inability to evaluate is never false.
        _ => None,
    })
}
