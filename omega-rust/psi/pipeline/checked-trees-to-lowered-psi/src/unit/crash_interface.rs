//! Effective machine crash ceilings shared by contracts and continuations.
//!
//! A `PublishedCeiling` plan's authored buckets are contract identity and are
//! consumed verbatim. An `InternalInferred` plan publishes none, yet its
//! retained body evidence — explicit crash sites, per-invocation surviving
//! buckets, and selected-operator buckets — still describes every crash the
//! lowered machine can commit. The terminal verifier covers each emitted
//! continuation and crash terminator against the machine's own contract and
//! independently reconstructs each invocation envelope by substituting the
//! callee's declared routes, so the machine contract and every call-site
//! continuation must derive from this same ceiling: an inferred body that can
//! crash must publish that union or the module fails coverage.
//!
//! Guards survive only when their checked scalar evidence stays inside the
//! machine's contract namespace. A surviving guard over a body local or a
//! storage read cannot name an entry parameter, so the cause widens to the
//! unconditional route — the same widening checked call custody applies when
//! a caller cannot carry the exact guard.

use std::collections::BTreeMap;

use checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedTrees, CrashInterface,
    CrashRouteBucket, CrashRouteGuard,
};

use crate::lowering_error::LoweringError;

/// The crash ceiling the checked contract plan proves for `machine`, in the
/// machine's own parameter namespace: authored buckets for a published
/// ceiling, or the union of retained body evidence for an inferred body.
/// Lowered machine contracts and invocation continuations share this source
/// so the verifier's independent substitution agrees with production.
///
/// The returned buckets are still checked terms: callers pick the scalar or
/// structural lowering lane for the machine's parameter shape, exactly as they
/// did for `CrashPlan::published()` — this function must not pre-validate one
/// lane, or scalar-only rejection would pre-empt structural member lowering.
pub(crate) fn effective_crash_routes(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<CrashRouteBucket>, LoweringError> {
    let Some(plan) = checked.facts.contract_plans.for_machine(machine) else {
        return Ok(Vec::new());
    };
    if plan.crash.interface() == CrashInterface::PublishedCeiling {
        return Ok(plan.crash.published().to_vec());
    }
    let mut grouped = BTreeMap::<checked_trees::CrashCause, Vec<CrashRouteGuard>>::new();
    // An explicit body crash is unconditional implementation evidence, matching
    // the checker's private-summary widening.
    for site in plan.crash.checked_sites() {
        grouped.insert(site.cause(), vec![CrashRouteGuard::Truth]);
    }
    for call in plan.crash.checked_calls() {
        contribute(&mut grouped, call.surviving_buckets());
    }
    for operator in plan.crash.checked_operators() {
        contribute(&mut grouped, operator.surviving());
    }
    Ok(grouped
        .into_iter()
        .filter_map(|(cause, guards)| CrashRouteBucket::new(cause, guards))
        .collect())
}

/// Merge one evidence bucket set into the per-cause union. Authored `true` and
/// `false` alternatives normalize exactly as `lower_formal_crash_routes` does;
/// a guard whose checked evidence cannot name contract parameters widens its
/// cause to `Truth` rather than lowering a partial contract.
fn contribute(
    grouped: &mut BTreeMap<checked_trees::CrashCause, Vec<CrashRouteGuard>>,
    buckets: &[CrashRouteBucket],
) {
    for bucket in buckets {
        let guards = grouped.entry(bucket.cause()).or_default();
        for guard in bucket.alternative_guards() {
            match guard {
                CrashRouteGuard::Truth => {
                    *guards = vec![CrashRouteGuard::Truth];
                }
                CrashRouteGuard::Predicate(predicate) => match predicate.scalar_expression() {
                    Some(CheckedBooleanExpression::Constant(false)) => {}
                    Some(CheckedBooleanExpression::Constant(true)) => {
                        *guards = vec![CrashRouteGuard::Truth];
                    }
                    Some(expression) if contract_scoped(expression) => {
                        if *guards != [CrashRouteGuard::Truth] {
                            guards.push(guard.clone());
                        }
                    }
                    _ => {
                        *guards = vec![CrashRouteGuard::Truth];
                    }
                },
            }
            if *guards == [CrashRouteGuard::Truth] {
                break;
            }
        }
    }
}

/// Whether a checked Boolean guard can be written over the machine's contract
/// parameters. Scalar parameter positions and structural parameter paths are
/// contract terms; body locals, mutable storage reads, literal floats, and
/// runtime-policy casts have no contract namespace, so their cause widens to
/// the unconditional route instead of lowering a partial contract.
fn contract_scoped(expression: &CheckedBooleanExpression) -> bool {
    match expression {
        CheckedBooleanExpression::Constant(_)
        | CheckedBooleanExpression::StorageRead { .. }
        | CheckedBooleanExpression::Local { .. } => false,
        CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::ErasedParameter { .. }
        | CheckedBooleanExpression::StructuralParameterField { .. }
        | CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => true,
        CheckedBooleanExpression::Not(operand) => contract_scoped(operand),
        CheckedBooleanExpression::Equal { left, right }
        | CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            contract_scoped(left) && contract_scoped(right)
        }
        CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            contract_scoped_scalar(left) && contract_scoped_scalar(right)
        }
    }
}

fn contract_scoped_scalar(expression: &CheckedScalarExpression) -> bool {
    match expression {
        CheckedScalarExpression::StorageRead { .. }
        | CheckedScalarExpression::Local { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::IntegerTrappingCast { .. } => false,
        CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::ErasedParameter { .. }
        | CheckedScalarExpression::StructuralParameterByteLength { .. }
        | CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::IntegerLiteral { .. } => true,
        CheckedScalarExpression::StructuralParameterIndexedRead { index, .. } => {
            contract_scoped_scalar(index)
        }
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            contract_scoped_scalar(left) && contract_scoped_scalar(right)
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. }
        | CheckedScalarExpression::IntegerWrappingCast { operand, .. } => {
            contract_scoped_scalar(operand)
        }
        CheckedScalarExpression::Boolean(expression) => contract_scoped(expression),
    }
}
