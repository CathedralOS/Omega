//! Shared ordinary call operands, requirements and crash substitution.
use super::super::{
    BTreeMap, ClaimId, MachineId, ObligationId, PermissionClaimIdentity, Proposition,
    StructuralArgument, StructuralParameterDeclaration, StructuralTypeDeclaration, ValueId,
    claim_id, structural_crash_route_argument_prefix, substitute_structural_crash_route_roots,
};
use super::{
    CheckedTrees, CheckedUnitEffectOperationPlan, ClaimTransfer, LoweringError, Multiplicity,
    Operation, OperationKind, OperationResult, PlaceId, SemanticDomainId, StructuralDomainId,
    StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, UnitBody, ValueDeclaration, allocate_dense,
    argument_evaluation, lookup_claim_id, lookup_domain_id, lookup_type_id,
    lower_checked_crash_route_buckets, lower_structural_arguments,
    lower_structural_crash_route_buckets, lower_structural_path, place_id, primitive_locals,
    terminal_scalar_type, unsupported, validate_transfer_shape,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, SourceCallCoordinate};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::unit::runtime_requirements::substitute_runtime_requirement_scalar_values;

pub(super) struct Target<'a> {
    pub parameters: &'a [StructuralParameterDeclaration],
    pub scalar_parameters: &'a [ValueDeclaration],
    pub erased_scalar_parameters: &'a [ValueDeclaration],
    pub predicate_parameters: &'a [StructuralParameterDeclaration],
    /// The exact `requires` rows the callee's emitted contract carries, in
    /// contract order: closed authored clauses merged ahead of runtime
    /// requirements.
    pub requires: &'a [Proposition],
    pub runtime_requirements: &'a [Proposition],
}

pub(super) struct PreparedCall {
    pub arguments: Vec<ValueId>,
    /// Proof-only erased actuals in the callee's erased-formal order.
    pub erased_arguments: Vec<semantic_vocabulary::ScalarTerm>,
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
    caller_scalar_values: &[ValueDeclaration],
    caller_erased_scalar_parameters: &[ValueDeclaration],
    parameters: &[StructuralParameterDeclaration],
    local_places: &[StructuralPlaceDeclaration],
    structural_result_places: &[(StructuralPlaceDeclaration, bool)],
    primitive_local_places: &[primitive_locals::PrimitiveLocal],
    type_ids: &[(String, StructuralTypeId)],
    structural_types: &[StructuralTypeDeclaration],
    call_byte_places: &[PlaceId],
    calls: &mut CallEmissionContext<'_>,
) -> Result<PreparedCall, LoweringError> {
    let (
        target_machine,
        scalar_arguments,
        erased_scalar_arguments,
        structural_arguments,
        claim_transfers,
    ) = match operation {
        CheckedUnitEffectOperationPlan::CallUnit {
            target_machine,
            scalar_arguments,
            erased_scalar_arguments,
            structural_arguments,
            claim_transfers,
            ..
        } => (
            target_machine,
            scalar_arguments,
            erased_scalar_arguments.as_slice(),
            structural_arguments,
            claim_transfers.as_slice(),
        ),
        CheckedUnitEffectOperationPlan::StructuralCall {
            target_machine,
            scalar_arguments,
            erased_scalar_arguments,
            structural_arguments,
            custody,
            ..
        } => (
            target_machine,
            scalar_arguments,
            erased_scalar_arguments.as_slice(),
            structural_arguments,
            custody.claim_transfers.as_slice(),
        ),
        _ => return unsupported("ordinary call preparation has no call operation"),
    };
    let checked_target = UnitBody::find(plans, *target_machine)?.entry()?;
    if target.erased_scalar_parameters.len() != checked_target.erased_scalar_parameters.len()
        || erased_scalar_arguments.len() != checked_target.erased_scalar_parameters.len()
    {
        return unsupported("Unit call erased formal roster drifted from its checked target");
    }
    let erased_arguments = erased_scalar_arguments
        .iter()
        .map(|argument| {
            let checked_trees::CheckedCallScalarArgument::Pure(expression) = argument else {
                return unsupported("erased Unit call actual must be a pure checked expression");
            };
            crate::proofs::crash_routes::checked_scalar_term(
                expression,
                caller_scalar_values,
                caller_erased_scalar_parameters,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
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
        None,
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
    let mut crash_continuations = {
        let target_routes = crate::unit::effective_crash_routes(checked, *target_machine)?;
        if target.parameters.is_empty() {
            lower_checked_crash_route_buckets(&target_routes, &terminal_scalar_values)?
        } else {
            lower_structural_crash_route_buckets(
                &target_routes,
                &terminal_scalar_values,
                target.predicate_parameters,
                structural_types,
                &target_runtime_requirements,
            )?
        }
    };
    if !crash_continuations.is_empty() {
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
    }
    // One obligation per published callee `requires` row: the verifier
    // re-derives the proposition from the emitted contract, so the count must
    // match the closed clauses plus runtime requirements exactly.
    let requirement_obligations = target
        .requires
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
        erased_arguments,
        structural_arguments: terminal_arguments,
        requirement_obligations,
        crash_continuations,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_structural(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    operation: &CheckedUnitEffectOperationPlan,
    prepared: PreparedCall,
    callee: MachineId,
    type_ids: &[(String, StructuralTypeId)],
    domain_ids: &[(SemanticDomainId, StructuralDomainId)],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    // Graph evaluation attaches the source local and registers this value as
    // one operation; ordinary and nested operands register it here instead.
    register_binding: bool,
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let CheckedUnitEffectOperationPlan::StructuralCall {
        coordinate,
        source_site,
        result,
        target_state,
        target_machine,
        custody,
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
            Multiplicity::Linear => StructuralMultiplicity::Linear,
        },
        qualifications: custody
            .result_qualifications
            .iter()
            .map(|domain| lookup_domain_id(domain_ids, *domain))
            .collect::<Result<Vec<_>, _>>()?,
        projected_qualifications: super::parameters::lower_projected_qualifications(
            validation::structural_result_projected_qualifications(
                &checked.typed,
                checked
                    .typed
                    .machine_states(
                        checked
                            .machines()
                            .iter()
                            .find(|machine| machine.symbol == *target_machine)
                            .ok_or(LoweringError::Unsupported(
                                "structural result has no exact target machine",
                            ))?,
                    )
                    .iter()
                    .find(|candidate| candidate.symbol == *target_state)
                    .ok_or(LoweringError::Unsupported(
                        "structural call target state is absent",
                    ))?
                    .return_type,
            )
            .map_err(LoweringError::Unsupported)?
            .as_slice(),
            domain_ids,
        )?,
        claims: custody
            .returned_claim_transfers
            .iter()
            .map(|transfer| {
                Ok(terminal_psi::StructuralResultClaimBinding {
                    claim: lookup_claim_id(claim_bindings, transfer.caller_claim)?,
                    path: lower_structural_path(&transfer.path),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?,
    };
    let target =
        UnitBody::find(&checked.facts.flow.terminal_unit_effects, *target_machine)?.entry()?;
    // Callee claims use the same dense entry ordering as lower_unit_entry_claims;
    // caller identities stay in the caller's namespace across normal completion.
    let returned_claim_transfers = custody
        .returned_claim_transfers
        .iter()
        .map(|transfer| {
            let position = target
                .entry_claims
                .iter()
                .position(|claim| claim.claim_identity == transfer.callee_claim)
                .ok_or(LoweringError::Unsupported(
                    "structural returned claim is not a callee entry claim",
                ))?;
            Ok(terminal_psi::StructuralResultClaimTransfer {
                callee_claim: claim_id(
                    u64::try_from(position)
                        .map_err(|_| LoweringError::Unsupported("callee claim ordinal overflow"))?
                        + 1,
                ),
                caller_claim: lookup_claim_id(claim_bindings, transfer.caller_claim)?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let claim_transfers = custody
        .claim_transfers
        .iter()
        .map(|transfer| {
            Ok(ClaimTransfer {
                claim: lookup_claim_id(claim_bindings, transfer.claim_identity)?,
                argument_index: transfer.argument_index,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    // Scalar values and structural custody are separate operand namespaces,
    // not different ownership rules. Keep the scalar-free encoding unchanged;
    // mixed calls retain the same checked claim and content correspondence.
    // `CallStructural` owns no erased lane; a proof-only actual selects the
    // scalar-argument shape even when every runtime operand is empty.
    let kind = if result.multiplicity == Multiplicity::Linear
        && prepared.arguments.is_empty()
        && prepared.erased_arguments.is_empty()
    {
        OperationKind::CallStructural {
            callee,
            structural_arguments: prepared.structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations: prepared.requirement_obligations,
            crash_continuations: prepared.crash_continuations,
            selected_evidence: Vec::new(),
        }
    } else {
        OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments: prepared.arguments,
            erased_arguments: prepared.erased_arguments,
            structural_arguments: prepared.structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations: prepared.requirement_obligations,
            crash_continuations: prepared.crash_continuations,
        }
    };
    operations.push(Operation {
        static_reach_binding: None,
        id,
        result: OperationResult::Structural(returned.clone()),
        kind,
    });
    if register_binding
        && operations
            .structural_values
            .iter()
            .any(|(ordinal, _)| *ordinal == result.binding_ordinal)
    {
        return unsupported("structural call result was published twice");
    }
    if register_binding {
        operations
            .structural_values
            .push((result.binding_ordinal, returned));
    }
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id,
            structural_type,
        },
    })
}
