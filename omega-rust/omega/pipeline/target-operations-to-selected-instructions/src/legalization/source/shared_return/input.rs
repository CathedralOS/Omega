//! Preserve the source CFG and its parameter transfers at a shared scalar return.

use super::super::conditions::DerivedCondition;
use super::super::shared::*;

pub(in crate::legalization::source) fn match_input<'a>(
    function: usize,
    native_target: target::NativeTarget,
    target: &'a target_operations::TargetFunction,
    abstracted: &'a abstract_operations::AbstractFunction,
    optimized: &'a optimization_unit::PsiOptimizationFunction,
) -> Result<MatchedFunction<'a>, LegalizationError> {
    let invalid = || Error::UnsupportedSourceShape { function };
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let scalar = ScalarType::Integer(integer);
    if target.machine != abstracted.machine
        || target.machine != optimized.machine
        || target.attachment.is_some()
        || abstracted.attachment.is_some()
        || target.mixed_structural_scalar_abi.is_some()
        || !abstracted.structural_parameters.is_empty()
        || !optimized.structural_parameters.is_empty()
        || !abstracted.entry_claims.is_empty()
        || !optimized.entry_claim_declarations.is_empty()
        || !optimized.entry_claims.is_empty()
        || !optimized.declared_places.is_empty()
        || !abstracted.published_service_ceiling.is_empty()
        || !optimized.published_service_ceiling.is_empty()
        || abstracted.block_entries.len() != 4
        || optimized.blocks.len() != 4
        || abstracted.operations.len() != 7
        || abstracted.parameters.len() != 2
        || optimized.parameters.len() != 2
        || optimized.entry != abstracted.entry
    {
        return Err(invalid());
    }
    // Compare the complete current graph, including authored but unused block
    // parameters. A target expression alone has already followed these edges.
    for (position, (entry, block)) in abstracted
        .block_entries
        .iter()
        .zip(&optimized.blocks)
        .enumerate()
    {
        let end = abstracted
            .block_entries
            .get(position + 1)
            .map_or(abstracted.operations.len(), |next| next.operation_offset);
        let operations = abstracted
            .operations
            .get(entry.operation_offset..end)
            .ok_or_else(invalid)?;
        if entry.block != block.id
            || entry.parameters.len() != block.parameters.len()
            || entry
                .parameters
                .iter()
                .zip(&block.parameters)
                .any(|(left, right)| {
                    left.value != right.value || left.scalar_type != right.scalar_type
                })
            || operations.len() != block.nodes.len()
            || operations
                .iter()
                .zip(&block.nodes)
                .any(|(left, right)| left != &right.operation)
        {
            return Err(Error::SourceCustodyMismatch);
        }
    }
    let entry = optimized
        .blocks
        .iter()
        .find(|block| block.id == optimized.entry)
        .ok_or_else(invalid)?;
    if entry.id != optimized.blocks[0].id || !entry.parameters.is_empty() || entry.nodes.len() != 2
    {
        return Err(invalid());
    }
    let condition = super::super::conditions::derive(function, target, abstracted, optimized)?;
    if condition.result_type != integer
        || condition.conditional_node_index != 1
        || !matches!(
            condition.legalized,
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
        condition: value,
        when_true,
        when_false,
    } = &branch.operation
    else {
        return Err(invalid());
    };
    if *value != condition.source
        || branch.successors.len() != 2
        || !branch.provenance.is_empty()
        || !branch.fuel.is_empty()
        || when_true.psi_edge != condition.when_true.psi_edge
        || when_false.psi_edge != condition.when_false.psi_edge
        || when_true.target == when_false.target
    {
        return Err(invalid());
    }
    for (successor, retained) in [when_true, when_false].into_iter().zip(&branch.successors) {
        if successor.psi_edge != retained.psi_edge
            || successor.target != retained.target
            || successor.bindings != retained.bindings
            || !successor.trivial_affine_discards.is_empty()
            || !retained.trivial_affine_discards.is_empty()
            || retained.provenance != [PsiProvenance::Edge(successor.psi_edge)]
        {
            return Err(invalid());
        }
    }
    let return_block = optimized
        .blocks
        .iter()
        .find(|block| {
            block.id != entry.id && block.id != when_true.target && block.id != when_false.target
        })
        .ok_or_else(invalid)?;
    let [parameter] = return_block.parameters.as_slice() else {
        return Err(invalid());
    };
    let [returned] = return_block.nodes.as_slice() else {
        return Err(invalid());
    };
    let AbstractOperation::Return {
        psi_edge: return_edge,
        result,
        value,
        scalar_type,
        cleanup_actions,
    } = &returned.operation
    else {
        return Err(invalid());
    };
    if parameter.scalar_type != scalar
        || *value != parameter.value
        || *scalar_type != scalar
        || !cleanup_actions.is_empty()
        || !returned.successors.is_empty()
        || returned.provenance != [PsiProvenance::Edge(*return_edge)]
    {
        return Err(invalid());
    }
    let abi = target
        .fixed_integer_scalar_abi
        .as_ref()
        .ok_or_else(invalid)?;
    if abi.result.scalar_type != integer || abi.result.value != *result || abi.parameters.len() != 2
    {
        return Err(invalid());
    }
    let call = evaluate_call_plan(
        CallingPolicy::native_for_target(native_target),
        &CallSignature {
            parameters: vec![calling_conventions::ValueShape::integer(8, 8); 2],
            result: Some(calling_conventions::ValueShape::integer(8, 8)),
        },
    )
    .map_err(|_| invalid())?;
    if abi.call_plan != call
        || Some(&abi.result.placement) != call.result.as_ref()
        || abi
            .parameters
            .iter()
            .zip(&call.parameters)
            .zip(&abstracted.parameters)
            .zip(&optimized.parameters)
            .any(|(((actual, placement), declared), checked)| {
                actual.placement != *placement
                    || actual.value != declared.value
                    || actual.value != checked.value
                    || ScalarType::Integer(actual.scalar_type) != declared.scalar_type
                    || declared.scalar_type != checked.scalar_type
            })
    {
        return Err(invalid());
    }
    let true_arm = arm(
        function,
        optimized,
        branch,
        when_true,
        condition.when_true.control.as_ref(),
        return_block.id,
        parameter.value,
        *return_edge,
    )?;
    let false_arm = arm(
        function,
        optimized,
        branch,
        when_false,
        condition.when_false.control.as_ref(),
        return_block.id,
        parameter.value,
        *return_edge,
    )?;
    let mut operations = condition.provenance_operations.clone();
    operations.extend([true_arm.operation, false_arm.operation]);
    let mut edges = abstracted
        .operations
        .iter()
        .flat_map(|operation| match operation {
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.psi_edge, when_false.psi_edge],
            AbstractOperation::Return { psi_edge, .. }
            | AbstractOperation::Jump { psi_edge, .. } => vec![*psi_edge],
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    edges.sort();
    let mut target_edges = target.provenance.edges.clone();
    target_edges.sort();
    if operations != target.provenance.operations || edges != target_edges {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(MatchedFunction {
        condition,
        entry,
        true_arm,
        false_arm,
        return_block,
        parameter,
        return_edge: *return_edge,
        returned,
    })
}
#[allow(clippy::too_many_arguments)]
fn arm<'a>(
    function: usize,
    optimized: &'a optimization_unit::PsiOptimizationFunction,
    branch: &'a optimization_unit::OptimizationNode,
    successor: &'a abstract_operations::AbstractSuccessor,
    target: &TargetIntegerControl,
    return_block: semantic_vocabulary::BlockId,
    return_parameter: ValueId,
    return_edge: EdgeId,
) -> Result<MatchedArm<'a>, LegalizationError> {
    let invalid = || Error::UnsupportedSourceShape { function };
    let block = optimized
        .blocks
        .iter()
        .find(|block| block.id == successor.target)
        .ok_or_else(invalid)?;
    let [constant, jump] = block.nodes.as_slice() else {
        return Err(invalid());
    };
    let AbstractOperation::IntegerConstant {
        psi_operation,
        result,
        scalar_type,
        value,
    } = &constant.operation
    else {
        return Err(invalid());
    };
    let AbstractOperation::Jump {
        psi_edge,
        target: destination,
        bindings,
        trivial_affine_discards,
    } = &jump.operation
    else {
        return Err(invalid());
    };
    let [binding] = bindings.as_slice() else {
        return Err(invalid());
    };
    let [definition] = constant.definitions.as_slice() else {
        return Err(invalid());
    };
    let [retained] = jump.successors.as_slice() else {
        return Err(invalid());
    };
    let TargetIntegerControl::Return {
        psi_return_edge,
        source_value,
        expression:
            TargetIntegerExpression::Immediate {
                value: target_value,
                source_value: expression_source,
            },
    } = target
    else {
        return Err(invalid());
    };
    if *destination != return_block
        || binding.parameter != return_parameter
        || binding.argument != *result
        || definition.value != *result
        || definition.scalar_type != binding.scalar_type
        || scalar_type != &binding.scalar_type
        || !trivial_affine_discards.is_empty()
        || !retained.trivial_affine_discards.is_empty()
        || retained.psi_edge != *psi_edge
        || retained.target != *destination
        || retained.bindings != *bindings
        || retained.provenance != [PsiProvenance::Edge(*psi_edge)]
        || !jump.provenance.is_empty()
        || !jump.fuel.is_empty()
        || !constant.successors.is_empty()
        || constant.provenance != [PsiProvenance::Operation(*psi_operation)]
        || *psi_return_edge != return_edge
        || *source_value != return_parameter
        || value != target_value
        || *expression_source != return_parameter
    {
        return Err(invalid());
    }
    Ok(MatchedArm {
        block,
        branch,
        successor,
        constant,
        jump,
        operation: *psi_operation,
        result: *result,
        value: *value,
        definition,
        edge: *psi_edge,
        binding,
    })
}

pub(in crate::legalization::source) struct MatchedFunction<'a> {
    pub condition: DerivedCondition<'a>,
    pub entry: &'a optimization_unit::OptimizationBlock,
    pub true_arm: MatchedArm<'a>,
    pub false_arm: MatchedArm<'a>,
    pub return_block: &'a optimization_unit::OptimizationBlock,
    pub parameter: &'a optimization_unit::ValueDefinition,
    pub return_edge: EdgeId,
    pub returned: &'a optimization_unit::OptimizationNode,
}
pub(in crate::legalization::source) struct MatchedArm<'a> {
    pub block: &'a optimization_unit::OptimizationBlock,
    pub branch: &'a optimization_unit::OptimizationNode,
    pub successor: &'a abstract_operations::AbstractSuccessor,
    pub constant: &'a optimization_unit::OptimizationNode,
    pub jump: &'a optimization_unit::OptimizationNode,
    pub operation: OperationId,
    pub result: ValueId,
    pub value: semantic_vocabulary::IntegerValue,
    pub definition: &'a optimization_unit::ValueDefinition,
    pub edge: EdgeId,
    pub binding: &'a abstract_operations::ValueBinding,
}
