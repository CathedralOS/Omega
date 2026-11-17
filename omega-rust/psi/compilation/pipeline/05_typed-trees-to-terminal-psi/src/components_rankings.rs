//! A declared measure can pass through helper calls without a second source
//! declaration. Follow the actual argument bindings from that measure;
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
    let mut forward = 0;
    let mut reverse = 0;
    loop {
        // A declared subject passed unchanged into a helper remains that
        // subject. Carry those bindings before reconstructing a measure from
        // a helper's outgoing calls, which may use a newly returned value.
        while let Some(&caller) = pending.get(forward) {
            forward += 1;
            for site in calls.calls(index(caller)) {
                if !components.same_component(index(caller), index(site.callee))
                    || !site.terminal
                    || site.required_contract.is_some()
                {
                    continue;
                }
                let source =
                    terminal_check_tables::row(signatures, caller, caller, Construct::Machine)?;
                let target = terminal_check_tables::row(
                    signatures,
                    site.callee,
                    caller,
                    Construct::Machine,
                )?;
                if target.ranking.is_some() || site.arguments.len() != target.parameters.len() {
                    continue;
                }
                let inputs =
                    forwarded_inputs(&source.parameters, &target.parameters, site.arguments);
                let Some(ranks) = source.ranking.as_ref().and_then(|ranks| {
                    ranks
                        .iter()
                        .map(|rank| {
                            substitute(rank, target, |slot| {
                                let position =
                                    inputs.binary_search_by_key(&slot, |(slot, _)| *slot).ok()?;
                                inputs[position].1.clone()
                            })
                        })
                        .collect::<Option<Vec<_>>>()
                }) else {
                    continue;
                };
                signatures
                    .get_mut(site.callee)
                    .expect("the callee signature was checked")
                    .ranking = Some(ranks);
                pending.push(site.callee);
            }
        }
        let Some(&callee) = pending.get(reverse) else {
            break;
        };
        reverse += 1;
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
                    .map(|rank| substitute(rank, source, |slot| site.arguments.get(slot).cloned()))
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

/// Invert only exact formal bindings. If two target formals receive the
/// same source formal, neither is selected as that measure's identity.
fn forwarded_inputs(
    source: &[ValueType],
    target: &[ValueType],
    arguments: &[Expression],
) -> Vec<(usize, Option<Expression>)> {
    let mut bindings = Vec::with_capacity(arguments.len());
    for (position, argument) in arguments.iter().enumerate() {
        let ExpressionKind::Slot(slot) = argument.kind else {
            continue;
        };
        let Some(parameter) = source.get(slot) else {
            continue;
        };
        if Some(*parameter) != argument.result || target.get(position) != Some(parameter) {
            continue;
        }
        let mut formal = argument.clone();
        formal.kind = ExpressionKind::Slot(position);
        bindings.push((slot, formal));
    }
    bindings.sort_unstable_by_key(|(slot, _)| *slot);
    let mut inputs: Vec<(usize, Option<Expression>)> = Vec::with_capacity(bindings.len());
    for (slot, formal) in bindings {
        if let Some((previous, input)) = inputs.last_mut()
            && *previous == slot
        {
            *input = None;
        } else {
            inputs.push((slot, Some(formal)));
        }
    }
    inputs
}

fn substitute(
    rank: &Expression,
    caller: &Signature,
    actuals: impl Fn(usize) -> Option<Expression> + Copy,
) -> Option<Expression> {
    let mut result = rank.clone();
    result.kind = match &rank.kind {
        ExpressionKind::Slot(slot) => {
            let actual = actuals(*slot)?;
            if actual.result != rank.result || !integer_input(&actual, caller) {
                return None;
            }
            return Some(actual);
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
            let actual = actuals(slot)?;
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
                base: Box::new(actual),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwarded_rank_inputs_require_one_exact_formal() {
        let ty = ValueType::plain(ScalarType::Boolean);
        let slot = |slot| Expression {
            call_context: None,
            kind: ExpressionKind::Slot(slot),
            result: Some(ty),
            source: crate::SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent),
        };
        let ambiguous = forwarded_inputs(&[ty], &[ty, ty], &[slot(0), slot(0)]);
        assert!(ambiguous[0].1.is_none());
        let permuted = forwarded_inputs(&[ty, ty], &[ty, ty], &[slot(1), slot(0)]);
        assert!(matches!(
            permuted[0].1.as_ref().unwrap().kind,
            ExpressionKind::Slot(1)
        ));
        assert!(matches!(
            permuted[1].1.as_ref().unwrap().kind,
            ExpressionKind::Slot(0)
        ));
        let different = ValueType::plain(ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 32)
                .unwrap(),
        ));
        assert!(forwarded_inputs(&[ty], &[different], &[slot(0)]).is_empty());
    }
}
