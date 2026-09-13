//! Transitive copy resolution across inventoried block-parameter bindings.
//!
//! A parameter is a copy when every incoming `Jump`/`Conditional` edge binds
//! the same resolved value at its position. Propositions and evidence rows
//! name value identities without listing direct uses, so every value they can
//! mention is retained before resolution begins.

use semantic_vocabulary::{BlockId, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    CrashRouteBucket, CrashRouteGuard, OperationKind as O, TerminalMachine, Terminator,
    ValueDeclaration,
};

pub(super) fn propagate(
    machine: &mut TerminalMachine,
    source_calls: &[lowered_psi::LoweredSourceCallOccurrence],
    retained_values: &mut BTreeSet<ValueId>,
) {
    // Ranking evidence names exact parameters and edge-argument positions.
    if machine.ranked_scc.is_some() {
        return;
    }
    // Propositions carried by contracts, crash sites, and call continuations
    // keep the exact identities they mention: they are proof terms, not uses.
    for proposition in &machine.contract.requires {
        retain_proposition(proposition, retained_values);
    }
    for clause in &machine.contract.ensures {
        retain_proposition(&clause.proposition, retained_values);
    }
    for guarantee in &machine.contract.outcome_specific_ensures {
        retain_proposition(&guarantee.proposition, retained_values);
    }
    retain_crash_routes(&machine.contract.crash_routes, retained_values);
    let operations = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    for operation in &operations {
        retain_crash_routes(crash_continuations(&operation.kind), retained_values);
    }
    for occurrence in source_calls {
        if operations
            .iter()
            .any(|operation| operation.id == occurrence.terminal_operation)
        {
            retained_values.extend(
                occurrence
                    .source_values_before_call
                    .iter()
                    .map(|value| value.id),
            );
        }
    }
    let mut incoming_arguments: BTreeMap<BlockId, Vec<Vec<ValueId>>> = BTreeMap::new();
    let mut structural_case_targets = BTreeSet::new();
    for block in &machine.blocks {
        match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => {
                incoming_arguments
                    .entry(*target)
                    .or_default()
                    .push(arguments.clone());
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    incoming_arguments
                        .entry(edge.target)
                        .or_default()
                        .push(edge.arguments.clone());
                }
            }
            Terminator::StructuralCase { cases, .. } => {
                structural_case_targets.extend(cases.iter().map(|case| case.target));
            }
            Terminator::Crash { site_guard, .. } => {
                for term in site_guard {
                    retain_proposition(term.proposition(), retained_values);
                }
            }
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. } => {}
        }
    }
    // A parameter resolves only when its block has inventoried incoming scalar
    // edges and is not a structural-case payload target, whose bindings are
    // positional rather than listed arguments.
    let mut raw: BTreeMap<ValueId, Vec<ValueId>> = BTreeMap::new();
    let mut declarations: BTreeMap<ValueId, ValueDeclaration> = BTreeMap::new();
    for parameter in &machine.parameters {
        declarations.insert(parameter.id, *parameter);
    }
    if let Some(result) = machine.result.scalar() {
        declarations.insert(result.id, result);
    }
    for block in &machine.blocks {
        for parameter in &block.parameters {
            declarations.insert(parameter.id, *parameter);
        }
        for operation in &block.operations {
            if let Some(result) = operation.result.scalar() {
                declarations.insert(result.id, result);
            }
        }
        if structural_case_targets.contains(&block.id) {
            continue;
        }
        if let Some(edges) = incoming_arguments.get(&block.id) {
            for (position, parameter) in block.parameters.iter().enumerate() {
                raw.insert(
                    parameter.id,
                    edges.iter().map(|arguments| arguments[position]).collect(),
                );
            }
        }
    }
    let mut memo: BTreeMap<ValueId, ValueId> = BTreeMap::new();
    let mut collapse: BTreeMap<ValueId, ValueId> = BTreeMap::new();
    for parameter in raw.keys() {
        if retained_values.contains(parameter) {
            continue;
        }
        let source = resolve(*parameter, &raw, &mut memo);
        let declaration = &declarations[parameter];
        if source != *parameter
            && declarations.get(&source).is_some_and(|source_declaration| {
                source_declaration.scalar_type == declaration.scalar_type
                    && source_declaration.qualifications == declaration.qualifications
            })
        {
            collapse.insert(*parameter, source);
        }
    }
    if collapse.is_empty() {
        return;
    }
    let mut removed_positions: BTreeMap<BlockId, Vec<usize>> = BTreeMap::new();
    for block in &mut machine.blocks {
        let removed = block
            .parameters
            .iter()
            .enumerate()
            .filter_map(|(position, parameter)| {
                collapse.contains_key(&parameter.id).then_some(position)
            })
            .collect::<Vec<_>>();
        if !removed.is_empty() {
            removed_positions.insert(block.id, removed);
        }
        block
            .parameters
            .retain(|parameter| !collapse.contains_key(&parameter.id));
        for operation in &mut block.operations {
            operation
                .kind
                .map_scalar_uses(&mut |value| collapse.get(&value).copied().unwrap_or(value));
        }
        block
            .terminator
            .map_scalar_uses(&mut |value| collapse.get(&value).copied().unwrap_or(value));
    }
    for block in &mut machine.blocks {
        match &mut block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => drop_positions(arguments, removed_positions.get(target)),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    drop_positions(&mut edge.arguments, removed_positions.get(&edge.target));
                }
            }
            _ => {}
        }
    }
}

/// The identity `value` keeps after every justified copy collapses: the unique
/// resolved source when all incoming edges agree, or `value` itself when it is
/// not a copy parameter or its bindings disagree. A value still mid-resolution
/// on the current cycle stands for itself.
fn resolve(
    value: ValueId,
    raw: &BTreeMap<ValueId, Vec<ValueId>>,
    memo: &mut BTreeMap<ValueId, ValueId>,
) -> ValueId {
    resolve_in(value, raw, memo, &mut BTreeSet::new())
}

fn resolve_in(
    value: ValueId,
    raw: &BTreeMap<ValueId, Vec<ValueId>>,
    memo: &mut BTreeMap<ValueId, ValueId>,
    visiting: &mut BTreeSet<ValueId>,
) -> ValueId {
    let Some(arguments) = raw.get(&value) else {
        return value;
    };
    if let Some(resolved) = memo.get(&value) {
        return *resolved;
    }
    if !visiting.insert(value) {
        return value;
    }
    let mut resolved = arguments
        .iter()
        .map(|argument| resolve_in(*argument, raw, memo, visiting));
    let first = resolved
        .next()
        .expect("an inventoried edge argument list is never empty");
    let result = if resolved.all(|other| other == first) {
        first
    } else {
        value
    };
    visiting.remove(&value);
    memo.insert(value, result);
    result
}

fn retain_proposition(
    proposition: &semantic_vocabulary::Proposition,
    retained_values: &mut BTreeSet<ValueId>,
) {
    proposition.visit_value_ids(|value| {
        retained_values.insert(value);
    });
}

fn retain_crash_routes(routes: &[CrashRouteBucket], retained_values: &mut BTreeSet<ValueId>) {
    for bucket in routes {
        for alternative in &bucket.alternatives {
            if let CrashRouteGuard::Predicate(term) = alternative {
                retain_proposition(term.proposition(), retained_values);
            }
        }
    }
}

fn crash_continuations(kind: &O) -> &[CrashRouteBucket] {
    match kind {
        O::Call {
            crash_continuations,
            ..
        }
        | O::CallUnit {
            crash_continuations,
            ..
        }
        | O::CallStructuralScalar {
            crash_continuations,
            ..
        }
        | O::CallDynamicScalar {
            crash_continuations,
            ..
        }
        | O::CallDynamicParameterScalar {
            crash_continuations,
            ..
        }
        | O::CallDynamicUnit {
            crash_continuations,
            ..
        }
        | O::CallDynamicParameterUnit {
            crash_continuations,
            ..
        }
        | O::CallStructural {
            crash_continuations,
            ..
        }
        | O::CallStructuralWithScalarArguments {
            crash_continuations,
            ..
        } => crash_continuations,
        _ => &[],
    }
}

/// Drop the listed old positions from one edge's scalar arguments.
fn drop_positions(arguments: &mut Vec<ValueId>, removed: Option<&Vec<usize>>) {
    let Some(removed) = removed else { return };
    let mut position = 0;
    arguments.retain(|_| {
        let retained = !removed.contains(&position);
        position += 1;
        retained
    });
}
