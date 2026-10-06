//! Project selected cleanup dispositions into the ordinary effect graph.
//!
//! Custody, not a type mention, selects each root. Its already-transferred
//! subtrees are excluded before nominal hooks are visited. This is a may-effect
//! set: an unknown active case contributes its possible remaining variants;
//! repeated array elements contribute one target edge, not invented call sites.
use super::carried_semantic_dependencies::{event_statement_index, event_type_reference};
use arena::Arena;
use checked_trees::{CheckFacts, ContextualCleanupDisposition};
use diagnostics::Diagnostic;
use facts::PlaceSegment;
use flow_effects::{CleanupDependency, OperationalPlan};
use language_semantics::{MachineSupplyMode, Multiplicity};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

type Substitutions = Vec<(SymbolHandle, TypeReferenceHandle)>;

/// Effect discovery retains array windows once; proof sequencing and executable
/// cleanup share their exact selected receivers. Reserve before writing them.
pub(crate) fn concrete_cleanup_paths(path: &[PlaceSegment]) -> Option<Vec<Vec<PlaceSegment>>> {
    let mut paths = vec![Vec::new()];
    for segment in path {
        if let PlaceSegment::FixedRange { start, end } = *segment {
            let count = paths.len().checked_mul(end.checked_sub(start)?)?;
            let mut expanded = Vec::new();
            expanded.try_reserve_exact(count).ok()?;
            for prefix in paths {
                for index in start..end {
                    let mut selected = prefix.clone();
                    selected.push(PlaceSegment::FixedIndex { index });
                    expanded.push(selected);
                }
            }
            paths = expanded;
        } else {
            for prefix in &mut paths {
                prefix.push(*segment);
            }
        }
    }
    Some(paths)
}

/// A selected owned cleanup leaf, relative to `disposition.root` and including
/// its selected path. A FixedRange denotes every element of that half-open
/// interval; it is never authority for just one representative element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CleanupLeaf {
    Nominal {
        machine: SymbolHandle,
        path: Vec<PlaceSegment>,
    },
    Symbolic {
        parameter: SymbolHandle,
        path: Vec<PlaceSegment>,
    },
}

/// Walk the selected residual once for both effects and generic obligations.
/// Unknown storage is `None`, distinct from an empty, cleanup-free residual.
pub(crate) fn selected_cleanup_leaves(
    program: &TypedTrees,
    facts: &CheckFacts,
    disposition: &ContextualCleanupDisposition,
) -> Option<Vec<CleanupLeaf>> {
    let dispositions = &facts.contextual_cleanup;
    let path = facts.flow.ownership.segments.span(disposition.path)?;
    let removed = dispositions
        .exclusions
        .span(disposition.exclusions)?
        .iter()
        .map(|place| dispositions.segments.span(place.segments))
        .collect::<Option<Vec<_>>>()?;
    if removed.iter().any(|removed| path.starts_with(removed)) {
        return Some(Vec::new());
    }
    let remaining = removed
        .iter()
        .filter_map(|removed| removed.strip_prefix(path))
        .collect::<Vec<_>>();
    let statement = event_statement_index(program, disposition.state, disposition.source);
    let (reference, substitutions) = if disposition.disposed_type.is_valid() {
        (disposition.disposed_type, Vec::new())
    } else {
        let reference =
            event_type_reference(program, disposition.state, statement, disposition.root, &[])?;
        select_type(program, reference, &[], path)?
    };
    let mut leaves = Vec::new();
    visit(
        program,
        reference,
        &substitutions,
        &remaining,
        &mut Vec::new(),
        &mut path.to_vec(),
        &mut leaves,
        &mut false,
    )?;
    Some(leaves)
}

/// Type-side eligibility for a whole owned death edge. This is not proof of
/// the selected hooks' requirements or guarantees: the resulting contextual
/// disposition still enters ordinary cleanup effect and contract admission.
/// In particular, a child hook cannot authorize a distinct nominal linear
/// obligation, and an owner hook cannot erase an undisposable child.
pub(crate) fn whole_cleanup_is_eligible(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    let mut has_erased_members = false;
    let complete = visit(
        program,
        reference,
        &[],
        &[],
        &mut Vec::new(),
        &mut Vec::new(),
        &mut Vec::new(),
        &mut has_erased_members,
    )
    .is_some();
    // Runtime cleanup leaves deliberately omit erased fields. They cannot
    // authorize whole linear consumption: erased semantic obligations need
    // their own disposition evidence before this query can admit that owner.
    complete && !has_erased_members
}

pub(crate) fn derive_cleanup_effect_dependencies(
    program: &TypedTrees,
    operational: &OperationalPlan,
    facts: &CheckFacts,
) -> Result<Arena<CleanupDependency>, Vec<Diagnostic>> {
    let dispositions = &facts.contextual_cleanup;
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
    for (_, disposition) in dispositions.dispositions.iter() {
        if !program
            .machine_by_symbol(disposition.machine)
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
        let invalid = || {
            vec![Diagnostic::error(format!(
                "cleanup effect dependency in `{}` has no exact structural residual disposition",
                program.symbols.name(disposition.machine),
            ))]
        };
        let leaves = selected_cleanup_leaves(program, facts, disposition).ok_or_else(invalid)?;
        for leaf in leaves {
            let CleanupLeaf::Nominal {
                machine: target, ..
            } = leaf
            else {
                return Err(invalid());
            };
            let Some(machine) = program.machine_by_symbol(target) else {
                continue;
            };
            let Some(entry) = program.machine_states(machine).first() else {
                continue;
            };
            let edge = CleanupDependency {
                caller_machine: disposition.machine,
                caller_state: disposition.state,
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
    path: &mut Vec<PlaceSegment>,
    leaves: &mut Vec<CleanupLeaf>,
    has_erased_members: &mut bool,
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
            let length = program.closed_array_extent(length, substitutions)?;
            visit_array(
                program,
                *element_type,
                length,
                substitutions,
                removed,
                active,
                path,
                leaves,
                has_erased_members,
            )
        }
        TypeReferenceNode::Named { symbol, .. } => visit_owner(
            program,
            *symbol,
            substitutions,
            removed,
            active,
            path,
            leaves,
            has_erased_members,
        ),
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
            visit_owner(
                program,
                *base_symbol,
                &nested,
                removed,
                active,
                path,
                leaves,
                has_erased_members,
            )
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
    path: &mut Vec<PlaceSegment>,
    leaves: &mut Vec<CleanupLeaf>,
    has_erased_members: &mut bool,
) -> Option<()> {
    let symbol = program
        .machine_by_symbol(symbol)
        .map_or(symbol, |machine| machine.attached_data_symbol);
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == symbol)
    else {
        return match program.symbols.get(symbol).kind {
            symbols::SymbolKind::BuiltinType => Some(()),
            symbols::SymbolKind::TypeParameter if removed.is_empty() => {
                leaves.push(CleanupLeaf::Symbolic {
                    parameter: symbol,
                    path: path.clone(),
                });
                Some(())
            }
            _ => None,
        };
    };
    let nominal = crate::checks::nominal_drop_machine_symbol(program, symbol);
    if data.properties.multiplicity == Multiplicity::Linear && nominal.is_none() {
        return None;
    }
    if let Some(target) = nominal {
        // A nominal hook requires the whole value. Partial transfer across
        // that prefix is independently forbidden by custody validation.
        if !removed.is_empty() {
            return None;
        }
        leaves.push(CleanupLeaf::Nominal {
            machine: target,
            path: path.clone(),
        });
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
                path.push(PlaceSegment::Field {
                    symbol: field.symbol,
                });
                visit(
                    program,
                    field.type_reference,
                    substitutions,
                    &child,
                    active,
                    path,
                    leaves,
                    has_erased_members,
                )?;
                path.pop();
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
                path.push(PlaceSegment::Case {
                    variant: variant.symbol,
                });
                let mut case_matched = 0;
                *has_erased_members |= program
                    .data_payload_fields(variant)
                    .iter()
                    .any(|field| field.relevance.is_erased());
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
                    path.push(PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    visit(
                        program,
                        field.type_reference,
                        substitutions,
                        &child,
                        active,
                        path,
                        leaves,
                        has_erased_members,
                    )?;
                    path.pop();
                }
                path.pop();
                if case_matched != case.len() {
                    return None;
                }
            }
            DataMember::Field(_) => *has_erased_members = true,
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
    path: &mut Vec<PlaceSegment>,
    leaves: &mut Vec<CleanupLeaf>,
    has_erased_members: &mut bool,
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
        path.push(if pair[1] == index + 1 {
            PlaceSegment::FixedIndex { index }
        } else {
            PlaceSegment::FixedRange {
                start: index,
                end: pair[1],
            }
        });
        visit(
            program,
            element,
            substitutions,
            &children,
            active,
            path,
            leaves,
            has_erased_members,
        )?;
        path.pop();
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
            length,
        } => {
            let length = program.closed_array_extent(length, substitutions)?;
            let PlaceSegment::FixedIndex { index } = path[0] else {
                return None;
            };
            return (index < length)
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
