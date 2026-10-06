//! Affine obligations an ordinary Unit return edge owes: whole-root discards and
//! the residual complement of a partially moved owned parameter.

use super::*;

/// Whole-root discards plus the residual complement of at most one partially
/// moved owned parameter on the ordinary Unit return edge. A projected owned
/// actual transfers only the selected subtree into the callee, so the
/// parameter's untouched remainder still dies on this return and must
/// publish residual rows rather than disappear from cleanup. Result/local
/// lifetimes are scheduled by their establishment and consuming continuation.
/// Stage 05 completes their scheduled disposal from the emitted transfer
/// frontier; this parameter roster must not reject those projections based on
/// whether their producer was a call, construction, named local or temporary.
pub(in crate::execution::terminal_unit) fn return_unit_affine_discards(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    source_parameters: &[StateParameter],
    operations: &[CheckedUnitEffectOperationPlan],
    admitted_local_symbols: &[SymbolHandle],
    structural_types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
) -> Option<(Vec<u32>, Vec<CheckedUnitPartialAffineDiscardPlan>, bool)> {
    let mut transferred_parameters = BTreeSet::new();
    let mut moved_parameter_paths =
        BTreeMap::<u32, Vec<(Vec<CheckedUnitStructuralPathSegment>, String)>>::new();
    for operation in operations
        .iter()
        .flat_map(|operation| operation.with_value_calls())
    {
        if let Some(call) = operation.call() {
            for argument in &call.structural_arguments {
                if argument.access != CheckedStructuralAccess::Owned {
                    continue;
                }
                if let Some(parameter_index) = argument.source_parameter_index() {
                    if argument.path.is_empty() {
                        transferred_parameters.insert(parameter_index);
                    } else {
                        moved_parameter_paths
                            .entry(parameter_index)
                            .or_default()
                            .push((argument.path.clone(), argument.type_identity.clone()));
                    }
                }
            }
        } else if let CheckedUnitEffectOperationPlan::EstablishReference { source, .. } = operation
        {
            // A projected reference establishment moves its carrier's
            // leaf into the result and consumes the carrier whole; the
            // carrier owes no separate exit discard. The whole-ingress
            // lane keeps an empty path and leaves the borrowed parameter
            // on the ordinary roster.
            if !source.path.is_empty()
                && let Some(parameter_index) = source.source_parameter_index()
            {
                transferred_parameters.insert(parameter_index);
            }
        }
    }
    let mut residual_roots = BTreeSet::new();
    let mut residual_affine_discards = Vec::new();
    for (parameter_index, moved_paths) in &moved_parameter_paths {
        // A root also passed whole is fully consumed; nothing remains to
        // discard and no exit custody exists.
        if transferred_parameters.contains(parameter_index) {
            continue;
        }
        let parameter = structural_parameters.get(*parameter_index as usize)?;
        // Residual rows are owed only for a root whose disposal dies on
        // this edge — an owned parameter. A borrowed root's untouched
        // complement returns to its caller: the projected move already
        // published the member's hole through the call's argument
        // custody, and no part of that storage dies here.
        if parameter.access != CheckedStructuralAccess::Owned {
            continue;
        }
        // A qualified owned root's residual complement still dies on
        // this edge, but the partial-affine carrier has no qualified
        // residual vocabulary to publish it with.
        if !parameter.qualifications.is_empty() {
            return None;
        }
        let rows = crate::execution::terminal_unit::cleanup::partial_affine_residuals(
            structural_types,
            &CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: *parameter_index,
            },
            &parameter.type_identity,
            moved_paths,
        )?;
        // Projections covering the whole root leave no complement.
        if rows.is_empty() {
            continue;
        }
        // Only an affine root may close a complement on the return edge.
        // A linear root must be used exactly once, so an unmoved remainder
        // is a program error rather than exit cleanup, and this edge has no
        // vocabulary for it.
        if parameter.multiplicity != Multiplicity::Affine {
            return None;
        }
        // The dedicated partial-affine terminator closes exactly one
        // partially moved root; a second root has no return-edge vocabulary.
        if !residual_roots.is_empty() {
            return None;
        }
        residual_roots.insert(*parameter_index);
        residual_affine_discards = rows;
    }
    // A projected move out of an owned affine parameter keeps the body on
    // the partial-affine carrier — even when the projections cover the root
    // and owe no residual rows — so the root-only roster stays free of
    // affine argument custody the ordinary return edge cannot close. A
    // linear root owes that edge nothing, so its projections leave it on the
    // ordinary roster where the legacy indexed cohort already lives.
    let has_projected_parameter_moves = moved_parameter_paths.keys().any(|index| {
        !transferred_parameters.contains(index)
            && structural_parameters
                .get(*index as usize)
                .is_some_and(|parameter| {
                    parameter.access == CheckedStructuralAccess::Owned
                        && parameter.qualifications.is_empty()
                        && parameter.multiplicity == Multiplicity::Affine
                })
    });
    // Each selected exit records its own disposal. Join equal owners through
    // the permission table's authenticated exit union, which still rejects
    // duplicate disposal on one exit and differing custody for the same root.
    let events = facts
        .flow
        .ownership
        .state_exit_cleanup_roots(machine, state)?
        .into_iter()
        .filter(|event| {
            event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Affine
                && event.claim_identity == PermissionClaimIdentity::Unknown
                && event.provenance == language_semantics::PermissionProvenance::Unknown
                && !event.obligation_live
                && facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .is_empty()
        })
        .collect::<Vec<_>>();
    let mut output = Vec::with_capacity(events.len());
    for event in events {
        let parameter_index = structural_parameters.iter().position(|parameter| {
            source_parameters
                .get(parameter.position as usize)
                .is_some_and(|source| event.root == facts::PlaceRoot::Symbol((source).symbol))
        });
        if parameter_index.is_none() {
            let facts::PlaceRoot::Symbol(root) = event.root else {
                return None;
            };
            // An erased formal has no runtime storage and no roster row, so
            // its exit drop owes the return edge nothing.
            let erased_formal = source_parameters.iter().any(|source| {
                strips_erased_parameter(source) == Some(true) && root == (source).symbol
            });
            if erased_formal || admitted_local_symbols.contains(&root) {
                continue;
            }
            return None;
        }
        let parameter_index = parameter_index?;
        let parameter = &structural_parameters[parameter_index];
        let source_parameter = source_parameters.get(parameter.position as usize)?;
        if parameter.multiplicity != Multiplicity::Affine
            || type_graph_requires_nominal_drop(program, source_parameter.type_reference)
            || output.contains(&(parameter_index as u32))
        {
            return None;
        }
        let parameter_index = u32::try_from(parameter_index).ok()?;
        if transferred_parameters.contains(&parameter_index)
            || residual_roots.contains(&parameter_index)
        {
            continue;
        }
        // A parameter consumed in full by its projections still records a
        // whole-root exit drop in the permission ledger; the reconstructed
        // complement is empty, so neither a trivial row nor residual rows
        // belong to it.
        if moved_parameter_paths.contains_key(&parameter_index) {
            continue;
        }
        output.push(parameter_index);
    }
    Some((
        output,
        residual_affine_discards,
        has_projected_parameter_moves,
    ))
}

pub(in crate::execution::terminal_unit) fn checked_no_code_affine_discard_positions(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
) -> Option<Vec<u32>> {
    let positions = crate::execution::terminal_cleanup::checked_whole_affine_discard_parameters(
        program, facts, machine, state,
    )?
    .into_iter()
    .map(|(_, position)| position)
    .collect::<Vec<_>>();
    if positions.iter().any(|position| {
        program
            .state_parameters(state)
            .get(*position as usize)
            .is_none_or(|parameter| {
                type_graph_requires_nominal_drop(program, parameter.type_reference)
            })
    }) {
        return None;
    }
    Some(positions)
}

/// Check source access independently of structural shape collection. Generic
/// arguments and declaration fields are both inspected: substituting an owned
/// argument cannot turn an authored reference field into owned storage.
pub(super) fn partial_affine_source_contents_are_owned(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    visited: &mut Vec<SymbolHandle>,
) -> bool {
    if !reference.is_valid() {
        return false;
    }
    // Range and policy constraints do not give primitive fields cleanup or
    // reference access. This classifier peels constraints but rejects references.
    if program.primitive_type_reference(reference).is_some() {
        return true;
    }
    // Preserve the existing no-cleanup bounded-byte field carrier, whose
    // domain spelling is retained by that carrier's own source classifier.
    if structural_access_for_type_reference(program, reference)
        == Some(CheckedStructuralAccess::Owned)
        && matches!(
            byte_sequence_carrier(program, reference, &[]),
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. })
        )
    {
        return true;
    }
    let symbol = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => *symbol,
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            if !program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .all(|argument| {
                    partial_affine_source_contents_are_owned(program, *argument, visited)
                })
            {
                return false;
            }
            *base_symbol
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(1..),
        } => return partial_affine_source_contents_are_owned(program, *element_type, visited),
        _ => return false,
    };
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == symbol)
    else {
        // Concrete arguments were checked at their application; an authored
        // type binder has no reference or qualification wrapper of its own.
        return program.data_definitions().iter().any(|data| {
            program
                .data_type_parameters(data)
                .iter()
                .any(|parameter| parameter.symbol == symbol)
        });
    };
    if visited.contains(&symbol) {
        return true;
    }
    visited.push(symbol);
    program.data_members(data).iter().all(|member| {
        // A selected case owns its whole payload: union members carry owned
        // contents exactly when every payload field is owned, the same
        // classification a record field takes.
        let fields: &[typed_trees::data::DataField] = match member {
            DataMember::Field(field) => std::slice::from_ref(field),
            DataMember::Variant(variant) => program.data_payload_fields(variant),
        };
        fields.iter().all(|field| {
            !field.relevance.is_erased()
                && partial_affine_source_contents_are_owned(program, field.type_reference, visited)
        })
    })
}
