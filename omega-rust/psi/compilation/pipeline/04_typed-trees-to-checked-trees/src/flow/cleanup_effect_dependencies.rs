//! Project selected cleanup dispositions into the ordinary effect graph.
//!
//! Custody, not a type mention, selects each root. Its transferred or absent
//! subtrees are excluded before nominal hooks are visited. This is a may-effect
//! set: an unknown active case contributes its possible remaining variants,
//! while a case the occurrence's source facts rule out holds no payload;
//! repeated array elements contribute one target edge, not invented call sites.
use super::carried_semantic_dependencies::{event_statement_index, event_type_reference};
use arena::Arena;
use checked_trees::{CheckFacts, ContextualCleanupDisposition};
use diagnostics::Diagnostic;
use facts::PlaceSegment;
use flow_effects::{CleanupDependency, OperationalPlan};
use language_semantics::{MachineSupplyMode, Multiplicity};
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

type Substitutions = Vec<(SymbolHandle, TypeReferenceHandle)>;

/// Whether the occurrence's facts rule out the case a residual path selects.
type InactiveCase<'a> = dyn Fn(&[PlaceSegment]) -> bool + 'a;

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
    program: &dyn ::validation::CheckingInput,
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
    // A zero image established no value: nothing selects a hook, and the
    // linear-owner refusal below guards values, not zero-filled storage.
    if disposition.zero_image {
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
    let contexts = facts
        .flow
        .control
        .states
        .iter()
        .find(|(_, state)| {
            state.machine_symbol == disposition.machine && state.state_symbol == disposition.state
        })
        .and_then(|(_, state)| {
            crate::facts::contextual_cleanup::occurrence_contexts(
                program,
                facts,
                state,
                disposition,
            )
        });
    // The occurrence's source facts decide which alternatives can hold a
    // payload here. Without contexts every alternative remains possible.
    let inactive_case = |segments: &[PlaceSegment]| {
        contexts.as_ref().is_some_and(|contexts| {
            contexts.iter().all(|(statement, contexts)| {
                crate::flow::place_case_path_is_inactive(
                    program,
                    &facts.semantic,
                    contexts,
                    disposition.machine,
                    disposition.state,
                    *statement,
                    &validation::CanonicalPlace {
                        root: disposition.root,
                        segments: segments.to_vec(),
                    },
                )
            })
        })
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
        &inactive_case,
    )?;
    Some(leaves)
}

/// Type-side eligibility for a whole owned death edge. This is not proof of
/// the selected hooks' requirements or guarantees: the resulting contextual
/// disposition still enters ordinary cleanup effect and contract admission.
/// In particular, a child hook cannot authorize a distinct nominal linear
/// obligation, and an owner hook cannot erase an undisposable child.
pub(crate) fn whole_cleanup_is_eligible(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    residual_cleanup_is_eligible(program, reference, &[])
}

/// The same type-side plan restricted to the authenticated remaining region.
pub(crate) fn residual_cleanup_is_eligible(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    removed: &[&[PlaceSegment]],
) -> bool {
    let mut has_erased_obligations = false;
    let complete = visit(
        program,
        reference,
        &[],
        removed,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut Vec::new(),
        &mut has_erased_obligations,
        &|_| false,
    )
    .is_some();
    // Runtime cleanup omits erased fields. Ordinary unrestricted subjects
    // owe no consuming disposition; other erased subjects conservatively need
    // semantic disposition evidence that this runtime-only query cannot supply.
    complete && !has_erased_obligations
}

pub(crate) fn derive_cleanup_effect_dependencies(
    program: &dyn ::validation::CheckingInput,
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

/// Descend into one field child under its `Field` path segment, then
/// restore the path.
#[allow(clippy::too_many_arguments)]
fn visit_field_child(
    program: &dyn ::validation::CheckingInput,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    child: &[&[PlaceSegment]],
    active: &mut Vec<TypeReferenceHandle>,
    path: &mut Vec<PlaceSegment>,
    leaves: &mut Vec<CleanupLeaf>,
    has_erased_obligations: &mut bool,
    inactive_case: &InactiveCase<'_>,
    field: &typed_trees::data::DataField,
) -> Option<()> {
    path.push(PlaceSegment::Field {
        symbol: field.symbol,
    });
    visit(
        program,
        field.type_reference,
        substitutions,
        child,
        active,
        path,
        leaves,
        has_erased_obligations,
        inactive_case,
    )?;
    path.pop();
    Some(())
}

fn visit(
    program: &dyn ::validation::CheckingInput,
    mut reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    removed: &[&[PlaceSegment]],
    active: &mut Vec<TypeReferenceHandle>,
    path: &mut Vec<PlaceSegment>,
    leaves: &mut Vec<CleanupLeaf>,
    has_erased_obligations: &mut bool,
    inactive_case: &InactiveCase<'_>,
) -> Option<()> {
    if removed.iter().any(|path| path.is_empty()) {
        return Some(());
    }
    reference = resolve_peeled_type_reference(program, reference, substitutions);
    if matches!(
        program.type_reference_table.type_reference(reference),
        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. }
    ) {
        return Some(());
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
        } => match program.closed_array_extent(length, substitutions) {
            Some(length) => visit_array(
                program,
                *element_type,
                length,
                substitutions,
                removed,
                active,
                path,
                leaves,
                has_erased_obligations,
                inactive_case,
            ),
            // A retained recipe fixes the extent only at target closure. The
            // whole array still owes no cleanup when one element owes none
            // and no custody endpoint names an element; any leaf would carry
            // an extent the shared judgment cannot spell.
            None if program
                .constant_recipe_for_array_length(reference)
                .is_valid()
                && removed.is_empty() =>
            {
                let mut element_leaves = Vec::new();
                let mut element_erased = false;
                path.push(PlaceSegment::FixedIndex { index: 0 });
                let visited = visit(
                    program,
                    *element_type,
                    substitutions,
                    &[],
                    active,
                    path,
                    &mut element_leaves,
                    &mut element_erased,
                    inactive_case,
                );
                path.pop();
                (visited.is_some() && element_leaves.is_empty() && !element_erased).then_some(())
            }
            None => None,
        },
        TypeReferenceNode::Named { symbol, .. } => visit_owner(
            program,
            *symbol,
            reference,
            substitutions,
            removed,
            active,
            path,
            leaves,
            has_erased_obligations,
            inactive_case,
        ),
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let data = program.data_definition_by_symbol(*base_symbol)?;
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
                reference,
                &nested,
                removed,
                active,
                path,
                leaves,
                has_erased_obligations,
                inactive_case,
            )
        }
        // A selector names an implementation table and owns no data, so it
        // contributes no cleanup leaf.
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. } => Some(()),
        TypeReferenceNode::Constrained { .. } => unreachable!("constraints were peeled"),
    };
    active.pop();
    result
}

/// Peel `Constrained` shells and chase `Named` type parameters through
/// `substitutions` until the handle is stable. A replacement equal to the
/// current handle is skipped so an identity substitution cannot loop; the
/// reverse find keeps the innermost binding authoritative.
fn resolve_peeled_type_reference(
    program: &dyn ::validation::CheckingInput,
    mut reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> TypeReferenceHandle {
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
    reference
}

fn visit_owner(
    program: &dyn ::validation::CheckingInput,
    symbol: SymbolHandle,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    removed: &[&[PlaceSegment]],
    active: &mut Vec<TypeReferenceHandle>,
    path: &mut Vec<PlaceSegment>,
    leaves: &mut Vec<CleanupLeaf>,
    has_erased_obligations: &mut bool,
    inactive_case: &InactiveCase<'_>,
) -> Option<()> {
    let symbol = program
        .machine_by_symbol(symbol)
        .map_or(symbol, |machine| machine.attached_data_symbol);
    let Some(data) = program.data_definition_by_symbol(symbol) else {
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
    let nominal = validation::nominal_cleanup_machine_for_type(program, reference, substitutions)
        .map(|machine| machine.symbol);
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
                visit_field_child(
                    program,
                    substitutions,
                    &child,
                    active,
                    path,
                    leaves,
                    has_erased_obligations,
                    inactive_case,
                    field,
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
                path.push(PlaceSegment::Case {
                    variant: variant.symbol,
                });
                // A case the occurrence's facts rule out holds no payload to
                // dispose. Any custody named inside it was counted above.
                if inactive_case(path) {
                    path.pop();
                    continue;
                }
                let mut case_matched = 0;
                *has_erased_obligations |=
                    program.data_payload_fields(variant).iter().any(|field| {
                        field.relevance.is_erased()
                            && erased_subject_needs_disposition(
                                program,
                                field.type_reference,
                                substitutions,
                            )
                    });
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
                    visit_field_child(
                        program,
                        substitutions,
                        &child,
                        active,
                        path,
                        leaves,
                        has_erased_obligations,
                        inactive_case,
                        field,
                    )?;
                }
                path.pop();
                if case_matched != case.len() {
                    return None;
                }
            }
            DataMember::Field(field) => {
                *has_erased_obligations |=
                    erased_subject_needs_disposition(program, field.type_reference, substitutions);
            }
        }
    }
    (matched == removed.len()).then_some(())
}

/// A runtime cleanup cannot discharge a semantic-only obligation. Reuse the
/// complete closed contents judgment: only unrestricted plain-owned subjects
/// can be forgotten without an erased disposition, never open type parameters,
/// linear descendants, loans, nominal hooks or unclassified qualifications.
fn erased_subject_needs_disposition(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    let carrier = resolve_peeled_type_reference(program, reference, substitutions);
    program.type_multiplicity(carrier) != Multiplicity::Unrestricted
        || !validation::has_contents(
            program,
            reference,
            substitutions,
            validation::ContentsRequirement::PlainOwned,
        )
}

fn selected<'a>(removed: &[&'a [PlaceSegment]], segment: PlaceSegment) -> Vec<&'a [PlaceSegment]> {
    removed
        .iter()
        .filter_map(|path| path.strip_prefix(&[segment]))
        .collect()
}

fn visit_array(
    program: &dyn ::validation::CheckingInput,
    element: TypeReferenceHandle,
    length: usize,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    removed: &[&[PlaceSegment]],
    active: &mut Vec<TypeReferenceHandle>,
    path: &mut Vec<PlaceSegment>,
    leaves: &mut Vec<CleanupLeaf>,
    has_erased_obligations: &mut bool,
    inactive_case: &InactiveCase<'_>,
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
            has_erased_obligations,
            inactive_case,
        )?;
        path.pop();
    }
    Some(())
}

/// Resolve a selected cleanup subtree through declaration identities while
/// retaining generic substitutions; a textual member spelling is no authority.
fn select_type(
    program: &dyn ::validation::CheckingInput,
    mut reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    path: &[PlaceSegment],
) -> Option<(TypeReferenceHandle, Substitutions)> {
    if path.is_empty() {
        return Some((reference, substitutions.to_vec()));
    }
    reference = resolve_peeled_type_reference(program, reference, substitutions);
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
            let data = program.data_definition_by_symbol(*base_symbol)?;
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
    let data = program.data_definition_by_symbol(symbol)?;
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
            let field = program.data_payload_field_by_symbol(variant, symbol)?;
            (field, &path[2..])
        }
        _ => return None,
    };
    if field.relevance.is_erased() {
        return None;
    }
    select_type(program, field.type_reference, &nested, suffix)
}

/// Canonical completed-owner cleanup order shared by prerequisite checking and
/// executable partitions: parent hooks first, siblings in reverse declaration
/// or fixed-index order. No region or array elements are expanded here.
pub(crate) fn cleanup_path_order(
    program: &dyn validation::CheckingInput,
    path: &[PlaceSegment],
) -> Option<Vec<usize>> {
    path.iter()
        .map(|segment| segment_order(program, *segment))
        .collect()
}

fn segment_order(
    program: &dyn ::validation::CheckingInput,
    segment: PlaceSegment,
) -> Option<usize> {
    let ordinal = match segment {
        PlaceSegment::FixedIndex { index } => index,
        PlaceSegment::Field { symbol } | PlaceSegment::Case { variant: symbol } => {
            program.data_definitions().iter().find_map(|data| {
                for (ordinal, member) in program.data_members(data).iter().enumerate() {
                    match member {
                        DataMember::Field(field) if field.symbol == symbol => return Some(ordinal),
                        DataMember::Variant(variant) => {
                            if variant.symbol == symbol {
                                return Some(ordinal);
                            }
                            if let Some(ordinal) = program
                                .data_payload_fields(variant)
                                .iter()
                                .position(|field| field.symbol == symbol)
                            {
                                return Some(ordinal);
                            }
                        }
                        _ => {}
                    }
                }
                None
            })?
        }
        PlaceSegment::FixedRange { .. } | PlaceSegment::Index { .. } => return None,
    };
    Some(usize::MAX - ordinal)
}
