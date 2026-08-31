//! Five-block Terminal emission for two conditional frontiers.

use super::*;

pub(super) fn emit(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
    admitted: super::admission::AdmittedNested<'_>,
    catalogs: super::super::catalogs::ComposedCatalogs,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let entry_parameters = [
        ValueDeclaration {
            id: value_id(1),
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            id: value_id(2),
            scalar_type: ScalarType::Boolean,
        },
    ];
    let dispatch_parameter = ValueDeclaration {
        id: value_id(3),
        scalar_type: ScalarType::Boolean,
    };
    let state_ids = [
        block_id(1),
        block_id(2),
        block_id(3),
        block_id(4),
        block_id(5),
    ];
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        guard: entry_guard, ..
    } = &admitted.entry.terminator
    else {
        unreachable!("nested admission retained an outer conditional")
    };
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        guard: dispatch_guard,
        ..
    } = &admitted.dispatch.terminator
    else {
        unreachable!("nested admission retained an inner conditional")
    };
    let entry_guard = lower_checked_scalar_expression(entry_guard)?;
    let dispatch_guard = lower_checked_scalar_expression(dispatch_guard)?;
    validate_direct_parameter_types(&entry_guard, &[ScalarType::Boolean, ScalarType::Boolean])?;
    validate_direct_parameter_types(&dispatch_guard, &[ScalarType::Boolean])?;
    let mut next_value = 4_u64;
    let mut next_edge = 1_u64;
    let mut entry_operations = OperationBuffer::new(0);
    let entry_condition = emit_direct_expression(
        &entry_guard,
        &entry_parameters,
        &mut next_value,
        &mut entry_operations,
    );
    let entry_next_operation = entry_operations.next_identity;
    let entry_block = Block {
        id: state_ids[0],
        parameters: Vec::new(),
        operations: entry_operations.operations,
        terminator: Terminator::Conditional {
            condition: entry_condition,
            when_true: successor(state_ids[1], vec![entry_parameters[1].id], &mut next_edge)?,
            when_false: successor(state_ids[4], Vec::new(), &mut next_edge)?,
        },
    };
    let mut dispatch_operations = OperationBuffer::new(entry_next_operation - 1);
    let dispatch_condition = emit_direct_expression(
        &dispatch_guard,
        std::slice::from_ref(&dispatch_parameter),
        &mut next_value,
        &mut dispatch_operations,
    );
    let mut next_operation = dispatch_operations.next_identity;
    let dispatch_block = Block {
        id: state_ids[1],
        parameters: vec![dispatch_parameter],
        operations: dispatch_operations.operations,
        terminator: Terminator::Conditional {
            condition: dispatch_condition,
            when_true: successor(state_ids[2], Vec::new(), &mut next_edge)?,
            when_false: successor(state_ids[3], Vec::new(), &mut next_edge)?,
        },
    };
    let mut blocks = vec![entry_block, dispatch_block];
    let mut source_call_occurrences = Vec::new();
    for (state, block) in admitted.leaf_calls.leaves.into_iter().zip(&state_ids[2..]) {
        let (leaf, mut occurrences) = match &state.operations[0] {
            CheckedUnitEffectOperationPlan::BoundaryCall { .. } => {
                super::super::emission::emit_boundary_leaf(
                    state,
                    *block,
                    &catalogs.lowered_boundaries,
                    &catalogs.type_ids,
                    &catalogs.structural_types,
                    &[],
                    &[],
                    &mut next_value,
                    &mut next_operation,
                    &mut next_edge,
                )?
            }
            CheckedUnitEffectOperationPlan::CallUnit { .. } => {
                super::super::internal_calls::emission::emit_leaf(
                    state,
                    *block,
                    &catalogs.internal_targets,
                    &mut next_operation,
                    &mut next_edge,
                )?
            }
            _ => unreachable!("nested admission retained exact call leaves"),
        };
        blocks.push(leaf);
        source_call_occurrences.append(&mut occurrences);
    }
    let attachment = lookup_type_id(&catalogs.type_ids, &plan.attachment_type_identity)?;
    let attachment_declaration = catalogs
        .structural_types
        .iter()
        .find(|declaration| declaration.id == attachment)
        .expect("nested composed attachment was selected");
    let provider_boundaries = catalogs
        .lowered_boundaries
        .iter()
        .map(|boundary| (boundary.source, boundary.id))
        .collect::<Vec<_>>();
    let mut next_place = catalogs.next_place;
    let structural_places =
        super::super::super::provider_attachments::lower_provider_attachment_places(
            attachment,
            attachment_declaration,
            &plan.provider_attachment_requirements,
            &provider_boundaries,
            &mut next_place,
        )?;
    let machine = TerminalMachine {
        id: machine_id(1),
        attachment: Some(attachment),
        parameters: entry_parameters.to_vec(),
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
    let mut machines = vec![machine];
    let mut next_block = 6_u64;
    machines.extend(super::super::internal_calls::emission::emit_targets(
        checked,
        &catalogs.internal_targets,
        &catalogs.type_ids,
        &catalogs.service_ids,
        &mut next_operation,
        &mut next_block,
        &mut next_edge,
    )?);
    super::super::emission::finish_module(machines, catalogs, source_call_occurrences)
}

fn successor(
    target: BlockId,
    arguments: Vec<ValueId>,
    next_edge: &mut u64,
) -> Result<SuccessorEdge, LoweringError> {
    Ok(SuccessorEdge {
        edge: edge_id(allocate_dense(next_edge)?),
        target,
        arguments,
        trivial_affine_discards: Vec::new(),
    })
}
