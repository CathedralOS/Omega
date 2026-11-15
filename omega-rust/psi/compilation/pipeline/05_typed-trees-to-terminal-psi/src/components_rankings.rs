//! A declared measure can pass through helper calls without a second source
//! declaration. Follow the actual argument bindings back from that measure;
//! the completed component still proves every preserving or decreasing edge.

use super::{CallGraph, Components, index};
use crate::body::{Expression, ExpressionKind, Signature, Strict, ValueType};
use semantic_vocabulary::{IdTable, MachineId, ScalarType};
use terminal_check_tables::{Checked, Construct};

pub(super) fn carry(
    calls: &CallGraph<'_>,
    components: &Components,
    signatures: &mut IdTable<MachineId, Signature>,
) -> Checked<()> {
    let mut pending = Vec::new();
    for (machine, signature) in signatures.iter() {
        if signature.ranking.is_some() && !components.members(machine)?.is_empty() {
            pending.push(machine);
        }
    }
    let mut next = 0;
    while let Some(&callee) = pending.get(next) {
        next += 1;
        for &(caller_index, occurrence) in components.call_sites(index(callee)) {
            if !components.same_component(caller_index, index(callee)) {
                continue;
            }
            let site = &calls.sites[occurrence];
            // Public selections retain their own invocation association. They
            // cannot acquire a body witness through the direct-call route.
            if !site.terminal || site.required_contract.is_some() {
                continue;
            }
            let caller = MachineId::new(caller_index as u64 + 1)
                .expect("an existing machine index has a nonzero identity");
            let source =
                terminal_check_tables::row(signatures, caller, caller, Construct::Machine)?;
            if source.ranking.is_some() {
                continue;
            }
            let target =
                terminal_check_tables::row(signatures, callee, caller, Construct::Machine)?;
            if site.arguments.len() != target.parameters.len() {
                continue;
            }
            let Some(ranks) = target.ranking.as_ref().and_then(|ranks| {
                ranks
                    .iter()
                    .map(|rank| substitute(rank, source, site.arguments))
                    .collect::<Option<Vec<_>>>()
            }) else {
                continue;
            };
            // Only the private measure changes. No helper receives a new
            // public termination promise or an induction hypothesis.
            signatures
                .get_mut(caller)
                .expect("the caller signature was checked")
                .ranking = Some(ranks);
            pending.push(caller);
        }
    }
    Ok(())
}

fn substitute(rank: &Expression, caller: &Signature, actuals: &[Expression]) -> Option<Expression> {
    let mut result = rank.clone();
    result.kind = match &rank.kind {
        ExpressionKind::Slot(slot) => {
            let actual = actuals.get(*slot)?;
            if actual.result != rank.result || !integer_input(actual, caller) {
                return None;
            }
            return Some(actual.clone());
        }
        ExpressionKind::Integer(value) => ExpressionKind::Integer(*value),
        ExpressionKind::Strict {
            operation: Strict::Subtract,
            policy,
            left,
            right,
        } => ExpressionKind::Strict {
            operation: Strict::Subtract,
            policy: *policy,
            left: Box::new(substitute(left, caller, actuals)?),
            right: Box::new(substitute(right, caller, actuals)?),
        },
        ExpressionKind::CollectionMeasure {
            base,
            measure: typed_trees::expression::CollectionMeasure::Length,
        } => {
            let ExpressionKind::Slot(slot) = base.kind else {
                return None;
            };
            let actual = actuals.get(slot)?;
            let ExpressionKind::Slot(input) = actual.kind else {
                return None;
            };
            if actual.result != base.result
                || caller.parameters.get(input).copied() != actual.result
                || !matches!(actual.result, Some(ValueType::Record(..)))
            {
                return None;
            }
            ExpressionKind::CollectionMeasure {
                base: Box::new(actual.clone()),
                measure: typed_trees::expression::CollectionMeasure::Length,
            }
        }
        _ => return None,
    };
    Some(result)
}

/// A copied rank may observe only the caller's formal values, using the same
/// finite expression forms that ranking lowering checks. A local, call, or
/// field read cannot become an invocation-entry observation by substitution.
fn integer_input(value: &Expression, caller: &Signature) -> bool {
    if !matches!(value.result, Some(ValueType::Scalar(ScalarType::Integer(integer), _))
        if !integer.is_address() && integer.bits().is_some())
    {
        return false;
    }
    match &value.kind {
        ExpressionKind::Slot(slot) => {
            caller.parameters.get(*slot).copied() == value.result
                && caller.borrowed(*slot).is_none()
        }
        ExpressionKind::Integer(_) => true,
        ExpressionKind::Strict {
            operation: Strict::Subtract,
            left,
            right,
            ..
        } => integer_input(left, caller) && integer_input(right, caller),
        _ => false,
    }
}
