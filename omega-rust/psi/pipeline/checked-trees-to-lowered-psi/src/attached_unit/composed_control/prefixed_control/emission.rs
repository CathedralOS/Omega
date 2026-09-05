//! Terminal emission for the finite scalar-prefix acyclic family.

use super::*;

pub(super) fn emit(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
    admitted: super::admission::AdmittedPrefixed<'_>,
    catalogs: super::super::catalogs::ComposedCatalogs,
) -> Result<LoweredPsi, LoweringError> {
    let control_parameters = (0..admitted.controls.len())
        .map(|index| {
            Ok(ValueDeclaration {
                id: value_id(dense_identity(index)?),
                scalar_type: ScalarType::Boolean,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let state_ids = (0..plan.states.len())
        .map(|index| Ok(block_id(dense_identity(index)?)))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut next_edge = 1_u64;
    let mut blocks = (0..admitted.controls.len() - 1)
        .map(|index| {
            Ok(Block {
                id: state_ids[index],
                parameters: (index != 0)
                    .then_some(control_parameters[index])
                    .into_iter()
                    .collect(),
                operations: Vec::new(),
                terminator: Terminator::Jump {
                    edge: edge_id(allocate_dense(&mut next_edge)?),
                    target: state_ids[index + 1],
                    arguments: vec![control_parameters[index].id],
                    trivial_affine_discards: Vec::new(),
                },
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let CheckedComposedUnitControlTerminatorPlan::Conditional { guard, .. } =
        &admitted.dispatch.terminator
    else {
        unreachable!("prefixed admission retained one conditional dispatch")
    };
    let guard = lower_checked_scalar_expression(guard)?;
    validate_direct_parameter_types(&guard, &[ScalarType::Boolean])?;
    let dispatch_index = admitted.controls.len() - 1;
    let dispatch_parameter = control_parameters[dispatch_index];
    let mut next_value = u64::try_from(control_parameters.len())
        .map_err(|_| LoweringError::Unsupported("prefixed scalar chain exceeds u64"))?
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "prefixed scalar value identity space is exhausted",
        ))?;
    let mut dispatch_operations = OperationBuffer::new(0);
    let condition = emit_direct_expression(
        &guard,
        std::slice::from_ref(&dispatch_parameter),
        &mut next_value,
        &mut dispatch_operations,
    );
    let mut next_operation = dispatch_operations.next_identity;
    let dispatch_block = Block {
        id: state_ids[dispatch_index],
        parameters: vec![dispatch_parameter],
        operations: dispatch_operations.operations,
        terminator: Terminator::Conditional {
            condition,
            when_true: empty_successor(state_ids[dispatch_index + 1], &mut next_edge)?,
            when_false: empty_successor(state_ids[dispatch_index + 2], &mut next_edge)?,
        },
    };
    blocks.push(dispatch_block);
    let mut source_call_occurrences = Vec::new();
    for (state, block) in admitted
        .leaf_calls
        .leaves
        .into_iter()
        .zip(&state_ids[dispatch_index + 1..])
    {
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
            _ => unreachable!("prefixed admission retained one exact leaf call"),
        };
        blocks.push(leaf);
        source_call_occurrences.append(&mut occurrences);
    }
    let attachment = lookup_type_id(&catalogs.type_ids, &plan.attachment_type_identity)?;
    let attachment_declaration = catalogs
        .structural_types
        .iter()
        .find(|declaration| declaration.id == attachment)
        .expect("prefixed composed attachment was selected");
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
        parameters: vec![control_parameters[0]],
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
    let mut next_block = u64::try_from(state_ids.len())
        .map_err(|_| LoweringError::Unsupported("prefixed block count exceeds u64"))?
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "prefixed block identity space is exhausted",
        ))?;
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

fn empty_successor(target: BlockId, next_edge: &mut u64) -> Result<SuccessorEdge, LoweringError> {
    Ok(SuccessorEdge {
        edge: edge_id(allocate_dense(next_edge)?),
        target,
        arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    })
}
