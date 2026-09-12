//! Shared ordinary call operands, requirements and crash substitution.
use super::*;
use crate::runtime_requirements::substitute_runtime_requirement_scalar_values;

pub(super) struct Target<'a> {
    pub parameters: &'a [StructuralParameterDeclaration],
    pub scalar_parameters: &'a [ValueDeclaration],
    pub predicate_parameters: &'a [StructuralParameterDeclaration],
    pub runtime_requirements: &'a [Proposition],
}

pub(super) struct PreparedCall {
    pub arguments: Vec<ValueId>,
    pub structural_arguments: Vec<StructuralArgument>,
    pub requirement_obligations: Vec<ObligationId>,
    pub crash_continuations: Vec<terminal_psi::CrashRouteBucket>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare(
    checked: &CheckedTrees,
    plans: &checked_trees::CheckedUnitEffectPlans,
    operation: &CheckedUnitEffectOperationPlan,
    target: Target<'_>,
    evaluated_scalar_arguments: Option<&[ValueDeclaration]>,
    parameters: &[StructuralParameterDeclaration],
    local_places: &[StructuralPlaceDeclaration],
    structural_result_places: &[(StructuralPlaceDeclaration, bool)],
    primitive_local_places: &[primitive_locals::PrimitiveLocal],
    type_ids: &[(String, StructuralTypeId)],
    structural_types: &[StructuralTypeDeclaration],
    call_byte_places: &[PlaceId],
    calls: &mut CallEmissionContext<'_>,
) -> Result<PreparedCall, LoweringError> {
    let (target_machine, scalar_arguments, structural_arguments, claim_transfers) = match operation
    {
        CheckedUnitEffectOperationPlan::CallUnit {
            target_machine,
            scalar_arguments,
            structural_arguments,
            claim_transfers,
            ..
        } => (
            target_machine,
            scalar_arguments,
            structural_arguments,
            claim_transfers.as_slice(),
        ),
        CheckedUnitEffectOperationPlan::StructuralCall {
            target_machine,
            scalar_arguments,
            structural_arguments,
            ..
        } => (
            target_machine,
            scalar_arguments,
            structural_arguments,
            &[][..],
        ),
        _ => return unsupported("ordinary call preparation has no call operation"),
    };
    let checked_target = UnitBody::find(plans, *target_machine)?.entry()?;
    if scalar_arguments.len() != checked_target.scalar_parameters.len() {
        return unsupported("Unit call scalar argument count disagrees with its target");
    }
    let terminal_scalar_values = argument_evaluation::validated_values(
        evaluated_scalar_arguments,
        &checked_target
            .scalar_parameters
            .iter()
            .map(|parameter| terminal_scalar_type(parameter.primitive_type))
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    let terminal_scalar_arguments = terminal_scalar_values
        .iter()
        .map(|value| value.id)
        .collect();
    validate_transfer_shape(
        structural_arguments,
        claim_transfers,
        parameters,
        local_places,
        structural_result_places,
        checked_target.structural_parameters,
        type_ids,
        structural_types,
        &checked_target
            .entry_claims
            .iter()
            .map(|claim| claim.parameter_index)
            .collect::<Vec<_>>(),
        primitive_local_places,
    )?;
    let terminal_arguments = lower_structural_arguments(
        structural_arguments,
        parameters,
        local_places,
        structural_result_places,
        call_byte_places,
        primitive_local_places,
    )?;

    if target.scalar_parameters.len() != terminal_scalar_values.len() {
        return unsupported("Unit call scalar requirement arity is inconsistent");
    }
    let scalar_substitutions = target
        .scalar_parameters
        .iter()
        .zip(&terminal_scalar_values)
        .map(|(formal, actual)| {
            if formal.scalar_type != actual.scalar_type {
                return unsupported("Unit call scalar requirement parameter type is inconsistent");
            }
            Ok((formal.id, *actual))
        })
        .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
    // Preserve callee requirement slots after substitution, even
    // if reordered or equal arguments change canonical term order.
    let target_runtime_requirements = target
        .runtime_requirements
        .iter()
        .map(|requirement| {
            let mut requirement = requirement.clone();
            substitute_runtime_requirement_scalar_values(&mut requirement, &scalar_substitutions)?;
            Ok(requirement)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut crash_continuations =
        if let Some(target_contract) = checked.facts.contract_plans.for_machine(*target_machine) {
            if target.parameters.is_empty() {
                lower_checked_crash_route_buckets(
                    target_contract.crash.published(),
                    &terminal_scalar_values,
                )?
            } else {
                lower_structural_crash_route_buckets(
                    target_contract.crash.published(),
                    &terminal_scalar_values,
                    target.predicate_parameters,
                    structural_types,
                    &target_runtime_requirements,
                )?
            }
        } else {
            Vec::new()
        };
    let substitutions = target
        .parameters
        .iter()
        .zip(&terminal_arguments)
        .map(|(parameter, argument)| {
            Ok((
                parameter.place,
                (
                    argument.place,
                    if call_byte_places.contains(&argument.place) {
                        // Transfer validation already required the exact whole
                        // immutable byte view. Its canonical path has no segments.
                        Vec::new()
                    } else {
                        structural_crash_route_argument_prefix(
                            argument,
                            parameters,
                            local_places,
                            structural_result_places,
                            structural_types,
                            primitive_local_places,
                        )?
                    },
                ),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, LoweringError>>()?;
    substitute_structural_crash_route_roots(&mut crash_continuations, &substitutions)?;
    let requirement_obligations = target_runtime_requirements
        .iter()
        .map(|_| {
            // Proof finalization reconstructs this exact callee
            // slot against the completed caller's pre-call facts.
            let obligation = calls.allocate_requirement()?;
            Ok(obligation)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok(PreparedCall {
        arguments: terminal_scalar_arguments,
        structural_arguments: terminal_arguments,
        requirement_obligations,
        crash_continuations,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_structural(
    state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    prepared: PreparedCall,
    callee: MachineId,
    type_ids: &[(String, StructuralTypeId)],
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let CheckedUnitEffectOperationPlan::StructuralCall {
        coordinate,
        source_site,
        result,
        target_state,
        ..
    } = operation
    else {
        return unsupported("structural call emission requires an owned result call");
    };
    let id = operations.allocate();
    let place = place_id(allocate_dense(next_place)?);
    let structural_type = lookup_type_id(type_ids, &result.type_identity)?;
    operations.record_source_call(
        SourceCallCoordinate {
            state,
            statement_index: coordinate.statement_index as usize,
            call_ordinal: coordinate.call_ordinal as usize,
        },
        *source_site,
        id,
        *target_state,
    )?;
    let returned = StructuralOperationResult {
        place,
        structural_type,
        multiplicity: match result.multiplicity {
            Multiplicity::Affine => StructuralMultiplicity::Affine,
            Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
            Multiplicity::Linear => {
                return unsupported("ordinary structural result has unsupported linear custody");
            }
        },
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    operations.push(Operation {
        static_reach_binding: None,
        id,
        result: OperationResult::Structural(returned.clone()),
        kind: OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments: prepared.arguments,
            structural_arguments: prepared.structural_arguments,
            claim_transfers: Vec::new(),
            returned_claim_transfers: Vec::new(),
            requirement_obligations: prepared.requirement_obligations,
            crash_continuations: prepared.crash_continuations,
        },
    });
    if operations
        .structural_values
        .iter()
        .any(|(ordinal, _)| *ordinal == result.binding_ordinal)
    {
        return unsupported("structural call result was published twice");
    }
    operations
        .structural_values
        .push((result.binding_ordinal, returned));
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id,
            structural_type,
        },
    })
}
