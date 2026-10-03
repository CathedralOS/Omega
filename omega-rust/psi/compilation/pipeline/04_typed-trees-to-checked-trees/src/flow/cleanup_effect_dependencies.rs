//! Project selected cleanup dispositions into the ordinary effect graph.
//!
//! Custody, not a type mention, selects each root. Its already-transferred
//! subtrees are excluded before nominal hooks are visited. This is a may-effect
//! set: an unknown active case contributes its possible remaining variants;
//! repeated array elements contribute one target edge, not invented call sites.
use super::carried_semantic_dependencies::{event_statement_index, event_type_reference};
use crate::checks::CleanupDispositions;
use arena::Arena;
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use facts::PlaceSegment;
use flow_effects::{CleanupDependency, OperationalPlan};
use language_semantics::{MachineSupplyMode, PermissionAccess, PermissionEventKind};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

type Substitutions = Vec<(SymbolHandle, TypeReferenceHandle)>;

pub(crate) fn derive_cleanup_effect_dependencies(
    program: &TypedTrees,
    operational: &OperationalPlan,
    facts: &CheckFacts,
    dispositions: &CleanupDispositions,
) -> Result<Arena<CleanupDependency>, Vec<Diagnostic>> {
    let mut dependencies = Arena::default();
    let proof_only = validation::proof_only_classification(program);
    // Specialization retains authored templates and their symbolic custody
    // rows, but only closed instances execute. Do not let a concrete caller
    // consume a template's provisional effect summary as an empty one.
    for caller in operational.machines() {
        let Some(machine) = program.machine_by_symbol(caller.symbol) else {
            continue;
        };
        if machine.supply_mode != MachineSupplyMode::CheckedBody
            || !program.machine_type_parameters(machine).is_empty()
            || proof_only.is_proof_machine(program, machine)
        {
            continue;
        }
        for state in operational.states.span_or_empty(caller.states) {
            for call in operational.calls.span_or_empty(state.calls) {
                if let Some(target) = program.machine_by_symbol(call.target_machine_symbol)
                    && !program.machine_type_parameters(target).is_empty()
                {
                    return Err(vec![Diagnostic::error(format!(
                        "cleanup effect preparation for `{}` requires a complete specialization of `{}`",
                        machine.name, target.name,
                    ))]);
                }
            }
        }
    }
    for (_, disposition) in dispositions.rows.iter() {
        let event = &disposition.event;
        if event.kind != PermissionEventKind::AffineDrop
            || event.access != PermissionAccess::Owned
            || !program
                .machine_by_symbol(event.machine_symbol)
                .is_some_and(|machine| {
                    machine.supply_mode == MachineSupplyMode::CheckedBody
                        // The source ledger still owns the template's symbolic
                        // disposition. Each exact instance has its own substituted
                        // body and custody rows; never union instance effects back
                        // onto the template or another application.
                        && program.machine_type_parameters(machine).is_empty()
                        && !proof_only.is_proof_machine(program, machine)
                })
        {
            continue;
        }
        let path = facts.flow.ownership.segments.span_or_empty(event.segments);
        let removed = dispositions
            .exclusions
            .span_or_empty(disposition.exclusions)
            .iter()
            .map(|place| dispositions.segments.span_or_empty(place.segments))
            .collect::<Vec<_>>();
        if removed.iter().any(|removed| path.starts_with(removed)) {
            continue;
        }
        let remaining = removed
            .iter()
            .filter_map(|removed| removed.strip_prefix(path))
            .collect::<Vec<_>>();
        let statement = event_statement_index(program, event.state_symbol, event.source);
        let Some(reference) =
            event_type_reference(program, event.state_symbol, statement, event.root, &[])
        else {
            return Err(vec![Diagnostic::error(
                "selected cleanup has no exact source storage type",
            )]);
        };
        let mut targets = Vec::new();
        let selected = select_type(program, reference, &[], path);
        if selected.is_none_or(|(reference, substitutions)| {
            visit(
                program,
                reference,
                &substitutions,
                &remaining,
                &mut Vec::new(),
                &mut targets,
            )
            .is_none()
        }) {
            return Err(vec![Diagnostic::error(format!(
                "cleanup effect dependency in `{}` has no exact structural residual disposition",
                program.symbols.name(event.machine_symbol),
            ))]);
        }
        for target in targets {
            let Some(machine) = program.machine_by_symbol(target) else {
                continue;
            };
            let Some(entry) = program.machine_states(machine).first() else {
                continue;
            };
            let edge = CleanupDependency {
                caller_machine: event.machine_symbol,
                caller_state: event.state_symbol,
                target_machine: target,
                target_state: entry.symbol,
            };
            if !dependencies.iter().any(|(_, prior)| prior == &edge) {
                dependencies.append(edge);
            }
        }
    }
    Ok(dependencies)
}

fn visit(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    removed: &[&[PlaceSegment]],
    active: &mut Vec<TypeReferenceHandle>,
    targets: &mut Vec<SymbolHandle>,
) -> Option<()> {
    if removed.iter().any(|path| path.is_empty()) {
        return Some(());
    }
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => {
                return Some(());
            }
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                if let Some((_, replacement)) =
                    substitutions.iter().rev().find(|(parameter, replacement)| {
                        parameter == symbol && *replacement != reference
                    })
                {
                    reference = *replacement;
                } else {
                    break;
                }
            }
            _ => break,
        }
    }
    // Re-entering the same owned type-reference node along one field chain
    // requires recursive runtime storage, not a finite residual expansion.
    if active.contains(&reference) {
        return None;
    }
    active.push(reference);
    let result = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let FixedArrayLength::Literal(length) = length else {
                return None;
            };
            visit_array(
                program,
                *element_type,
                *length,
                substitutions,
                removed,
                active,
                targets,
            )
        }
        TypeReferenceNode::Named { symbol, .. } => {
            visit_owner(program, *symbol, substitutions, removed, active, targets)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let data = program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == *base_symbol)?;
            let parameters = program.data_type_parameters(data);
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            if parameters.len() != arguments.len() {
                return None;
            }
            let mut nested: Substitutions = substitutions.to_vec();
            nested.extend(
                parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| (parameter.symbol, *argument)),
            );
            visit_owner(program, *base_symbol, &nested, removed, active, targets)
        }
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit
        | TypeReferenceNode::ConstExpression(_) => Some(()),
        TypeReferenceNode::DynamicTrait { .. } => None,
        TypeReferenceNode::Constrained { .. } => unreachable!("constraints were peeled"),
    };
    active.pop();
    result
}

fn visit_owner(
    program: &TypedTrees,
    symbol: SymbolHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    removed: &[&[PlaceSegment]],
    active: &mut Vec<TypeReferenceHandle>,
    targets: &mut Vec<SymbolHandle>,
) -> Option<()> {
    let symbol = program
        .machine_by_symbol(symbol)
        .map_or(symbol, |machine| machine.attached_data_symbol);
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == symbol)
    else {
        // Only authenticated primitive leaves have no hidden nominal storage.
        // An unsubstituted type parameter or dynamic owner cannot publish a
        // pure cleanup effect merely because its concrete data is unavailable.
        return (program.symbols.get(symbol).kind == symbols::SymbolKind::BuiltinType)
            .then_some(());
    };
    if let Some(target) = crate::checks::nominal_drop_machine_symbol(program, symbol) {
        // A nominal hook requires the whole value. Partial transfer across
        // that prefix is independently forbidden by custody validation.
        if !removed.is_empty() {
            return None;
        }
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    let mut matched = 0;
    for member in program.data_members(data) {
        match member {
            DataMember::Field(field) if !field.relevance.is_erased() => {
                let child = selected(
                    removed,
                    PlaceSegment::Field {
                        symbol: field.symbol,
                    },
                );
                matched += child.len();
                visit(
                    program,
                    field.type_reference,
                    substitutions,
                    &child,
                    active,
                    targets,
                )?;
            }
            DataMember::Variant(variant) => {
                let case = selected(
                    removed,
                    PlaceSegment::Case {
                        variant: variant.symbol,
                    },
                );
                matched += case.len();
                if case.iter().any(|path| path.is_empty()) {
                    continue;
                }
                let mut case_matched = 0;
                for field in program
                    .data_payload_fields(variant)
                    .iter()
                    .filter(|field| !field.relevance.is_erased())
                {
                    let child = selected(
                        &case,
                        PlaceSegment::Field {
                            symbol: field.symbol,
                        },
                    );
                    case_matched += child.len();
                    visit(
                        program,
                        field.type_reference,
                        substitutions,
                        &child,
                        active,
                        targets,
                    )?;
                }
                if case_matched != case.len() {
                    return None;
                }
            }
            DataMember::Field(_) => {}
        }
    }
    (matched == removed.len()).then_some(())
}

fn selected<'a>(removed: &[&'a [PlaceSegment]], segment: PlaceSegment) -> Vec<&'a [PlaceSegment]> {
    removed
        .iter()
        .filter_map(|path| path.strip_prefix(&[segment]))
        .collect()
}

fn visit_array(
    program: &TypedTrees,
    element: TypeReferenceHandle,
    length: usize,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    removed: &[&[PlaceSegment]],
    active: &mut Vec<TypeReferenceHandle>,
    targets: &mut Vec<SymbolHandle>,
) -> Option<()> {
    // Only endpoints named by custody participate. This is an effect set, so
    // all untouched elements share one traversal, even for enormous arrays.
    let mut cuts = vec![0, length];
    for path in removed {
        match path.first()? {
            PlaceSegment::FixedIndex { index } if *index < length => {
                cuts.extend([*index, index + 1]);
            }
            PlaceSegment::FixedRange { start, end }
                if start <= end && *end <= length && path.len() == 1 =>
            {
                cuts.extend([*start, *end]);
            }
            _ => return None,
        }
    }
    cuts.sort_unstable();
    cuts.dedup();
    for pair in cuts.windows(2) {
        let index = pair[0];
        if index == pair[1] {
            continue;
        }
        if removed.iter().any(|path| {
            matches!(path.first(),
            Some(PlaceSegment::FixedRange { start, end }) if *start <= index && index < *end)
        }) {
            continue;
        }
        let children = selected(removed, PlaceSegment::FixedIndex { index });
        visit(program, element, substitutions, &children, active, targets)?;
    }
    Some(())
}

/// Resolve a selected cleanup subtree through declaration identities while
/// retaining generic substitutions; a textual member spelling is no authority.
fn select_type(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &[PlaceSegment],
) -> Option<(TypeReferenceHandle, Substitutions)> {
    if path.is_empty() {
        return Some((reference, substitutions.to_vec()));
    }
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, .. } => {
                if let Some((_, replacement)) =
                    substitutions.iter().rev().find(|(parameter, replacement)| {
                        parameter == symbol && *replacement != reference
                    })
                {
                    reference = *replacement;
                } else {
                    break;
                }
            }
            _ => break,
        }
    }
    let (symbol, nested) = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => {
            let PlaceSegment::FixedIndex { index } = path[0] else {
                return None;
            };
            return (index < *length)
                .then(|| select_type(program, *element_type, substitutions, &path[1..]))
                .flatten();
        }
        TypeReferenceNode::Named { symbol, .. } => (*symbol, substitutions.to_vec()),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let data = program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == *base_symbol)?;
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let parameters = program.data_type_parameters(data);
            if arguments.len() != parameters.len() {
                return None;
            }
            let mut nested = substitutions.to_vec();
            nested.extend(
                parameters
                    .iter()
                    .zip(arguments)
                    .map(|(parameter, argument)| (parameter.symbol, *argument)),
            );
            (*base_symbol, nested)
        }
        _ => return None,
    };
    let symbol = program
        .machine_by_symbol(symbol)
        .map_or(symbol, |machine| machine.attached_data_symbol);
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == symbol)?;
    let members = program.data_members(data);
    let (field, suffix) = match path[0] {
        PlaceSegment::Field { symbol } => {
            let field = members.iter().find_map(|member| match member {
                DataMember::Field(field) if field.symbol == symbol => Some(field),
                _ => None,
            })?;
            (field, &path[1..])
        }
        PlaceSegment::Case { variant } => {
            let variant = members.iter().find_map(|member| match member {
                DataMember::Variant(candidate) if candidate.symbol == variant => Some(candidate),
                _ => None,
            })?;
            let PlaceSegment::Field { symbol } = *path.get(1)? else {
                return None;
            };
            let field = program
                .data_payload_fields(variant)
                .iter()
                .find(|field| field.symbol == symbol)?;
            (field, &path[2..])
        }
        _ => return None,
    };
    if field.relevance.is_erased() {
        return None;
    }
    select_type(program, field.type_reference, &nested, suffix)
}
