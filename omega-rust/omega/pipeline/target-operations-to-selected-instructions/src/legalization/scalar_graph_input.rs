//! Input-only custody for ordinary ordered scalar graphs. No legalized output is built here.
use super::LegalizationError;
use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
};
use calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, ValueLocation, ValuePlacement, ValueShape,
    evaluate_call_plan,
};
use optimization_unit::{
    OptimizationBlock, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiProvenance, ValueDefinition, ValueDefinitionSite,
};
use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType, ValueId};
use target_operations::{TargetFunction, TargetOperation, TargetOperationPlan};
mod header;
mod nodes;
mod target;
use header::function_abi;
pub(super) use nodes::instruction;
use target::validate_target;

pub(super) struct Input<'a> {
    pub target: &'a TargetFunction,
    pub optimized: &'a PsiOptimizationFunction,
    pub block: &'a OptimizationBlock,
    pub body: &'a [OptimizationNode],
    pub returned: &'a OptimizationNode,
    pub call_plan: CallPlan,
}
pub(super) fn u64_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).expect("U64")
}
pub(super) fn register(placement: &ValuePlacement) -> bool {
    placement.shape == ValueShape::integer(8, 8)
        && matches!(
            placement.locations.as_slice(),
            [ValueLocation::Register {
                value_byte_offset: 0,
                byte_size: 8,
                ..
            }]
        )
}
pub(super) fn match_input<'a>(
    target: &'a TargetFunction,
    abstracted: &'a AbstractFunction,
    optimized: &'a PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Input<'a>, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let call_plan = function_abi(native.target, target, abstracted, optimized)?;
    let [entry] = abstracted.block_entries.as_slice() else {
        return Err(invalid);
    };
    let [block] = optimized.blocks.as_slice() else {
        return Err(invalid);
    };
    let Some((returned, body)) = block.nodes.split_last() else {
        return Err(invalid);
    };
    if entry.block != abstracted.entry
        || optimized.entry != entry.block
        || block.id != entry.block
        || entry.operation_offset != 0
        || !entry.parameters.is_empty()
        || !block.parameters.is_empty()
        || abstracted.operations
            != block
                .nodes
                .iter()
                .map(|node| node.operation.clone())
                .collect::<Vec<_>>()
    {
        return Err(invalid);
    }
    nodes::validate(block, body, returned, abstracted, optimized)?;
    // Ordered source calls execute even when their result is not returned.
    // Target expression trees witness referenced values, not execution order.
    for node in body {
        if let AbstractOperation::Call {
            callee, arguments, ..
        } = &node.operation
        {
            let call = callee_plan(*callee, native, plan, unit)?;
            if call.parameters.len() != arguments.len() {
                return Err(invalid);
            }
        }
    }
    validate_target(target, abstracted, optimized, native, plan, unit)?;
    Ok(Input {
        target,
        optimized,
        block,
        body,
        returned,
        call_plan,
    })
}
pub(super) fn callee_plan(
    callee: MachineId,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<CallPlan, LegalizationError> {
    let targets = native
        .functions
        .iter()
        .filter(|function| function.machine == callee)
        .collect::<Vec<_>>();
    let abstracts = plan
        .functions
        .iter()
        .filter(|function| function.machine == callee)
        .collect::<Vec<_>>();
    let optimized = unit
        .functions
        .iter()
        .filter(|function| function.machine == callee)
        .collect::<Vec<_>>();
    let ([target], [abstracted], [optimized]) = (
        targets.as_slice(),
        abstracts.as_slice(),
        optimized.as_slice(),
    ) else {
        return Err(LegalizationError::SourceCustodyMismatch);
    };
    if target.attachment.is_some()
        || !matches!(abstracted.result, AbstractFunctionResult::Scalar(_))
    {
        return Err(LegalizationError::SourceCustodyMismatch);
    }
    function_abi(native.target, target, abstracted, optimized)
}
