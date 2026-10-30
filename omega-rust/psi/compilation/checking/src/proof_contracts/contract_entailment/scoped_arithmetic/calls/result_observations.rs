//! Public result coordinates are observations, not inferred bounds. The
//! selected return declaration supplies geometry; the existing call-completion
//! owner independently transports only its checked public guarantees.
use super::super::ScopedCallResultObservation;
use super::*;

pub(in crate::proof_contracts::contract_entailment::scoped_arithmetic) fn result_observations(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Vec<ScopedCallResultObservation> {
    let Some(call) = program.expression_table.call_node(expression) else {
        return Vec::new();
    };
    let Some(owner) = selected_call_site_contract(
        program,
        machine.symbol,
        crate::CallSite::Expression { expression, call },
    ) else {
        return Vec::new();
    };
    if matches!(owner, SelectedCallContract::DynamicSlot { .. }) {
        return Vec::new();
    }
    // A concrete completion's arithmetic telescope is its entry state, while
    // reserved result projections belong to the machine declaration. Keep
    // those identities separate rather than comparing their handle positions.
    let result_owner = match owner {
        SelectedCallContract::Concrete(machine, _) => machine.symbol,
        _ => owner_symbol(&owner),
    };
    let reference =
        crate::proof_contracts::contract_entailment::specification_calls::owner_return_type(&owner);
    // Only inline owned result bits are independent snapshots. A returned
    // reference needs its own exact binding/load dependencies before its live
    // referent can enter this roster; return type alone supplies none.
    if crate::reference_shell(program, reference).is_some()
        || program.primitive_type_reference(reference).is_some()
    {
        return Vec::new();
    }
    // These are checked public guarantees of the completed value, not a
    // pending source formation goal or facts inferred from its private body.
    // A case-dependent coordinate is available only when that exact result's
    // selected case follows from this roster on every normal completion.
    let guarantees = owner_contracts(program, &owner)
        .filter(|contract| {
            contract.kind == SignatureContractKind::Ensures && contract.binding.is_none()
        })
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| fact.as_expression().copied())
        .collect::<Vec<_>>();
    let mut pending = owner_contracts(program, &owner)
        .filter(|contract| {
            contract.kind == SignatureContractKind::Ensures && contract.binding.is_none()
        })
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| fact.as_expression().copied())
        .collect::<Vec<_>>();
    let mut observations: Vec<ScopedCallResultObservation> = Vec::new();
    let mut visited = Vec::new();
    while let Some(selected) = pending.pop() {
        if visited.contains(&selected) {
            continue;
        }
        visited.push(selected);
        if let Some(place) = crate::reserved_result_place(program, selected)
            && place.machine_symbol == result_owner
            && !place.segments.is_empty()
            && let Some(geometry) = crate::structural_scalar_coordinate_in_type(
                program,
                facts::PlaceRoot::Expression(expression),
                reference,
                &place.segments,
            )
            && geometry.primitive_type.is_fixed_integer()
            && geometry.projections.iter().all(|step| {
                matches!(
                    step,
                    crate::ScalarCoordinateProjection::Storage(
                        checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                            | checked_trees::CheckedUnitStructuralPathSegment::Case(_)
                            | checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(_)
                    )
                )
            })
            && super::super::super::declaration_projection_cases_hold(
                program,
                result_owner,
                owner_parameters(program, &owner),
                &guarantees,
                &crate::CanonicalPlace {
                    root: facts::PlaceRoot::Expression(place.root),
                    segments: place.segments.clone(),
                },
            )
        {
            let value = observations
                .iter()
                .find(|existing| existing.path == place.segments)
                .map(|existing| existing.value.clone())
                .unwrap_or_else(|| ScopedArithmeticValue::Atom {
                    identity: format!(
                        "state:{:?}:call-result:{expression:?}:field:{:?}",
                        state.symbol, place.segments
                    ),
                    unsigned: matches!(
                        geometry.primitive_type,
                        typed_trees::unsigned_integer_primitive_pattern!()
                    ),
                });
            observations.push(ScopedCallResultObservation {
                expression: selected,
                path: place.segments,
                primitive_type: geometry.primitive_type,
                value,
            });
        }
        children(
            program,
            program.expression_table.expression(selected),
            |child| pending.push(child),
        );
    }
    observations
}

pub(super) fn result_uses_are_bound(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    bindings: &[ScopedArithmeticBinding],
) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if bindings
            .iter()
            .any(|binding| binding.binder == ScopedArithmeticBinder::Occurrence(expression))
        {
            continue;
        }
        if program.expression_table.names_reserved_result(expression) {
            return false;
        }
        children(
            program,
            program.expression_table.expression(expression),
            |child| pending.push(child),
        );
    }
    true
}
