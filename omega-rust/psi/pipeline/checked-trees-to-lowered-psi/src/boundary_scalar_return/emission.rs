//! Boundary-return body emission in an already selected catalog and identity space.

use super::*;

pub(crate) struct BoundaryScalarReturnCatalogs<'a> {
    pub(crate) structural_types: &'a [StructuralTypeDeclaration],
    pub(crate) type_ids: &'a [(String, StructuralTypeId)],
    pub(crate) service_ids: &'a [(ServiceReachId, ServiceId)],
}

pub(crate) struct BoundaryScalarReturnIdentities {
    pub(crate) machine: MachineId,
    pub(crate) contract: ContractId,
    pub(crate) boundary: BoundaryMachineId,
    pub(crate) identity_base: u64,
}

pub(crate) struct EmittedBoundaryScalarReturn {
    pub(crate) machine: TerminalMachine,
    pub(crate) source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
    pub(crate) selected_ieee_float_fma_occurrences: Vec<LoweredSelectedIeeeFloatFmaOccurrence>,
}

pub(crate) fn emit_boundary_scalar_return(
    checked: &CheckedTrees,
    plan: &CheckedBoundaryScalarReturnMachinePlan,
    parameters: Vec<StructuralParameterDeclaration>,
    catalogs: BoundaryScalarReturnCatalogs<'_>,
    identities: BoundaryScalarReturnIdentities,
    scalar_calls: &mut CallEmissionContext<'_>,
) -> Result<EmittedBoundaryScalarReturn, LoweringError> {
    let boundary = validate_boundary_scalar_return(checked, plan)?;
    let BoundaryScalarReturnCatalogs {
        structural_types,
        type_ids,
        service_ids,
    } = catalogs;
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        source_site,
        target_machine,
        scalar_arguments,
        structural_arguments,
        completion_receipts,
        ..
    } = &plan.boundary_call
    else {
        return unsupported("result-bearing boundary plan does not contain a boundary call");
    };
    let boundary_scalar_parameters = boundary
        .scalar_parameters
        .iter()
        .map(|parameter| terminal_scalar_type(parameter.primitive_type))
        .collect::<Result<Vec<_>, _>>()?;
    let expected_base = (identities.machine.get() - 1)
        .checked_mul(TERMINAL_MACHINE_IDENTITY_STRIDE)
        .ok_or(LoweringError::Unsupported(
            "boundary-return machine identity range overflows",
        ))?;
    if expected_base != identities.identity_base {
        return unsupported("boundary-return identity base disagrees with its exact machine");
    }
    let identity_limit = identities
        .identity_base
        .checked_add(TERMINAL_MACHINE_IDENTITY_STRIDE)
        .ok_or(LoweringError::Unsupported(
            "boundary-return identity range overflows",
        ))?;
    let first_identity =
        identities
            .identity_base
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "boundary-return identity range has no first identity",
            ))?;
    let mut next_claim = first_identity;
    let mut entry_claims = Vec::with_capacity(plan.entry_claims.len());
    let mut claim_bindings = Vec::with_capacity(plan.entry_claims.len());
    for claim in &plan.entry_claims {
        if claim.carry != CarryPolicy::STRICT {
            return unsupported("result-bearing boundary entry claim has non-default carry");
        }
        let parameter = parameters
            .get(usize::try_from(claim.parameter_index).map_err(|_| {
                LoweringError::Unsupported("boundary entry claim parameter exceeds usize")
            })?)
            .ok_or(LoweringError::Unsupported(
                "result-bearing boundary entry claim has an invalid parameter",
            ))?;
        let PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source: language_semantics::PermissionEventSource::StateEntry,
            ..
        } = claim.claim_identity
        else {
            return unsupported("result-bearing boundary entry claim is not exact");
        };
        if machine_symbol != plan.machine || state_symbol != plan.state {
            return unsupported("result-bearing boundary entry claim belongs to another state");
        }
        let id = claim_id(allocate_dense(&mut next_claim)?);
        entry_claims.push(EntryClaim {
            claim: id,
            input: parameter.place,
            path: lower_structural_path(&claim.path),
        });
        claim_bindings.push((claim.claim_identity, id));
    }
    let expected_claim_arguments = structural_arguments
        .iter()
        .enumerate()
        .flat_map(|(argument_index, argument)| {
            plan.entry_claims
                .iter()
                .filter(move |claim| {
                    Some(claim.parameter_index) == argument.source_parameter_index()
                        && (argument.path.is_empty() || claim.path == argument.path)
                })
                .map(move |_| {
                    u32::try_from(argument_index).map_err(|_| {
                        LoweringError::Unsupported("boundary argument index exceeds u32")
                    })
                })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    validate_transfer_shape(
        structural_arguments,
        completion_receipts,
        &parameters,
        &[],
        &[],
        &[],
        &boundary.structural_parameters,
        type_ids,
        structural_types,
        &expected_claim_arguments,
    )?;
    if scalar_arguments.len() != boundary_scalar_parameters.len() {
        return unsupported(
            "result-bearing boundary scalar argument count disagrees with its declaration",
        );
    }
    let mut operations = OperationBuffer::new(identities.identity_base);
    let mut next_value_identity = first_identity;
    let mut next_block = first_identity;
    let mut next_edge = first_identity;
    let mut evaluation = argument_evaluation::Evaluation::new(&mut next_block)?;
    let arguments = evaluation.arguments(
        checked,
        plan.machine,
        plan.state,
        &plan.boundary_call,
        &mut Vec::new(),
        &mut next_value_identity,
        &mut next_block,
        &mut next_edge,
        &mut operations,
        scalar_calls,
    )?;
    let arguments =
        argument_evaluation::validated_values(arguments.as_deref(), &boundary_scalar_parameters)?
            .into_iter()
            .map(|value| value.id)
            .collect();
    let scalar_type = terminal_scalar_type(plan.result_type)?;
    let call_result = ValueDeclaration {
        id: value_id(next_value_identity),
        scalar_type,
    };
    next_value_identity = next_value_identity
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "result-bearing boundary value identity space is exhausted",
        ))?;
    let operation_id = operations.allocate();
    operations.record_source_call(
        SourceCallCoordinate {
            state: plan.state,
            statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                LoweringError::Unsupported(
                    "result-bearing boundary statement coordinate exceeds usize",
                )
            })?,
            call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("result-bearing boundary call ordinal exceeds usize")
            })?,
        },
        *source_site,
        operation_id,
        *target_machine,
    )?;
    let operation = Operation {
        id: operation_id,
        result: terminal_psi::OperationResult::Scalar(call_result),
        kind: OperationKind::BoundaryCall {
            boundary: identities.boundary,
            arguments,
            structural_arguments: lower_structural_arguments(
                structural_arguments,
                &parameters,
                &[],
                &[],
                &[],
                &[],
            )?,
            completion_receipts: completion_receipts
                .iter()
                .map(|receipt| {
                    Ok(CompletionReceipt {
                        claim: lookup_claim_id(&claim_bindings, receipt.claim_identity)?,
                        argument_index: receipt.argument_index,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
        },
    };
    operations.push(operation);
    let machine_result = ValueDeclaration {
        id: value_id(next_value_identity),
        scalar_type,
    };
    let content_entry_claims = content_conservation::lower_whole_content_entry_claims(
        checked,
        &plan.structural_parameters,
        &parameters,
        &plan.entry_claims,
        &claim_bindings,
    )?;
    let OperationBuffer {
        operations,
        source_calls: source_call_occurrences,
        selected_ieee_float_fmas: selected_ieee_float_fma_occurrences,
        ..
    } = operations;
    evaluation.blocks.push(Block {
        id: evaluation.current,
        parameters: evaluation.parameters,
        operations: operations[evaluation.operation_start..].to_vec(),
        terminator: Terminator::Return {
            edge: edge_id(allocate_dense(&mut next_edge)?),
            value: call_result.id,
            cleanup_actions: Vec::new(),
        },
    });
    evaluation.blocks.sort_by_key(|block| block.id);
    let machine = TerminalMachine {
        id: identities.machine,
        attachment: Some(lookup_type_id(type_ids, &plan.attachment_type_identity)?),
        parameters: Vec::new(),
        structural_parameters: parameters.clone(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(machine_result),
        structural_places: parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                },
            })
            .collect(),
        entry_claims,
        published_service_ceiling: lower_installation_machine_service_ceiling(
            checked,
            plan.machine,
            plan.contract_service_reach,
            plan.service_reach,
            service_ids,
        )?,
        content_entry_claims,
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: evaluation.entry,
        blocks: evaluation.blocks,
        contract: MachineContract {
            id: identities.contract,
            crash_routes: lower_checked_crash_route_buckets(
                &lower_checked_crash_routes(checked, plan.machine)?,
                &[],
            )?,
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    if [next_claim, next_value_identity, next_block, next_edge]
        .into_iter()
        .any(|next| next >= identity_limit)
        || machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| operation.id.get() >= identity_limit)
    {
        return unsupported("boundary-return body exhausts its reserved identity range");
    }
    Ok(EmittedBoundaryScalarReturn {
        machine,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences,
    })
}
