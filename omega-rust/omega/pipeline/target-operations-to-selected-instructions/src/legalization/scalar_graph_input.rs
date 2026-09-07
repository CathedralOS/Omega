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
    OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    ValueDefinitionSite,
};
use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, ScalarType, ValueId};
use target_operations::{TargetFunction, TargetOperation, TargetOperationPlan};
mod boolean;
mod control;
mod custody;
pub(super) use custody::validate_unit_custody;
mod byte_views;
mod header;
mod ranked;
pub(super) fn structural_contract(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Option<legalized_operations::LegalizedStructuralContract> {
    if let Some(abi) = &target.mixed_structural_scalar_abi {
        return Some(legalized_operations::LegalizedStructuralContract {
            structural_types: plan.structural_types.clone(),
            parameters: abstracted
                .structural_parameters
                .iter()
                .zip(&abi.structural_parameters)
                .map(
                    |(semantic, target)| legalized_operations::LegalizedCallUnitParameter {
                        semantic: semantic.clone(),
                        target: target.clone(),
                    },
                )
                .collect(),
            structural_places: optimized.structural_places.clone(),
            entry_claims: abstracted.entry_claims.clone(),
            published_service_ceiling: abstracted.published_service_ceiling.clone(),
        });
    }
    ranked::structural_contract(target, abstracted, optimized)
}
mod nodes;
mod target;
use header::function_abi;
pub(super) use nodes::instruction;
use target::validate_target;

pub(super) fn u32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 32).expect("U32")
}
pub(super) fn u8_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 8).expect("U8")
}
pub(super) fn scalar_shape(scalar: ScalarType) -> Option<ValueShape> {
    match scalar {
        ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        ScalarType::Integer(integer) if integer == u64_type() || integer == i64_type() => {
            Some(ValueShape::integer(8, 8))
        }
        _ => None,
    }
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
pub(super) fn match_input(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<CallPlan, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let ranked = matches!(target.operation, TargetOperation::RankedU32Countdown(_));
    let call_plan = if ranked {
        ranked::validate(target, abstracted, optimized, native, plan, unit)?
    } else if target.mixed_structural_scalar_abi.is_some() {
        byte_views::validate(target, abstracted, optimized, native.target, plan)?
    } else {
        function_abi(native.target, target, abstracted, optimized)?
    };
    if optimized.blocks.is_empty()
        || optimized.entry != abstracted.entry
        || abstracted.block_entries.len() != optimized.blocks.len()
        || abstracted.block_entries[0].operation_offset != 0
    {
        return Err(invalid);
    }
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
            .ok_or(invalid.clone())?;
        if entry.block != block.id
            || entry.parameters.len() != block.parameters.len()
            || entry
                .parameters
                .iter()
                .zip(&block.parameters)
                .any(|(declared, actual)| {
                    declared.value != actual.value || declared.scalar_type != actual.scalar_type
                })
            || operations.len() != block.nodes.len()
            || operations
                .iter()
                .zip(&block.nodes)
                .any(|(left, right)| left != &right.operation)
        {
            return Err(invalid);
        }
        nodes::validate(block, optimized, ranked)?;
    }
    let entry = optimized
        .blocks
        .iter()
        .find(|block| block.id == optimized.entry)
        .ok_or(invalid.clone())?;
    if !entry.parameters.is_empty() {
        return Err(invalid);
    }
    if optimized
        .parameters
        .iter()
        .zip(&call_plan.parameters)
        .any(|(parameter, placement)| {
            optimized
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .any(|node| node.uses.iter().any(|used| used.value == parameter.value))
                && !(Some(placement.shape) == (if ranked && parameter.scalar_type == ScalarType::Integer(u32_type()) { Some(ValueShape::integer(4, 4)) } else { scalar_shape(parameter.scalar_type) })
                    && matches!(placement.locations.as_slice(), [ValueLocation::Register {value_byte_offset:0,byte_size,..}] if *byte_size == placement.shape.byte_size))
        })
    {
        return Err(invalid);
    }
    // Ordered source calls execute even when their result is not returned.
    // Target expression trees witness referenced values, not execution order.
    for node in optimized.blocks.iter().flat_map(|block| &block.nodes) {
        if let AbstractOperation::ExactIntegerAdd {
            psi_operation,
            obligation,
            ..
        }
        | AbstractOperation::ExactIntegerSubtract {
            psi_operation,
            obligation,
            ..
        }
        | AbstractOperation::ByteSequenceRead {
            psi_operation,
            obligation,
            ..
        } = &node.operation
            && (!unit.accepted_obligation_facts.iter().any(|fact|
                fact.machine == optimized.machine && fact.operation == *psi_operation && fact.obligation == *obligation)
                || !optimized.facts.iter().any(|fact| matches!(fact,
                    optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                    if referenced == obligation && support == psi_operation)))
        {
            return Err(invalid);
        }
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
    if !ranked && !acyclic(optimized) {
        return Err(invalid);
    }
    if !ranked {
        validate_target(target, abstracted, optimized, native, plan, unit)?;
    }
    Ok(call_plan)
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
        || !matches!(abstracted.result, AbstractFunctionResult::Scalar(result) if result.scalar_type == ScalarType::Integer(u64_type()))
        || abstracted
            .parameters
            .iter()
            .any(|parameter| parameter.scalar_type != ScalarType::Integer(u64_type()))
    {
        return Err(LegalizationError::SourceCustodyMismatch);
    }
    let call_plan = function_abi(native.target, target, abstracted, optimized)?;
    if !call_plan.parameters.iter().all(register) {
        return Err(LegalizationError::SourceCustodyMismatch);
    }
    Ok(call_plan)
}
pub(super) fn i64_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 64).expect("I64")
}
pub(super) fn integer_type(scalar: ScalarType) -> Option<IntegerType> {
    match scalar {
        ScalarType::Integer(integer)
            if integer == u64_type() || integer == i64_type() || integer == u32_type() =>
        {
            Some(integer)
        }
        _ => None,
    }
}
pub(super) fn value_type(function: &PsiOptimizationFunction, value: ValueId) -> Option<ScalarType> {
    function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        }))
        .find(|definition| definition.value == value)
        .map(|definition| definition.scalar_type)
}
fn acyclic(function: &PsiOptimizationFunction) -> bool {
    let mut completed = Vec::new();
    loop {
        let before = completed.len();
        for block in &function.blocks {
            if !completed.contains(&block.id)
                && block
                    .nodes
                    .iter()
                    .flat_map(|node| &node.successors)
                    .all(|edge| completed.contains(&edge.target))
            {
                completed.push(block.id);
            }
        }
        if completed.len() == function.blocks.len() {
            return true;
        }
        if before == completed.len() {
            return false;
        }
    }
}
