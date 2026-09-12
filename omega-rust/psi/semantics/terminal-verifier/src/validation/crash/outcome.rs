//! Admission of a reported boundary crash, not prediction of an invocation.

use super::*;
use terminal_psi::CrashCause;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryCrashOutcomeError {
    InvalidDeclaration(ModuleError),
    InvalidArguments,
    UndeclaredCause,
    GuardFalse,
    UnsupportedGuard,
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
    for guard in &bucket.alternatives {
        let value = match guard {
            CrashRouteGuard::Truth => Some(true),
            CrashRouteGuard::Predicate(predicate) => closed_guard(&substitute_proposition_values(
                predicate.proposition(),
                &substitutions,
            )),
        };
        match value {
            Some(true) => return Ok(()),
            Some(false) => {}
            None => unsupported = true,
        }
    }
    Err(if unsupported {
        BoundaryCrashOutcomeError::UnsupportedGuard
    } else {
        BoundaryCrashOutcomeError::GuardFalse
    })
}

fn closed_guard(proposition: &Proposition) -> Option<bool> {
    match proposition {
        Proposition::Truth => Some(true),
        Proposition::Falsehood => Some(false),
        Proposition::Equal(left, right) if left.scalar_type() == ScalarType::Boolean => {
            Some(left.boolean_value()? == right.boolean_value()?)
        }
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            let (left_type, left) = left.integer_value()?;
            let (right_type, right) = right.integer_value()?;
            if left_type != right_type {
                return None;
            }
            let ordering = left_type.compare(left, right)?;
            Some(match proposition {
                Proposition::Equal(..) => ordering.is_eq(),
                Proposition::LessThan(..) => ordering.is_lt(),
                _ => !ordering.is_gt(),
            })
        }
        Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
            let conjunction = matches!(proposition, Proposition::Conjunction(_));
            let mut value = conjunction;
            for child in children {
                let child = closed_guard(child)?;
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
        } => Some(!closed_guard(premise)? || closed_guard(conclusion)?),
        // Mathematical integers and structural/opaque predicates need their
        // own closed denotation support; inability to evaluate is never false.
        _ => None,
    }
}
