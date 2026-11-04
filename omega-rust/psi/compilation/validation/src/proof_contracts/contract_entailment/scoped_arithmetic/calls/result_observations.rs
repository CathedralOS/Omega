//! Public result coordinates are observations, not inferred bounds. The
//! selected return declaration supplies geometry; the existing call-completion
//! owner independently transports only its checked public guarantees.
use super::super::ScopedCallResultObservation;
use super::*;

struct ResultObservationPlan<'program> {
    owner: SelectedCallContract<'program>,
    guarantees: Vec<ExpressionHandle>,
    coordinates: Vec<super::super::PendingScopedCallResultObservation>,
}

pub(in crate::proof_contracts::contract_entailment::scoped_arithmetic) fn result_observations(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Vec<ScopedCallResultObservation> {
    let Some(ResultObservationPlan {
        owner,
        guarantees,
        coordinates,
    }) = discover_result_observations(program, machine, state, expression)
    else {
        return Vec::new();
    };
    let result_owner = result_declaration_owner(&owner);
    let frames = std::cell::OnceCell::new();
    coordinates
        .into_iter()
        .filter_map(|pending| {
            // This public view remains unconditional. A normal-call completion
            // can separately establish these same pending requirements at its cut.
            if !super::super::super::declaration_projection_cases_hold(
                program,
                result_owner,
                owner_parameters(program, &owner),
                &guarantees,
                &crate::CanonicalPlace {
                    root: facts::PlaceRoot::Expression(pending.declaration_root),
                    segments: pending.observation.path.clone(),
                },
            ) {
                return None;
            }
            let frames = if pending.requires_backing {
                frames
                    .get_or_init(|| crate::CallFrameResolver::new(program))
                    .as_ref()
            } else {
                None
            };
            resolve_backing(program, machine, state, expression, pending, frames)
        })
        .collect()
}

pub(in crate::proof_contracts::contract_entailment::scoped_arithmetic) fn pending_result_observations(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Vec<super::super::PendingScopedCallResultObservation> {
    discover_result_observations(program, machine, state, expression)
        .map(|plan| plan.coordinates)
        .unwrap_or_default()
}

fn result_declaration_owner(owner: &SelectedCallContract<'_>) -> SymbolHandle {
    match owner {
        SelectedCallContract::Concrete(machine, _) => machine.symbol,
        _ => owner_symbol(owner),
    }
}

fn discover_result_observations<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<ResultObservationPlan<'program>> {
    let call = program.expression_table.call_node(expression)?;
    let owner = selected_call_site_contract(
        program,
        machine.symbol,
        crate::CallSite::Expression { expression, call },
    )?;
    if matches!(owner, SelectedCallContract::DynamicSlot { .. }) {
        return None;
    }
    let result_owner = result_declaration_owner(&owner);
    let reference =
        crate::proof_contracts::contract_entailment::specification_calls::owner_return_type(&owner);
    if crate::reference_shell(program, reference).is_some()
        || program.primitive_type_reference(reference).is_some()
    {
        return None;
    }
    // Discover only coordinates authored in the checked public promises.
    // Their case and extent requirements remain pending until a consumer
    // establishes them; discovery supplies no value or storage observation.
    let guarantees = owner_contracts(program, &owner)
        .filter(|contract| {
            contract.kind == SignatureContractKind::Ensures && contract.binding.is_none()
        })
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .filter_map(|fact| fact.as_expression().copied())
        .collect::<Vec<_>>();
    let mut pending = guarantees.clone();
    let mut coordinates: Vec<super::super::PendingScopedCallResultObservation> = Vec::new();
    let mut visited = Vec::new();
    while let Some(selected) = pending.pop() {
        if visited.contains(&selected) {
            continue;
        }
        visited.push(selected);
        let place = crate::reserved_result_place(program, selected);
        if let Some(place) = place
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
                    crate::ScalarCoordinateProjection::Element { .. }
                        | crate::ScalarCoordinateProjection::Storage(
                            checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                                | checked_trees::CheckedUnitStructuralPathSegment::Case(_)
                                | checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(_)
                                | checked_trees::CheckedUnitStructuralPathSegment::Referent
                        )
                )
            })
            && let Some((cases, indices)) =
                super::super::scalar_projection_requirements(program, reference, &place.segments)
        {
            let requires_backing = geometry.projections.iter().any(|step| {
                matches!(
                    step,
                    crate::ScalarCoordinateProjection::Element { extent: None, .. }
                        | crate::ScalarCoordinateProjection::Storage(
                            checked_trees::CheckedUnitStructuralPathSegment::Referent
                        )
                )
            });
            let value = coordinates
                .iter()
                .find(|existing| existing.observation.path == place.segments)
                .map(|existing| existing.observation.value.clone())
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
            coordinates.push(super::super::PendingScopedCallResultObservation {
                observation: ScopedCallResultObservation {
                    expression: selected,
                    path: place.segments,
                    primitive_type: geometry.primitive_type,
                    value,
                    read_dependencies: Vec::new(),
                    backing: None,
                },
                declaration_root: place.root,
                cases,
                indices,
                requires_backing,
            });
        }
        children(
            program,
            program.expression_table.expression(selected),
            |child| pending.push(child),
        );
    }
    Some(ResultObservationPlan {
        owner,
        guarantees,
        coordinates,
    })
}

pub(in crate::proof_contracts::contract_entailment) fn resolve_backing(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    pending: super::super::PendingScopedCallResultObservation,
    frames: Option<&crate::CallFrameResolver<'_>>,
) -> Option<ScopedCallResultObservation> {
    let mut observation = pending.observation;
    if pending.requires_backing {
        let reference =
            crate::expression_result_type_reference(program, machine, state, expression)?;
        let backing = frames?.completed_value_read_backing(
            machine,
            state,
            expression,
            reference,
            &observation.path,
        )?;
        for path in backing
            .values
            .iter()
            .chain(backing.bindings.iter().map(|binding| &binding.path))
        {
            if !observation.read_dependencies.contains(path) {
                observation.read_dependencies.push(path.clone());
            }
        }
        observation.backing = Some(backing);
    }
    Some(observation)
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
