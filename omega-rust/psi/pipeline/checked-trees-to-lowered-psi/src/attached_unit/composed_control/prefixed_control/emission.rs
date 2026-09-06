//! Terminal emission for the finite scalar-prefix acyclic family.

use super::*;

pub(super) fn emit(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
    admitted: super::admission::AdmittedPrefixed<'_>,
    mut catalogs: super::super::catalogs::ComposedCatalogs,
) -> Result<ComposedLowered, LoweringError> {
    let mut next_value = catalogs.next_value;
    let mut next_block = catalogs.next_block;
    let control_parameters = (0..admitted.controls.len())
        .map(|_| {
            Ok(ValueDeclaration {
                id: value_id(allocate_dense(&mut next_value)?),
                scalar_type: ScalarType::Boolean,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let state_ids = (0..plan.states.len())
        .map(|_| Ok(block_id(allocate_dense(&mut next_block)?)))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut next_edge = catalogs.next_edge;
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
    let mut dispatch_operations = OperationBuffer::new(catalogs.next_operation - 1);
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
        let (leaf, mut occurrences) = super::super::emission::emit_call_leaf(
            checked,
            plan.machine,
            state,
            *block,
            &mut catalogs,
            &[],
            &[],
            &[],
            &mut next_value,
            &mut next_block,
            &mut next_operation,
            &mut next_edge,
        )?;
        blocks.extend(leaf);
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
    super::super::emission::finish_module(
        plan.machine,
        vec![machine],
        catalogs,
        source_call_occurrences,
    )
}

fn empty_successor(target: BlockId, next_edge: &mut u64) -> Result<SuccessorEdge, LoweringError> {
    Ok(SuccessorEdge {
        edge: edge_id(allocate_dense(next_edge)?),
        target,
        arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    })
}
