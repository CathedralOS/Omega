//! Three-block Terminal emission after exact admission and catalog projection.

use super::*;

pub(super) fn emit_composed_unit_control(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
    admitted: admission::AdmittedComposedUnit<'_>,
    catalogs: catalogs::ComposedCatalogs,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let [entry, _, _] = plan.states.as_slice() else {
        unreachable!("admission retained exactly three states")
    };
    let entry_parameters = entry
        .scalar_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            Ok(ValueDeclaration {
                id: value_id(dense_identity(index)?),
                scalar_type: terminal_scalar_type(parameter.primitive_type)?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let parameter_types = entry_parameters
        .iter()
        .map(|parameter| parameter.scalar_type)
        .collect::<Vec<_>>();
    let CheckedComposedUnitControlTerminatorPlan::Conditional { guard, .. } = &entry.terminator
    else {
        unreachable!("admission retained one conditional entry")
    };
    let guard = lower_checked_scalar_expression(guard)?;
    validate_direct_parameter_types(&guard, &parameter_types)?;
    let state_ids = [block_id(1), block_id(2), block_id(3)];
    let mut next_value = u64::try_from(entry_parameters.len())
        .map_err(|_| LoweringError::Unsupported("composed Unit scalar arity exceeds u64"))?
        + 1;
    let mut next_edge = 1_u64;
    let mut entry_operations = OperationBuffer::new(0);
    let guard = emit_direct_expression(
        &guard,
        &entry_parameters,
        &mut next_value,
        &mut entry_operations,
    );
    let mut next_operation = entry_operations.next_identity;
    let mut blocks = vec![Block {
        id: state_ids[0],
        parameters: Vec::new(),
        operations: entry_operations.operations,
        terminator: Terminator::Conditional {
            condition: guard,
            when_true: empty_successor(state_ids[1], &mut next_edge)?,
            when_false: empty_successor(state_ids[2], &mut next_edge)?,
        },
    }];
    let mut source_call_occurrences = Vec::new();
    for (state, block) in admitted.leaves.into_iter().zip(&state_ids[1..]) {
        let (lowered_block, mut occurrences) = emit_leaf(
            state,
            *block,
            &catalogs.lowered_boundaries,
            &mut next_value,
            &mut next_operation,
            &mut next_edge,
        )?;
        blocks.push(lowered_block);
        source_call_occurrences.append(&mut occurrences);
    }
    let attachment = lookup_type_id(&catalogs.type_ids, &plan.attachment_type_identity)?;
    let attachment_declaration = catalogs
        .structural_types
        .iter()
        .find(|declaration| declaration.id == attachment)
        .expect("composed attachment declaration was selected");
    let provider_boundaries = catalogs
        .lowered_boundaries
        .iter()
        .map(|(symbol, boundary, _)| (*symbol, *boundary))
        .collect::<Vec<_>>();
    let mut next_place = 1_u64;
    let structural_places = super::super::provider_attachments::lower_provider_attachment_places(
        attachment,
        attachment_declaration,
        &plan.provider_attachment_requirements,
        &provider_boundaries,
        &mut next_place,
    )?;
    let machine = TerminalMachine {
        id: machine_id(1),
        attachment: Some(attachment),
        parameters: entry_parameters,
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places,
        entry_claims: Vec::new(),
        published_service_ceiling: lower_installation_machine_service_ceiling(
            checked,
            plan.machine,
            plan.contract_service_reach,
            plan.service_reach,
            &catalogs.service_ids,
        )?,
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: state_ids[0],
        blocks,
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    finish_module(machine, catalogs, source_call_occurrences)
}

fn emit_leaf(
    state: &psi_checked_trees::CheckedComposedUnitControlStatePlan,
    block: BlockId,
    boundaries: &[(
        psi_symbols::SymbolHandle,
        BoundaryMachineId,
        Vec<ScalarType>,
    )],
    next_value: &mut u64,
    next_operation: &mut u64,
    next_edge: &mut u64,
) -> Result<(Block, Vec<LoweredSourceCallOccurrence>), LoweringError> {
    let CheckedUnitEffectOperationPlan::BoundaryCall {
        coordinate,
        source_site,
        target_machine,
        scalar_arguments,
        ..
    } = &state.operations[0]
    else {
        unreachable!("admission retained one boundary call")
    };
    let (_, boundary, target_types) = boundaries
        .iter()
        .find(|(candidate, _, _)| candidate == target_machine)
        .ok_or(LoweringError::Unsupported(
            "composed Unit boundary target is absent from its exact catalog",
        ))?;
    if scalar_arguments.len() != target_types.len() {
        return unsupported("composed Unit boundary scalar arity drifted");
    }
    let mut operations = OperationBuffer::new(*next_operation - 1);
    let arguments = scalar_arguments
        .iter()
        .zip(target_types)
        .map(|(argument, target_type)| {
            let argument = lower_checked_scalar_expression(argument)?;
            if argument.scalar_type() != *target_type {
                return unsupported("composed Unit boundary scalar type drifted");
            }
            validate_direct_parameter_types(&argument, &[])?;
            Ok(emit_direct_expression(
                &argument,
                &[],
                next_value,
                &mut operations,
            ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let call_id = operations.allocate();
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
        id: call_id,
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: *boundary,
            arguments,
            structural_arguments: Vec::new(),
            completion_receipts: Vec::new(),
            requirement_obligations: Vec::new(),
        },
    });
    *next_operation = operations.next_identity;
    let OperationBuffer {
        operations,
        source_calls,
        ..
    } = operations;
    Ok((
        Block {
            id: block,
            parameters: Vec::new(),
            operations,
            terminator: Terminator::ReturnUnit {
                edge: edge_id(allocate_dense(next_edge)?),
                trivial_affine_discards: Vec::new(),
            },
        },
        source_calls,
    ))
}

fn empty_successor(target: BlockId, next_edge: &mut u64) -> Result<SuccessorEdge, LoweringError> {
    Ok(SuccessorEdge {
        edge: edge_id(allocate_dense(next_edge)?),
        target,
        arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    })
}

fn finish_module(
    machine: TerminalMachine,
    catalogs: catalogs::ComposedCatalogs,
    source_call_occurrences: Vec<LoweredSourceCallOccurrence>,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let mut lowered = LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine.id,
            structural_types: catalogs.structural_types,
            structural_domains: Vec::new(),
            services: catalogs.services,
            root_service_reach: catalogs.root_service_reach,
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            boundary_machines: catalogs.boundary_machines,
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            closed_conformance_applications: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![machine],
        },
        proof_bundle: ProofBundle::default(),
        debug_map: None,
        source_call_occurrences,
    };
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}
