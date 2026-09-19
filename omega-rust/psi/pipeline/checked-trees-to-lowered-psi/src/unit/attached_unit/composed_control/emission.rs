//! Shared call emission and final module assembly for composed control.
use super::super::super::{
    BlockId, ClaimId, LoweredSourceCallOccurrence, PermissionClaimIdentity,
    StructuralParameterDeclaration, StructuralTypeDeclaration,
};
use super::super::{
    Block, BoundaryMachineResult, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
    CompletionReceipt, LoweredPsi, Operation, OperationKind, OperationResult, PlaceId, ProofBundle,
    StructuralOperationResult, StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeId,
    TerminalMachine, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    allocate_dense, edge_id, finalize_operation_proofs, lookup_claim_id, lookup_machine_id,
    lookup_type_id, lower_checked_crash_route_buckets, lower_structural_arguments,
    lower_structural_path, machine_id, place_id, terminal_scalar_type, unsupported,
    validate_transfer_shape,
};
use super::{
    CheckedTrees, LoweringError, SourceMappedLowered, catalogs, internal_calls, literal_arguments,
    state_graph,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, SourceCallCoordinate};
use crate::emission::operation_emission::calls::CallEmissionContext;

pub(crate) fn emit_call_leaf(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    block: BlockId,
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    scalar_parameters: &[ValueDeclaration],
    erased_parameters: &[ValueDeclaration],
    next_value: &mut u64,
    next_block: &mut u64,
    next_operation: &mut u64,
    next_edge: &mut u64,
) -> Result<(Vec<Block>, Vec<LoweredSourceCallOccurrence>), LoweringError> {
    if !state.erased_scalar_parameters.is_empty() {
        return unsupported(
            "composed call leaves with erased formals require erased edge operands",
        );
    }
    // A retained `requires` row belongs on a state-graph header invariant;
    // call leaves have no header lane to publish it through.
    if state.requires.iter().any(Option::is_some) {
        return unsupported("composed call leaf cannot publish its retained requires");
    }
    let mut operations = OperationBuffer::new(*next_operation - 1);
    let mut evaluation = super::super::argument_evaluation::Evaluation {
        structural_value_owners: Vec::new(),
        selection_cleanups: Vec::new(),
        structural_locals: Vec::new(),
        local_cases: Vec::new(),
        record_fields: crate::scalar_graph::scalar_computations::fields::prepare(
            checked,
            machine,
            &catalogs.structural_types,
        )?,
        arrays: crate::scalar_graph::scalar_computations::arrays::prepare(
            checked,
            machine,
            &catalogs.structural_types,
            &mut catalogs.next_place,
        )?,
        cases: crate::scalar_graph::scalar_computations::cases::prepare(
            checked,
            machine,
            &catalogs.structural_types,
            &mut catalogs.next_place,
        )?,
        primitive_storage: Vec::new(),
        scalar_bindings: None,
        structural_fields: Vec::new(),
        structural_cases: Vec::new(),
        structural_parameters: Vec::new(),
        erased_scalar_formals: erased_parameters.to_vec(),
        entry: block,
        current: block,
        parameters: scalar_parameters.to_vec(),
        block_structural_parameters: Vec::new(),
        operation_start: 0,
        blocks: Vec::new(),
    };
    let mut values = scalar_parameters.to_vec();
    let result_start = catalogs.result_places.len();
    emit_call_operations(
        checked,
        machine,
        state,
        &state.operations,
        catalogs,
        parameters,
        claim_bindings,
        &mut evaluation,
        &mut values,
        erased_parameters,
        next_value,
        next_block,
        next_edge,
        &mut operations,
    )?;
    *next_operation = operations.next_identity;
    let discards = evaluation.selection_return_discards(
        catalogs.result_places[result_start..]
            .iter()
            .rev()
            .map(|declaration| {
                let discard = operations
                    .structural_values
                    .iter()
                    .find(|(_, result)| result.place == declaration.id)
                    .and_then(|(ordinal, _)| {
                        state
                            .operations
                            .iter()
                            .find_map(|operation| match operation {
                                CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                                    result,
                                    discard_result_on_return,
                                    ..
                                } if result.binding_ordinal == *ordinal => {
                                    Some(*discard_result_on_return)
                                }
                                _ => None,
                            })
                    })
                    .unwrap_or(true);
                (declaration.id, discard)
            })
            .collect(),
    )?;
    evaluation.remap_transported_call_operands(&mut operations);
    evaluation.blocks.push(Block {
        structural_parameters: evaluation.block_structural_parameters,
        id: evaluation.current,
        parameters: evaluation.parameters,
        erased_scalar_formals: Vec::new(),
        operations: operations[evaluation.operation_start..].to_vec(),
        terminator: Terminator::ReturnUnit {
            edge: edge_id(allocate_dense(next_edge)?),
            trivial_affine_discards: discards,
        },
    });
    Ok((evaluation.blocks, operations.source_calls))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_call_operations(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    planned_operations: &[CheckedUnitEffectOperationPlan],
    catalogs: &mut catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    evaluation: &mut super::super::argument_evaluation::Evaluation,
    values: &mut Vec<ValueDeclaration>,
    // The emitting machine's erased roster, for `ErasedParameter` actuals
    // forwarded through proof-only call operands.
    erased_parameters: &[ValueDeclaration],
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    for operation in planned_operations {
        if matches!(
            operation,
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
        ) {
            let mut calls = catalogs.scalar_calls.emission_context();
            // The operand closure holds an immutable view of the caller's
            // scalar namespace while the emitter mutates `values` around it.
            let operand_values = values.clone();
            let mut emit_operand_call =
                |operand: &CheckedUnitEffectOperationPlan,
                 evaluated: Option<&[ValueDeclaration]>,
                 call_context: &mut CallEmissionContext<'_>,
                 output: &mut OperationBuffer,
                 place_counter: &mut u64| {
                    let CheckedUnitEffectOperationPlan::StructuralCall { target_machine, .. } =
                        operand
                    else {
                        return unsupported("record operand is not a structural call");
                    };
                    let callee = lookup_machine_id(call_context.machine_ids, *target_machine)?;
                    let shared =
                        catalogs
                            .shared_units
                            .as_ref()
                            .ok_or(LoweringError::Unsupported(
                                "nested structural target has no shared closure",
                            ))?;
                    let target = shared
                        .semantic_module
                        .machines
                        .iter()
                        .find(|target| target.id == callee)
                        .ok_or(LoweringError::Unsupported(
                            "nested structural target is missing",
                        ))?;
                    let entry = super::super::bodies::UnitBody::find(
                        &checked.facts.flow.terminal_unit_effects,
                        *target_machine,
                    )?
                    .entry()?;
                    if entry.structural_parameters.len() != target.structural_parameters.len() {
                        return unsupported("nested target predicate roster differs");
                    }
                    let predicates = entry
                        .structural_parameters
                        .iter()
                        .zip(&target.structural_parameters)
                        .map(|(source, parameter)| StructuralParameterDeclaration {
                            position: source.position,
                            ..parameter.clone()
                        })
                        .collect::<Vec<_>>();
                    let earlier = catalogs
                        .result_places
                        .iter()
                        .cloned()
                        .map(|place| (place, false))
                        .collect::<Vec<_>>();
                    let prepared = super::super::ordinary_calls::prepare(
                        checked,
                        &checked.facts.flow.terminal_unit_effects,
                        operand,
                        super::super::ordinary_calls::Target {
                            parameters: &target.structural_parameters,
                            scalar_parameters: &target.parameters,
                            erased_scalar_parameters: &target.contract.erased_scalar_formals,
                            predicate_parameters: &predicates,
                            requires: &target.contract.requires,
                            runtime_requirements: &target.contract.requires,
                        },
                        evaluated,
                        &operand_values,
                        erased_parameters,
                        parameters,
                        &[],
                        &earlier,
                        &[],
                        &catalogs.type_ids,
                        &catalogs.structural_types,
                        &[],
                        call_context,
                    )?;
                    let declaration = super::super::ordinary_calls::emit_structural(
                        checked,
                        state.state,
                        operand,
                        prepared,
                        callee,
                        &catalogs.type_ids,
                        &catalogs.domain_ids,
                        claim_bindings,
                        true,
                        place_counter,
                        output,
                    )?;
                    catalogs.result_places.push(declaration);
                    Ok(declaration)
                };
            let declaration = crate::unit::attached_unit::structural_values::emit(
                checked,
                machine,
                state.state,
                operation,
                &catalogs.structural_types,
                &catalogs.type_ids,
                &mut catalogs.next_place,
                &mut catalogs.temporary_places,
                &mut emit_operand_call,
                &mut calls,
                evaluation,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
            )?;
            catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
            catalogs.result_places.push(declaration);
            continue;
        }
        if let CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value } = operation {
            let mut calls = catalogs.scalar_calls.emission_context();
            let evaluated = evaluation.source_value(
                checked,
                machine,
                state.state,
                result.statement_index,
                CheckedScalarExpressionRole::LocalInitializer {
                    binding_ordinal: result.binding_ordinal,
                },
                value,
                values.len(),
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                &mut calls,
            )?;
            catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
            if evaluated.scalar_type != terminal_scalar_type(result.primitive_type)? {
                return unsupported("graph scalar local carrier differs from its checked result");
            }
            evaluation
                .scalar_bindings
                .as_mut()
                .ok_or(LoweringError::Unsupported(
                    "graph scalar local namespace missing",
                ))?
                .append(
                    checked_trees::CheckedScalarBindingDestination::Immutable,
                    evaluated.scalar_type,
                    values.len(),
                )?;
            values.push(evaluated);
            continue;
        }
        if let CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) = operation {
            let bindings =
                evaluation
                    .scalar_bindings
                    .as_ref()
                    .ok_or(LoweringError::Unsupported(
                        "byte-view write has no scalar namespace",
                    ))?;
            let index = bindings.expression_at(
                checked,
                state.state,
                write.statement_index,
                CheckedScalarExpressionRole::AssignmentIndex,
            )?;
            let value = crate::emission::byte_store_scalar_value(
                bindings,
                checked,
                state.state,
                write.statement_index,
                &write.value,
                values,
            )?;
            let kind = crate::emission::byte_sequence_write::emit(
                write,
                parameters,
                &catalogs.structural_types,
                &index,
                &value,
                values,
                next_value,
                &mut catalogs.scalar_calls.next_call_obligation,
                operations,
            )?;
            let id = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                id,
                result: OperationResult::Unit,
                kind,
            });
            continue;
        }
        if let CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) =
            operation
        {
            let bindings =
                evaluation
                    .scalar_bindings
                    .as_ref()
                    .ok_or(LoweringError::Unsupported(
                        "indexed byte store has no scalar namespace",
                    ))?;
            let index = bindings.expression_at(
                checked,
                state.state,
                store.statement_index,
                CheckedScalarExpressionRole::AssignmentIndex,
            )?;
            let value = crate::emission::byte_store_scalar_value(
                bindings,
                checked,
                state.state,
                store.statement_index,
                &store.value,
                values,
            )?;
            let kind = crate::emission::structural_byte_sequence_index_store::emit(
                store,
                parameters,
                &catalogs.structural_types,
                &index,
                &value,
                values,
                next_value,
                &mut catalogs.scalar_calls.next_call_obligation,
                operations,
            )?;
            let id = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                id,
                result: OperationResult::Unit,
                kind,
            });
            continue;
        }
        if let CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store) = operation {
            if let std::borrow::Cow::Owned(types) = &mut catalogs.structural_types {
                crate::emission::structural_byte_sequence_store::literal_view_type(types)?;
            }
            let kind = crate::emission::structural_byte_sequence_store::emit(
                store,
                parameters,
                &catalogs.structural_types,
                &mut catalogs.temporary_places,
                &mut catalogs.next_place,
                next_value,
                &mut catalogs.scalar_calls.next_call_obligation,
                operations,
            )?;
            let id = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                id,
                result: OperationResult::Unit,
                kind,
            });
            continue;
        }
        if let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) = operation {
            state_graph::body::emit_store(
                checked, machine, state, store, catalogs, parameters, evaluation, values,
                next_value, next_block, next_edge, operations,
            )?;
            continue;
        }
        if let CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index,
            destination,
            path,
            value,
        } = operation
        {
            let checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index } =
                destination
            else {
                return unsupported(
                    "composed primitive store has no retained parameter destination",
                );
            };
            let parameter =
                parameters
                    .get(*parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "composed primitive store parameter is absent",
                    ))?;
            let destination = crate::emission::primitive_store::parameter_destination(
                parameter,
                path,
                &catalogs.structural_types,
            )?;
            let mut calls = catalogs.scalar_calls.emission_context();
            let kind = crate::emission::primitive_store::emit_assignment(
                checked,
                machine,
                state.state,
                *statement_index,
                destination,
                value,
                evaluation,
                values.len(),
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                &mut calls,
            )?;
            catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
            let id = operations.allocate();
            operations.push(Operation {
                static_reach_binding: None,
                id,
                result: OperationResult::Unit,
                kind,
            });
            continue;
        }
        if let CheckedUnitEffectOperationPlan::CallContinuationCleanup {
            affine_discards, ..
        } = operation
        {
            let mut discards = Vec::new();
            let mut residuals = Vec::new();
            for discard in affine_discards {
                let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal,
                } = discard.source
                else {
                    return unsupported(
                        "Unit graph continuation cleanup requires a result binding",
                    );
                };
                let produced =
                    state_graph::case_emission::result(state, binding_ordinal, operations)?;
                if discard.path.is_empty() {
                    discards.push(produced.place);
                } else {
                    residuals.push(terminal_psi::StructuralAffineDiscard {
                        place: produced.place,
                        path: lower_structural_path(&discard.path),
                        structural_type: lookup_type_id(
                            &catalogs.type_ids,
                            &discard.type_identity,
                        )?,
                    });
                }
            }
            evaluation.cleanup_continuation(
                discards, residuals, values, next_value, next_block, next_edge, operations,
            )?;
            continue;
        }
        let (arguments, byte_argument_places) = literal_arguments::evaluate(
            checked,
            machine,
            state.state,
            operation,
            catalogs,
            evaluation,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
        )?;
        match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. } => {
                emit_boundary_call_operation(
                    state,
                    operation,
                    &catalogs.lowered_boundaries,
                    &catalogs.type_ids,
                    &catalogs.structural_types,
                    parameters,
                    claim_bindings,
                    arguments.as_deref(),
                    &byte_argument_places,
                    &mut catalogs.next_place,
                    &mut catalogs.result_places,
                    operations,
                )?
            }
            CheckedUnitEffectOperationPlan::CallUnit { .. }
            | CheckedUnitEffectOperationPlan::StructuralCall { .. } => {
                internal_calls::emission::emit_call_operation(
                    checked,
                    state,
                    operation,
                    &catalogs.internal_targets,
                    parameters,
                    &catalogs.type_ids,
                    &catalogs.domain_ids,
                    claim_bindings,
                    &catalogs.structural_types,
                    arguments.as_deref(),
                    values.as_slice(),
                    erased_parameters,
                    &mut catalogs.scalar_calls,
                    &byte_argument_places,
                    &mut catalogs.next_place,
                    &mut catalogs.result_places,
                    operations,
                )?
            }
            _ => return unsupported("composed Unit operation escaped exact call custody"),
        }
        if let CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate, result, ..
        }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
            coordinate, result, ..
        } = operation
        {
            let occurrence = operations
                .source_calls
                .iter()
                .find(|occurrence| {
                    occurrence.source_state == state.state
                        && occurrence.statement_index == coordinate.statement_index as usize
                        && occurrence.call_ordinal == coordinate.call_ordinal as usize
                })
                .ok_or(LoweringError::Unsupported(
                    "structural call result lost its source occurrence",
                ))?;
            let produced = operations
                .iter()
                .find(|operation| operation.id == occurrence.terminal_operation)
                .and_then(|operation| match &operation.result {
                    OperationResult::Structural(result) => Some(result.clone()),
                    _ => None,
                })
                .ok_or(LoweringError::Unsupported(
                    "structural call did not establish its result",
                ))?;
            evaluation.establish_structural_result(
                checked,
                state.state,
                result,
                produced,
                &catalogs.structural_types,
                operations,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_boundary_call_operation(
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    boundaries: &[catalogs::LoweredComposedBoundary],
    type_ids: &[(String, StructuralTypeId)],
    structural_types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    scalar_values: Option<&[ValueDeclaration]>,
    byte_argument_places: &[PlaceId],
    next_place: &mut u64,
    result_places: &mut Vec<StructuralPlaceDeclaration>,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    let (CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        source_site,
        target_machine,
        scalar_arguments,
        structural_arguments,
        completion_receipts,
        ..
    }
    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
        coordinate,
        source_site,
        target_machine,
        scalar_arguments,
        structural_arguments,
        completion_receipts,
        ..
    }) = operation
    else {
        unreachable!("admission retained one boundary call")
    };
    let target = boundaries
        .iter()
        .find(|candidate| candidate.source == *target_machine)
        .ok_or(LoweringError::Unsupported(
            "composed Unit boundary target is absent from its exact catalog",
        ))?;
    if scalar_arguments.len() != target.scalar_parameters.len() {
        return unsupported("composed Unit boundary scalar arity drifted");
    }
    let expected_claim_arguments = structural_arguments
        .iter()
        .enumerate()
        .flat_map(|(argument_index, argument)| {
            state
                .entry_claims
                .iter()
                .filter(move |claim| {
                    Some(claim.parameter_index) == argument.source_parameter_index()
                        && (argument.path.is_empty() || claim.path == argument.path)
                })
                .map(move |_| {
                    u32::try_from(argument_index).map_err(|_| {
                        LoweringError::Unsupported(
                            "composed Unit boundary argument index exceeds u32",
                        )
                    })
                })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    validate_transfer_shape(
        structural_arguments,
        completion_receipts,
        parameters,
        &[],
        &[],
        &target.checked_structural_parameters,
        type_ids,
        structural_types,
        &expected_claim_arguments,
        &[],
        None,
    )?;
    let arguments = super::super::argument_evaluation::validated_values(
        scalar_values,
        &target.scalar_parameters,
    )?
    .into_iter()
    .map(|value| value.id)
    .collect();
    let call_id = operations.allocate();
    let result = match operation {
        CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. } => {
            let BoundaryMachineResult::Structural(result) = &target.result else {
                return unsupported("composed Unit structural boundary lost its result");
            };
            let place = place_id(allocate_dense(next_place)?);
            result_places.push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: call_id,
                    structural_type: result.structural_type,
                },
            });
            OperationResult::Structural(StructuralOperationResult {
                place,
                structural_type: result.structural_type,
                multiplicity: result.multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            })
        }
        _ => OperationResult::Unit,
    };
    operations.record_source_call(
        SourceCallCoordinate {
            state: state.state,
            statement_index: usize::try_from(coordinate.statement_index).map_err(|_| {
                LoweringError::Unsupported("composed Unit statement coordinate exceeds usize")
            })?,
            call_ordinal: usize::try_from(coordinate.call_ordinal).map_err(|_| {
                LoweringError::Unsupported("composed Unit call coordinate exceeds usize")
            })?,
        },
        *source_site,
        call_id,
        *target_machine,
    )?;
    operations.push(Operation {
        static_reach_binding: None,
        id: call_id,
        result,
        kind: OperationKind::BoundaryCall {
            boundary: target.id,
            arguments,
            structural_arguments: lower_structural_arguments(
                structural_arguments,
                parameters,
                &[],
                &[],
                byte_argument_places,
                &[],
            )?,
            completion_receipts: completion_receipts
                .iter()
                .map(|receipt| {
                    Ok(CompletionReceipt {
                        claim: lookup_claim_id(claim_bindings, receipt.claim_identity)?,
                        argument_index: receipt.argument_index,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?,
        },
    });
    Ok(())
}

pub(super) fn finish_module(
    source_root: symbols::SymbolHandle,
    mut machines: Vec<TerminalMachine>,
    mut catalogs: catalogs::ComposedCatalogs,
    mut source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
) -> Result<SourceMappedLowered, LoweringError> {
    let mut source_machine_ids = catalogs.scalar_calls.machine_ids.clone();
    if !source_machine_ids
        .iter()
        .any(|(source, _)| *source == source_root)
    {
        source_machine_ids.push((source_root, machine_id(1)));
    }
    for machine in &mut machines {
        machine.blocks.sort_by_key(|block| block.id);
    }
    let root = machines.first_mut().ok_or(LoweringError::Unsupported(
        "composed Unit emission produced no entry machine",
    ))?;
    root.contract.crash_routes =
        lower_checked_crash_route_buckets(&catalogs.root_crash_routes, &root.parameters)?;
    let entry = machines
        .first()
        .map(|machine| machine.id)
        .ok_or(LoweringError::Unsupported(
            "composed Unit emission produced no entry machine",
        ))?;
    if let Some(mut lowered) = catalogs.shared_units.take() {
        if lowered.semantic_module.entry != entry {
            return unsupported("shared Unit module lost its reserved composed entry");
        }
        machines.append(&mut lowered.semantic_module.machines);
        lowered.semantic_module.machines = machines;
        source_call_occurrences.append(&mut lowered.source_call_occurrences);
        lowered.source_call_occurrences = source_call_occurrences;
        finalize_operation_proofs(&mut lowered)?;
        return SourceMappedLowered::new(lowered, source_machine_ids);
    }
    let mut lowered = LoweredPsi {
        semantic_module: TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry,
            structural_types: catalogs.structural_types.into_owned(),
            structural_domains: Vec::new(),
            services: catalogs.services.into_owned(),
            root_service_reach: catalogs.root_service_reach.into_owned(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: catalogs.boundary_machines.into_owned(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines,
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences: Vec::new(),
        selected_ieee_float_comparison_occurrences: Vec::new(),
        selected_integer_comparison_occurrences: Vec::new(),
    };
    catalogs.scalar_calls.append_to(&mut lowered)?;
    finalize_operation_proofs(&mut lowered)?;
    SourceMappedLowered::new(lowered, source_machine_ids)
}
