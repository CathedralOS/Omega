//! Check a retained common-return CFG in place, independently of construction.

use super::shared::*;
use legalized_operations::LegalizedSharedReturnConditionalFunction;
use optimization_unit::FuelSettlement;

pub(super) fn validate(
    index: usize,
    native_target: target::NativeTarget,
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    proposed: &LegalizedSharedReturnConditionalFunction,
) -> Result<(), LegalizationError> {
    let invalid = || Error::NonCanonicalLegalizedPlan;
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"));
    if proposed.machine != target.machine
        || proposed.machine != abstracted.machine
        || proposed.machine != optimized.machine
        || proposed.attachment.is_some()
        || target.attachment.is_some()
        || abstracted.attachment.is_some()
        || proposed.provenance != target.provenance
        || Some(&proposed.abi) != target.fixed_integer_scalar_abi.as_ref()
        || target.mixed_structural_scalar_abi.is_some()
        || !abstracted.structural_parameters.is_empty()
        || !optimized.structural_parameters.is_empty()
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.declared_places.is_empty()
        || !abstracted.published_service_ceiling.is_empty()
        || !optimized.published_service_ceiling.is_empty()
        || abstracted.block_entries.len() != 4
        || optimized.blocks.len() != 4
        || abstracted.operations.len() != 7
        || abstracted.parameters.len() != 2
        || optimized.parameters.len() != 2
        || proposed.entry_block != abstracted.entry
        || proposed.entry_block != optimized.entry
        || proposed.return_parameter.scalar_type != scalar
    {
        return Err(invalid());
    }
    let ids = [
        proposed.entry_block,
        proposed.return_block,
        proposed.when_true.block,
        proposed.when_false.block,
    ];
    if ids.iter().collect::<std::collections::BTreeSet<_>>().len() != 4 {
        return Err(invalid());
    }
    for (position, entry) in abstracted.block_entries.iter().enumerate() {
        let block = optimized.blocks.get(position).ok_or_else(invalid)?;
        let end = abstracted
            .block_entries
            .get(position + 1)
            .map_or(abstracted.operations.len(), |next| next.operation_offset);
        let operations = abstracted
            .operations
            .get(entry.operation_offset..end)
            .ok_or_else(invalid)?;
        if block.id != entry.block
            || !ids.contains(&block.id)
            || entry.parameters.len() != block.parameters.len()
            || entry
                .parameters
                .iter()
                .zip(&block.parameters)
                .any(|(declared, value)| {
                    declared.value != value.value || declared.scalar_type != value.scalar_type
                })
            || operations.len() != block.nodes.len()
            || operations
                .iter()
                .zip(&block.nodes)
                .any(|(operation, node)| operation != &node.operation)
        {
            return Err(invalid());
        }
    }
    let entry = optimized
        .blocks
        .iter()
        .find(|block| block.id == proposed.entry_block)
        .ok_or_else(invalid)?;
    let returned = optimized
        .blocks
        .iter()
        .find(|block| block.id == proposed.return_block)
        .ok_or_else(invalid)?;
    if entry.id != optimized.blocks[0].id
        || !entry.parameters.is_empty()
        || entry.nodes.len() != 2
        || returned.parameters != [proposed.return_parameter]
        || returned.nodes.len() != 1
    {
        return Err(invalid());
    }
    let condition = super::conditions::replay(
        index,
        native_target.architecture,
        target,
        abstracted,
        optimized,
        proposed.condition_source,
        &proposed.condition,
    )?;
    if condition.conditional_node_index != 1
        || ScalarType::Integer(condition.result_type) != scalar
        || !matches!(
            proposed.condition,
            LegalizedCondition::IntegerEqualParametersV1 { .. }
                | LegalizedCondition::IntegerLessThanParametersV1 { .. }
                | LegalizedCondition::IntegerLessOrEqualParametersV1 { .. }
                | LegalizedCondition::I64LessThanParametersV1 { .. }
                | LegalizedCondition::I64LessOrEqualParametersV1 { .. }
        )
    {
        return Err(invalid());
    }
    let branch = &entry.nodes[1];
    let AbstractOperation::Conditional {
        condition: branch_value,
        when_true,
        when_false,
    } = &branch.operation
    else {
        return Err(invalid());
    };
    if *branch_value != proposed.condition_source
        || branch.successors.len() != 2
        || !branch.provenance.is_empty()
        || !branch.fuel.is_empty()
    {
        return Err(invalid());
    }
    let mut expected_operations = condition.provenance_operations;
    for (((arm, successor), edge), target_arm) in [&proposed.when_true, &proposed.when_false]
        .into_iter()
        .zip([when_true, when_false])
        .zip(&branch.successors)
        .zip([condition.when_true, condition.when_false])
    {
        let block = optimized
            .blocks
            .iter()
            .find(|block| block.id == arm.block)
            .ok_or_else(invalid)?;
        if block.parameters != arm.parameters
            || block.nodes.len() != 2
            || arm.branch_edge != successor.psi_edge
            || arm.block != successor.target
            || arm.branch_bindings != successor.bindings
            || !successor.trivial_affine_discards.is_empty()
            || !edge.trivial_affine_discards.is_empty()
            || edge.psi_edge != arm.branch_edge
            || edge.target != arm.block
            || edge.bindings != arm.branch_bindings
            || edge.provenance != [PsiProvenance::Edge(arm.branch_edge)]
            || edge.fuel != arm.branch_fuel
            || !exact_fuel(&arm.branch_fuel, PsiProvenance::Edge(arm.branch_edge))
        {
            return Err(invalid());
        }
        let constant = &block.nodes[0];
        let jump = &block.nodes[1];
        let AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } = &constant.operation
        else {
            return Err(invalid());
        };
        if *psi_operation != arm.constant.constant_operation
            || *result != arm.constant.source_value
            || *value != arm.constant.value
            || *scalar_type != scalar
            || constant.definitions
                != [optimization_unit::ValueDefinition {
                    value: *result,
                    scalar_type: scalar,
                    site: arm.constant.definition_site,
                }]
            || constant.provenance != [PsiProvenance::Operation(*psi_operation)]
            || constant.fuel != arm.constant.fuel
            || !exact_fuel(&arm.constant.fuel, PsiProvenance::Operation(*psi_operation))
            || !constant.successors.is_empty()
        {
            return Err(invalid());
        }
        let AbstractOperation::Jump {
            psi_edge,
            target: join,
            bindings,
            trivial_affine_discards,
        } = &jump.operation
        else {
            return Err(invalid());
        };
        let [transfer] = jump.successors.as_slice() else {
            return Err(invalid());
        };
        if *psi_edge != arm.transfer_edge
            || *join != proposed.return_block
            || bindings != &[arm.transfer_binding]
            || arm.transfer_binding.argument != arm.constant.source_value
            || arm.transfer_binding.parameter != proposed.return_parameter.value
            || arm.transfer_binding.scalar_type != scalar
            || !trivial_affine_discards.is_empty()
            || !transfer.trivial_affine_discards.is_empty()
            || transfer.psi_edge != *psi_edge
            || transfer.target != *join
            || transfer.bindings != *bindings
            || transfer.provenance != [PsiProvenance::Edge(*psi_edge)]
            || transfer.fuel != arm.transfer_fuel
            || !exact_fuel(&arm.transfer_fuel, PsiProvenance::Edge(*psi_edge))
            || !jump.provenance.is_empty()
            || !jump.fuel.is_empty()
        {
            return Err(invalid());
        }
        let TargetIntegerControl::Return {
            psi_return_edge,
            source_value,
            expression:
                TargetIntegerExpression::Immediate {
                    value: target_value,
                    source_value: expression_source,
                },
        } = target_arm.control.as_ref()
        else {
            return Err(invalid());
        };
        if *psi_return_edge != proposed.return_edge
            || *source_value != proposed.return_parameter.value
            || target_value != value
            || *expression_source != proposed.return_parameter.value
        {
            return Err(invalid());
        }
        expected_operations.push(*psi_operation);
    }
    let result = &returned.nodes[0];
    let AbstractOperation::Return {
        psi_edge,
        result: result_value,
        value,
        scalar_type,
        cleanup_actions,
    } = &result.operation
    else {
        return Err(invalid());
    };
    if *psi_edge != proposed.return_edge
        || *value != proposed.return_parameter.value
        || *scalar_type != scalar
        || !cleanup_actions.is_empty()
        || *result_value != proposed.abi.result.value
        || ScalarType::Integer(proposed.abi.result.scalar_type) != scalar
        || result.provenance != [PsiProvenance::Edge(*psi_edge)]
        || result.fuel != proposed.return_fuel
        || !exact_fuel(&proposed.return_fuel, PsiProvenance::Edge(*psi_edge))
        || !result.successors.is_empty()
        || proposed.provenance.operations != expected_operations
    {
        return Err(invalid());
    }
    let mut edges = vec![
        proposed.when_true.branch_edge,
        proposed.when_false.branch_edge,
        proposed.return_edge,
        proposed.when_true.transfer_edge,
        proposed.when_false.transfer_edge,
    ];
    edges.sort();
    let mut retained = proposed.provenance.edges.clone();
    retained.sort();
    if edges != retained {
        return Err(invalid());
    }
    let call = calling_conventions::evaluate_call_plan(
        calling_conventions::CallingPolicy::native_for_target(native_target),
        &calling_conventions::CallSignature {
            parameters: vec![calling_conventions::ValueShape::integer(8, 8); 2],
            result: Some(calling_conventions::ValueShape::integer(8, 8)),
        },
    )
    .map_err(|_| invalid())?;
    if proposed.abi.call_plan != call
        || Some(&proposed.abi.result.placement) != call.result.as_ref()
        || proposed.abi.parameters.len() != 2
        || proposed
            .abi
            .parameters
            .iter()
            .zip(&call.parameters)
            .zip(&abstracted.parameters)
            .zip(&optimized.parameters)
            .any(|(((value, placement), declared), checked)| {
                value.placement != *placement
                    || value.value != declared.value
                    || value.value != checked.value
                    || ScalarType::Integer(value.scalar_type) != declared.scalar_type
                    || declared.scalar_type != checked.scalar_type
            })
    {
        return Err(invalid());
    }
    Ok(())
}

fn exact_fuel(fuel: &[FuelSettlement], site: PsiProvenance) -> bool {
    !fuel.is_empty() && fuel.iter().all(|value| value.site == site)
}
