//! Shared call emission for composed control.
use super::super::super::{
    BlockId, ClaimId, LoweredSourceCallOccurrence, PermissionClaimIdentity,
    StructuralParameterDeclaration, StructuralTypeDeclaration,
};
use super::super::{
    Block, BoundaryMachineResult, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
    CompletionReceipt, Operation, OperationKind, OperationResult, PlaceId, SemanticDomainId,
    StructuralDomainId, StructuralOperationResult, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeId, Terminator, ValueDeclaration, allocate_dense, edge_id, lookup_claim_id,
    lookup_machine_id, lookup_type_id, lower_checked_crash_route_buckets,
    lower_structural_arguments, lower_structural_path, place_id, terminal_scalar_type, unsupported,
    validate_transfer_shape, value_id,
};
use super::{
    CheckedTrees, LoweringError, catalogs, internal_calls, literal_arguments, state_graph,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, SourceCallCoordinate};
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::scalar_graph::scalar_call_closure::callee::CheckedScalarCallee;
use crate::scalar_graph::scalar_contracts::erased_proof_formal_declarations;

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
        erased_proof_formals: state.erased_proof_parameters.clone(),
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
        erased_proof_formals: erased_proof_formal_declarations(&state.erased_proof_parameters),
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
                        super::super::bodies::UnitPlans::published(
                            &checked.facts.flow.terminal_unit_effects,
                        ),
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
                        super::super::bodies::UnitPlans::published(
                            &checked.facts.flow.terminal_unit_effects,
                        ),
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
                        &state.erased_proof_parameters,
                        parameters,
                        &[],
                        &earlier,
                        &[],
                        &catalogs.type_ids,
                        &catalogs.structural_types,
                        &[],
                        &output.structural_values,
                        &catalogs.domain_ids,
                        claim_bindings,
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
                suspension_crossing: None,
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
                suspension_crossing: None,
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
                suspension_crossing: None,
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
        if let CheckedUnitEffectOperationPlan::WriteOnlyIndexedPrimitiveStore {
            statement_index,
            destination,
            path,
            index,
            value,
        } = operation
        {
            let checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index } =
                destination
            else {
                return unsupported(
                    "composed indexed primitive store has no retained parameter destination",
                );
            };
            let parameter =
                parameters
                    .get(*parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "composed indexed primitive store parameter is absent",
                    ))?;
            let destination = crate::emission::primitive_store::indexed_parameter_destination(
                parameter,
                path,
                &catalogs.structural_types,
            )?;
            let mut calls = catalogs.scalar_calls.emission_context();
            let kind = crate::emission::primitive_store::emit_indexed_assignment(
                checked,
                machine,
                state.state,
                *statement_index,
                destination,
                index,
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
                suspension_crossing: None,
                id,
                result: OperationResult::Unit,
                kind,
            });
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
                suspension_crossing: None,
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
            CheckedUnitEffectOperationPlan::ScalarCall {
                coordinate, result, ..
            } => {
                let position = values.len();
                emit_scalar_call_operation(
                    checked,
                    state,
                    operation,
                    parameters,
                    &catalogs.type_ids,
                    &catalogs.domain_ids,
                    claim_bindings,
                    &catalogs.structural_types,
                    arguments.as_deref(),
                    erased_parameters,
                    &mut catalogs.scalar_calls,
                    &byte_argument_places,
                    &catalogs.result_places,
                    values,
                    next_value,
                    operations,
                )?;
                // A retained call result is the next dense value, so it also
                // enters the source binding namespace at its checked ordinal,
                // exactly like an established scalar local. A discarded result
                // occupies no source position; call leaves without a prepared
                // namespace resolve ordinals through the dense identity map.
                if !crate::emission::call_source_custody::initializers::discards_result(
                    checked,
                    state.state,
                    *coordinate,
                )? && let Some(bindings) = evaluation.scalar_bindings.as_mut()
                {
                    bindings.append(
                        checked_trees::CheckedScalarBindingDestination::Immutable,
                        terminal_scalar_type(result.primitive_type)?,
                        position,
                    )?;
                }
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
                qualification_establishments: Vec::new(),
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
        suspension_crossing: None,
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

/// Lower one checked `ScalarCall` into the shared call lane. A store reading
/// the produced scalar shares the call's authored statement; the dense scalar
/// namespace still binds the result exactly like the ordinary sequencer does.
#[allow(clippy::too_many_arguments)]
fn emit_scalar_call_operation(
    checked: &CheckedTrees,
    state: &checked_trees::CheckedComposedUnitControlStatePlan,
    operation: &CheckedUnitEffectOperationPlan,
    parameters: &[StructuralParameterDeclaration],
    type_ids: &[(String, StructuralTypeId)],
    domain_ids: &[(SemanticDomainId, StructuralDomainId)],
    claim_bindings: &[(PermissionClaimIdentity, ClaimId)],
    structural_types: &[StructuralTypeDeclaration],
    scalar_values: Option<&[ValueDeclaration]>,
    caller_erased_formals: &[ValueDeclaration],
    scalar_calls: &mut super::scalar_calls::ComposedScalarCalls,
    byte_argument_places: &[PlaceId],
    result_places: &[StructuralPlaceDeclaration],
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::ScalarCall {
        coordinate,
        result,
        target_machine,
        target_state,
        erased_scalar_arguments,
        erased_proof_arguments,
        structural_arguments,
        claim_transfers,
        ..
    } = operation
    else {
        return unsupported("composed Unit scalar call custody drifted before emission");
    };
    let target = CheckedScalarCallee::find_for_unit_call(checked, *target_machine)?;
    if target.entry_state()? != *target_state
        || target.result_type()? != terminal_scalar_type(result.primitive_type)?
    {
        return unsupported(
            "composed Unit scalar call disagrees with its checked target signature",
        );
    }
    if usize::try_from(result.binding_ordinal)
        .ok()
        .and_then(|ordinal| ordinal.checked_add(state.scalar_parameters.len()))
        != Some(values.len())
    {
        return unsupported("Unit scalar result binding ordinal drifted from source order");
    }
    // The plan's scalar roster carries only retained operands; the proof-only
    // erased lane rejoins its own checked actuals.
    let arguments = super::super::argument_evaluation::validated_values(
        scalar_values,
        &target
            .parameter_types()?
            .iter()
            .map(|primitive| terminal_scalar_type(*primitive))
            .collect::<Result<Vec<_>, LoweringError>>()?,
    )?;
    if erased_scalar_arguments.len() != target.erased_parameters().len() {
        return unsupported(
            "composed Unit scalar call erased lane disagrees with its target roster",
        );
    }
    let erased_arguments = erased_scalar_arguments
        .iter()
        .map(|argument| {
            let checked_trees::CheckedCallScalarArgument::Pure(expression) = argument else {
                return unsupported(
                    "composed Unit scalar call erased actual must be a pure checked expression",
                );
            };
            crate::proofs::crash_routes::checked_scalar_term(
                expression,
                values.as_slice(),
                caller_erased_formals,
            )
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if erased_proof_arguments.len() != target.erased_proof_parameters().len() {
        return unsupported(
            "composed Unit scalar call erased proof lane disagrees with its target roster",
        );
    }
    let erased_proof_arguments = erased_proof_arguments
        .iter()
        .map(|term| {
            crate::scalar_graph::scalar_contracts::checked_proof_term(
                checked,
                term,
                &state.erased_proof_parameters,
            )
            .and_then(|term| {
                crate::scalar_graph::scalar_contracts::lowered_proof_term(
                    &term,
                    values.as_slice(),
                    caller_erased_formals,
                )
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    if checked
        .facts
        .contract_plans
        .for_machine(*target_machine)
        .is_none()
    {
        return Err(LoweringError::Unsupported(
            "composed Unit scalar call target has no checked contract",
        ));
    }
    let mut calls = scalar_calls.emission_context();
    let requirement_count = calls
        .requirement_counts
        .iter()
        .find_map(|(source, count)| (*source == *target_machine).then_some(*count))
        .ok_or(LoweringError::Unsupported(
            "composed Unit scalar call target has no prepared contract",
        ))?;
    let requirement_obligations = (0..requirement_count)
        .map(|_| calls.allocate_requirement())
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let callee = lookup_machine_id(calls.machine_ids, *target_machine)?;
    scalar_calls.next_call_obligation = calls.next_obligation_identity;
    let crash_continuations = lower_checked_crash_route_buckets(
        &crate::unit::effective_crash_routes(checked, *target_machine)?,
        &arguments,
    )?;
    let value = ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(allocate_dense(next_value)?),
        scalar_type: terminal_scalar_type(result.primitive_type)?,
    };
    let operation_id = operations.allocate();
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
        None,
        operation_id,
        *target_state,
    )?;
    let argument_ids = arguments
        .iter()
        .map(|argument| argument.id)
        .collect::<Vec<_>>();
    let kind = if !structural_arguments.is_empty()
        || !claim_transfers.is_empty()
        || target.requires_structural_frame()
    {
        if target.structural_parameters().is_empty() && !structural_arguments.is_empty() {
            return unsupported("structural scalar call has no structural checked body");
        }
        // Binding ordinals belong to this source state, whereas the place
        // catalog spans the whole emitted machine. Rejoin the current
        // operation registry.
        let earlier_results = if structural_arguments.iter().any(|argument| {
            argument
                .source_structural_result_binding_ordinal()
                .is_some()
        }) {
            operations
                .structural_values
                .iter()
                .enumerate()
                .map(|(binding_position, (ordinal, result))| {
                    if *ordinal as usize != binding_position {
                        return unsupported(
                            "composed scalar call result binding namespace is stale or duplicated",
                        );
                    }
                    let mut declarations = result_places
                        .iter()
                        .filter(|place| place.id == result.place);
                    let declaration = declarations.next().ok_or(LoweringError::Unsupported(
                        "composed scalar call completed result has no place declaration",
                    ))?;
                    if declarations.next().is_some()
                        || !matches!(declaration.kind,
                            StructuralPlaceKind::OperationResult { structural_type, producer }
                                if structural_type == result.structural_type && operations.operations.iter().any(|candidate|
                                    candidate.id == producer && candidate.result.structural() == Some(result)))
                    {
                        return unsupported(
                            "composed scalar call result declaration differs from its operation",
                        );
                    }
                    Ok((*declaration, false))
                })
                .collect::<Result<Vec<_>, LoweringError>>()?
        } else {
            Vec::new()
        };
        validate_transfer_shape(
            structural_arguments,
            claim_transfers,
            parameters,
            &[],
            &earlier_results,
            target.structural_parameters(),
            type_ids,
            structural_types,
            // A claim-carrying `self` formal fed by a completed result consumes
            // the moved frontier instead of publishing a transfer row; every
            // other entry claim expects exactly one.
            &target
                .entry_claims()
                .iter()
                .filter(|claim| {
                    target
                        .structural_parameters()
                        .get(claim.parameter_index as usize)
                        .zip(structural_arguments.get(claim.parameter_index as usize))
                        .is_none_or(|(parameter, argument)| {
                            !parameter.is_self
                                || argument
                                    .source_structural_result_binding_ordinal()
                                    .is_none()
                        })
                })
                .map(|claim| claim.parameter_index)
                .collect::<Vec<_>>(),
            &[],
            Some(
                crate::unit::attached_unit::parameters::StructuralResultCustody {
                    results: &operations.structural_values,
                    domains: domain_ids,
                    claims: claim_bindings,
                    target_entry_claims: target.entry_claims(),
                },
            ),
        )?;
        OperationKind::CallStructuralScalar {
            callee,
            arguments: argument_ids,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments: lower_structural_arguments(
                structural_arguments,
                parameters,
                &[],
                &earlier_results,
                byte_argument_places,
                &[],
            )?,
            claim_transfers: crate::unit::attached_unit::parameters::emitted_claim_transfers(
                structural_arguments,
                claim_transfers,
                target.structural_parameters(),
                &operations.structural_values,
                claim_bindings,
            )?,
            requirement_obligations,
            crash_continuations,
        }
    } else {
        OperationKind::Call {
            callee,
            arguments: argument_ids,
            erased_arguments,
            erased_proof_arguments,
            requirement_obligations,
            crash_continuations,
        }
    };
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id,
        result: OperationResult::Scalar(value),
        kind,
    });
    // A discarded call keeps its typed result without entering the caller's
    // scalar namespace; a retained one is the next dense value.
    if !crate::emission::call_source_custody::initializers::discards_result(
        checked,
        state.state,
        *coordinate,
    )? {
        values.push(value);
    }
    Ok(())
}
