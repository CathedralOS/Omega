//! Dynamic Terminal emission for general acyclic conditional graphs.

use super::*;

pub(super) fn emit(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
    admitted: super::admission::AdmittedNested<'_>,
    catalogs: super::super::catalogs::ComposedCatalogs,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let control_count = admitted.controls.len();
    let state_ids = (0..plan.states.len())
        .map(|index| Ok(block_id(dense_identity(index)?)))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut next_value = 1_u64;
    let control_parameters = admitted
        .controls
        .iter()
        .map(|state| {
            (0..state.scalar_parameters.len())
                .map(|_| {
                    Ok(ValueDeclaration {
                        id: value_id(allocate_dense(&mut next_value)?),
                        scalar_type: ScalarType::Boolean,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut next_edge = 1_u64;
    let mut next_operation = 1_u64;
    let mut blocks = Vec::with_capacity(plan.states.len());
    for index in 0..control_count {
        let CheckedComposedUnitControlTerminatorPlan::Conditional {
            guard,
            when_true,
            when_false,
        } = &admitted.controls[index].terminator
        else {
            unreachable!("nested admission retained conditional controls")
        };
        let guard = lower_checked_scalar_expression(guard)?;
        let parameter_types = vec![ScalarType::Boolean; control_parameters[index].len()];
        validate_direct_parameter_types(&guard, &parameter_types)?;
        let mut operations = OperationBuffer::new(next_operation - 1);
        super::operations::emit(
            &admitted.controls[index],
            &catalogs.internal_targets,
            &mut operations,
        )?;
        let condition = emit_direct_expression(
            &guard,
            &control_parameters[index],
            &mut next_value,
            &mut operations,
        );
        next_operation = operations.next_identity;
        blocks.push(Block {
            id: state_ids[index],
            parameters: (index != 0)
                .then(|| control_parameters[index].clone())
                .unwrap_or_default(),
            operations: operations.operations,
            terminator: Terminator::Conditional {
                condition,
                when_true: lower_successor(
                    plan,
                    &state_ids,
                    &control_parameters[index],
                    when_true,
                    &mut next_edge,
                )?,
                when_false: lower_successor(
                    plan,
                    &state_ids,
                    &control_parameters[index],
                    when_false,
                    &mut next_edge,
                )?,
            },
        });
    }
    let mut source_call_occurrences = Vec::new();
    for (state, block) in admitted
        .leaf_calls
        .leaves
        .into_iter()
        .zip(&state_ids[control_count..])
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
        parameters: control_parameters[0].clone(),
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
        .map_err(|_| LoweringError::Unsupported("nested block count exceeds u64"))?
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "nested block identity space is exhausted",
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

fn lower_successor(
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
    state_ids: &[BlockId],
    source_parameters: &[ValueDeclaration],
    edge: &psi_checked_trees::CheckedStructuralControlSuccessorPlan,
    next_edge: &mut u64,
) -> Result<SuccessorEdge, LoweringError> {
    let target_index = plan
        .states
        .iter()
        .position(|state| state.state == edge.target_state)
        .ok_or(LoweringError::Unsupported(
            "nested emitted edge target disappeared",
        ))?;
    let arguments = edge
        .scalar_arguments
        .iter()
        .map(|argument| {
            source_parameters
                .get(
                    usize::try_from(argument.source_scalar_parameter_index).map_err(|_| {
                        LoweringError::Unsupported("nested scalar source index exceeds usize")
                    })?,
                )
                .map(|parameter| parameter.id)
                .ok_or(LoweringError::Unsupported(
                    "nested scalar edge names an unknown source parameter",
                ))
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    successor(state_ids[target_index], arguments, next_edge)
}
