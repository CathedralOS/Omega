//! Affine obligations an ordinary Unit return edge owes: whole-root discards and
//! the residual complement of a partially moved owned parameter.

use super::*;

/// Whole-root discards plus the residual complement of a partially moved
/// owned parameter. Only owned transfers consume that custody. Establishing a
/// reference, projected or whole, borrows backing and leaves its source owner
/// on the checked cleanup roster; a returned descriptor carries its own loan.
pub(in crate::execution::terminal_unit) fn return_unit_affine_discards(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    source_parameters: &[StateParameter],
    operations: &[CheckedUnitEffectOperationPlan],
    admitted_local_symbols: &[SymbolHandle],
    structural_types: &BTreeMap<
        typed_trees::type_identity::TypeIdentityId,
        CheckedUnitStructuralTypePlan,
    >,
) -> Option<(Vec<u32>, Vec<CheckedUnitPartialAffineDiscardPlan>, bool)> {
    let mut transferred_parameters = BTreeSet::new();
    let mut moved_parameter_paths = BTreeMap::<
        u32,
        Vec<(
            Vec<CheckedUnitStructuralPathSegment>,
            typed_trees::type_identity::TypeIdentityId,
        )>,
    >::new();
    for (parameter_index, parameter) in structural_parameters.iter().enumerate() {
        if parameter.access != CheckedStructuralAccess::Owned {
            continue;
        }
        let source = source_parameters.get(parameter.position as usize)?;
        let parameter_index = u32::try_from(parameter_index).ok()?;
        let vacancies = super::super::storage_transfers::parameter_vacancies(
            program,
            facts,
            parameter_index,
            parameter.position,
            source.symbol,
            operations,
        )?;
        // A whole affine root moved on only some alternatives (one arm of a
        // value `match` selecting among owned parameters) needs no exit work
        // on the others when its type has no nominal cleanup: discarding it
        // there is a no-op, exactly as for a maybe-moved trivial local, so
        // the root leaves the return-edge roster. Partial or cleanup-bearing
        // maybe-moves still need selected cleanup this edge cannot express.
        if !vacancies.maybe.is_empty() {
            if vacancies.maybe.iter().any(|path| !path.is_empty())
                || parameter.multiplicity != Multiplicity::Affine
                || type_graph_requires_nominal_drop(
                    program,
                    source.type_reference,
                    &[],
                    &mut BTreeSet::new(),
                )
            {
                return None;
            }
            transferred_parameters.insert(parameter_index);
        }
        for path in vacancies.definite {
            if path.is_empty() {
                transferred_parameters.insert(parameter_index);
            } else {
                let endpoint = projected_type(structural_types, parameter.type_identity, &path)?;
                moved_parameter_paths
                    .entry(parameter_index)
                    .or_default()
                    .push((path, endpoint));
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
            parameter.type_identity,
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
            let root = event.root.as_symbol()?;
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
        if parameter.multiplicity != Multiplicity::Affine || output.contains(&parameter_index) {
            return None;
        }
        output.push(parameter_index);
    }
    Some((
        output,
        residual_affine_discards,
        has_projected_parameter_moves,
    ))
}

/// Check source access independently of structural shape collection. Generic
/// arguments and declaration fields are both inspected: substituting an owned
/// argument cannot turn an authored reference field into owned storage.
pub(super) fn partial_affine_source_contents_are_owned(
    program: &dyn ::validation::CheckingInput,
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
    let Some(data) = program.data_definition_by_symbol(symbol) else {
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
    program
        .data_member_fields(program.data_members(data))
        .all(|field| {
            !field.relevance.is_erased()
                && partial_affine_source_contents_are_owned(program, field.type_reference, visited)
        })
}

/// Resolve a moved endpoint in the same checked shape catalog used to
/// reconstruct its residual. Case identities select payload namespaces; a
/// bare Case is not a value. No reference or runtime-index inference occurs.
fn projected_type(
    types: &BTreeMap<typed_trees::type_identity::TypeIdentityId, CheckedUnitStructuralTypePlan>,
    mut current: typed_trees::type_identity::TypeIdentityId,
    mut path: &[CheckedUnitStructuralPathSegment],
) -> Option<typed_trees::type_identity::TypeIdentityId> {
    while let Some((segment, remaining)) = path.split_first() {
        let shape = &types.get(&current)?.shape;
        match (shape, segment) {
            (
                CheckedUnitStructuralTypeShape::Record { fields, .. }
                | CheckedUnitStructuralTypeShape::Mixed { fields, .. },
                CheckedUnitStructuralPathSegment::Field(identity),
            ) => {
                current = fields
                    .iter()
                    .find(|field| &field.identity == identity)?
                    .field_type
                    .structural_type_identity()?;
                path = remaining;
            }
            (
                CheckedUnitStructuralTypeShape::Sum { cases }
                | CheckedUnitStructuralTypeShape::Mixed { cases, .. },
                CheckedUnitStructuralPathSegment::Case(identity),
            ) => {
                let case = cases.iter().find(|case| &case.identity == identity)?;
                let (CheckedUnitStructuralPathSegment::Field(field), suffix) =
                    remaining.split_first()?
                else {
                    return None;
                };
                current = case
                    .fields
                    .iter()
                    .find(|candidate| &candidate.identity == field)?
                    .field_type
                    .structural_type_identity()?;
                path = suffix;
            }
            (
                CheckedUnitStructuralTypeShape::FixedArray {
                    element_type_identity,
                    length,
                },
                CheckedUnitStructuralPathSegment::FixedIndex(index),
            ) if length.admits_fixed_index(*index) => {
                current = *element_type_identity;
                path = remaining;
            }
            _ => return None,
        }
    }
    Some(current)
}
