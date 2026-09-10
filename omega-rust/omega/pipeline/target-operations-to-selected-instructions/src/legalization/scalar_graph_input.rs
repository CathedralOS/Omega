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
mod control;
mod custody;
pub(super) use custody::validate_unit_custody;
pub(super) mod aggregate_results;
mod byte_views;
mod header;
mod hosted_scalar;
pub(super) mod read_byte;
pub(super) mod scalar_arrays;
pub(super) mod structural_case;
mod unobserved_owned;
pub(super) use hosted_scalar::hosted_realization;
mod literals;
mod primitive_locals;
mod ranked;
pub(super) mod structural_call;
fn structural_parameters(
    target: &TargetFunction,
) -> Option<&[target_operations::TargetStructuralParameter]> {
    if let Some(abi) = &target.mixed_structural_scalar_abi {
        Some(&abi.structural_parameters)
    } else if let TargetOperation::UnitBody(body) = &target.operation {
        (!body.parameters.is_empty()).then_some(body.parameters.as_slice())
    } else if let TargetOperation::ControlGraph(graph) = &target.operation {
        (!graph.parameters.is_empty()).then_some(graph.parameters.as_slice())
    } else {
        None
    }
}
pub(super) fn structural_contract(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Option<legalized_operations::LegalizedStructuralContract> {
    // A matching place roster alone must not replace ranked parameter custody
    // with an empty local-only signature; the sum fallback needs an actual producer.
    if let Some(parameters) = structural_parameters(target).or_else(|| {
        (!optimized.structural_places.is_empty()
            && (literals::roster(optimized)
                || read_byte::roster(optimized)
                || primitive_locals::roster(optimized)
                || (aggregate_results::uses(optimized) && aggregate_results::roster(optimized))))
        .then_some(&[][..])
    }) {
        return Some(legalized_operations::LegalizedStructuralContract {
            structural_types: plan.structural_types.clone(),
            parameters: abstracted
                .structural_parameters
                .iter()
                .zip(parameters)
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
            result: abstracted.result.structural().cloned(),
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
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => {
            Some(ValueShape::float(4))
        }
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => {
            Some(ValueShape::float(8))
        }
        ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        ScalarType::Integer(integer)
            if integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
                && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
        {
            Some(ValueShape::integer(integer.bits() / 8, integer.bits() / 8))
        }
        _ => None,
    }
}

/// The admitted exact-cast slice excludes sub-64-bit signed-to-signed casts
/// and 16-bit-source widening.
pub(super) fn exact_cast_has_native_carriers(source: IntegerType, target: IntegerType) -> bool {
    scalar_shape(ScalarType::Integer(source)).is_some()
        && scalar_shape(ScalarType::Integer(target)).is_some()
        && source.can_exact_cast_to(target)
        && !(source.sign() == IntegerSign::Signed
            && target.sign() == IntegerSign::Signed
            && (source.bits() != 64 || target.bits() != 64))
        && !(source.bits() == 16 && target.bits() > 16)
}

#[cfg(test)]
mod exact_cast_carrier_tests;

pub(super) fn i32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).expect("I32")
}
pub(super) fn u64_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).expect("U64")
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
    primitive_locals::validate(optimized, &unit.structural_types)?;
    let ranked = matches!(target.operation, TargetOperation::RankedU32Countdown(_));
    let call_plan = if ranked {
        ranked::validate(target, abstracted, optimized, native, plan, unit)?
    } else if aggregate_results::uses(optimized) {
        aggregate_results::header(target, abstracted, optimized, native.target, plan)?
    } else if structural_parameters(target).is_some() {
        byte_views::validate(target, abstracted, optimized, native.target, plan)?
    } else {
        function_abi(native.target, target, abstracted, optimized)?
    };
    // Only the Unit-body target reader currently rejoins executable boundary
    // settlements. An unused scalar-tree row is not a builtin realization witness.
    if optimized
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .any(|node| matches!(node.operation, AbstractOperation::BoundaryCall { .. }))
        && (!matches!(
            target.operation,
            TargetOperation::UnitBody(_) | TargetOperation::ControlGraph(_)
        ) || !(abstracted.result == AbstractFunctionResult::Unit
            || abstracted.result.structural().is_some()))
    {
        return Err(invalid);
    }
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
            || entry.structural_parameters != block.structural_parameters
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
        nodes::validate(block, optimized, ranked, plan)?;
    }
    let entry = optimized
        .blocks
        .iter()
        .find(|block| block.id == optimized.entry)
        .ok_or(invalid.clone())?;
    if !entry.parameters.is_empty() || !entry.structural_parameters.is_empty() {
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
                    && (matches!(placement.locations.as_slice(), [ValueLocation::Register {value_byte_offset:0,byte_size,..}] if *byte_size == placement.shape.byte_size)
                        || !ranked
                            && (matches!(abstracted.result, AbstractFunctionResult::Unit | AbstractFunctionResult::Structural(_))
                                || (target.mixed_structural_scalar_abi.is_some()
                                    && matches!(target.operation, TargetOperation::ControlGraph(_))))
                            && scalar_stack(placement)))
        })
    {
        return Err(invalid);
    }
    // Ordered source calls execute even when their result is not returned.
    // Target expression trees witness referenced values, not execution order.
    for node in optimized.blocks.iter().flat_map(|block| &block.nodes) {
        if let AbstractOperation::EstablishByteSequenceLiteral {
            structural_type, ..
        } = &node.operation
            && (!plan.structural_types.contains(structural_type)
                || !unit.structural_types.contains(structural_type))
        {
            return Err(invalid);
        }
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
        | AbstractOperation::ByteSequenceWrite { psi_operation, obligation, .. }
        | AbstractOperation::ByteSequenceRead {
            psi_operation,
            obligation,
            ..
        }
        | AbstractOperation::ByteSequenceSubslice {
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
            if call.result.is_none()
                || call.parameters.len() != arguments.len()
                || !call.parameters.iter().all(scalar_register)
            {
                return Err(invalid);
            }
        }
        if let AbstractOperation::CallStructuralScalar {
            psi_operation,
            callee,
            arguments,
            structural_arguments,
            ..
        }
        | AbstractOperation::CallUnit {
            psi_operation,
            callee,
            arguments,
            structural_arguments,
            ..
        } = &node.operation
        {
            let call = callee_plan(*callee, native, plan, unit)?;
            if structural_arguments.len() > 1 {
                return Err(invalid);
            }
            let called = unit
                .functions
                .iter()
                .find(|function| function.machine == *callee)
                .ok_or(invalid.clone())?;
            if call.result.is_some()
                != matches!(
                    node.operation,
                    AbstractOperation::CallStructuralScalar { .. }
                )
                || call.parameters.len() != arguments.len() + structural_arguments.len()
                || called.parameters.len() != arguments.len()
                || called.structural_parameters.len() != structural_arguments.len()
                || arguments
                    .iter()
                    .zip(&called.parameters)
                    .zip(&call.parameters)
                    .any(|((value, parameter), placement)| {
                        value_type(optimized, *value) != Some(parameter.scalar_type)
                            || scalar_shape(parameter.scalar_type) != Some(placement.shape)
                            || !(scalar_register(placement) || scalar_stack(placement))
                    })
            {
                return Err(invalid);
            }
            for argument in structural_arguments {
                structural_call::argument(
                    argument,
                    *psi_operation,
                    optimized,
                    *callee,
                    native,
                    plan,
                    unit,
                )?;
            }
        }
    }
    // Complete-unit custody admits cycles only after independent source proof
    // replay. Per-function matching below retains every executable edge.
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
    if !aggregate_results::uses(optimized)
        && ((target.attachment.is_some()
            && !matches!(abstracted.result, AbstractFunctionResult::Unit))
            || !matches!(abstracted.result, AbstractFunctionResult::Unit)
                && !matches!(abstracted.result, AbstractFunctionResult::Scalar(result) if matches!(result.scalar_type, ScalarType::Boolean | ScalarType::Integer(_)) && scalar_shape(result.scalar_type).is_some())
            || abstracted.parameters.iter().any(|parameter| {
                if matches!(abstracted.result, AbstractFunctionResult::Unit) {
                    scalar_shape(parameter.scalar_type).is_none()
                } else {
                    !matches!(
                        parameter.scalar_type,
                        ScalarType::Integer(_) | ScalarType::Boolean
                    ) || scalar_shape(parameter.scalar_type).is_none()
                }
            }))
    {
        return Err(LegalizationError::SourceCustodyMismatch);
    }
    let call_plan = if aggregate_results::uses(optimized) {
        aggregate_results::header(target, abstracted, optimized, native.target, plan)?
    } else if structural_parameters(target).is_some() {
        byte_views::validate(target, abstracted, optimized, native.target, plan)?
    } else {
        function_abi(native.target, target, abstracted, optimized)?
    };
    if !call_plan.parameters.iter().enumerate().all(|(position, placement)| {
        scalar_register(placement)
            || position.checked_sub(abstracted.parameters.len())
                .and_then(|position| abstracted.structural_parameters.get(position))
                .is_some_and(|parameter| parameter.access == terminal_psi::StructuralAccess::Owned
                    && crate::structural_reference_input::parameter_shape(parameter, &plan.structural_types) == Some(placement.shape)
                    && placement.locations.iter().all(|location| matches!(location,
                        ValueLocation::Register { byte_size: 1 | 2 | 4 | 8, .. }
                        | ValueLocation::Stack { byte_size: 1 | 2 | 4 | 8, .. })))
            || scalar_stack(placement)
            || crate::structural_reference_input::stack_pointer_offset(placement).is_some()
            || placement.shape.class == calling_conventions::ValueClass::BorrowedReference
                && matches!(placement.locations.as_slice(),
                    [ValueLocation::Register { value_byte_offset: 0, byte_size: 8, .. }])
            || placement.shape.class == calling_conventions::ValueClass::BorrowedReference
                && matches!(
                    placement.locations.as_slice(),
                    [ValueLocation::Indirect {
                        pointer: calling_conventions::IndirectPointerLocation::Register(_),
                        copy_stack_byte_offset: None,
                        byte_size,
                        alignment,
                    }] if *byte_size == placement.shape.byte_size && *alignment == placement.shape.alignment
                )
    }) {
        return Err(LegalizationError::SourceCustodyMismatch);
    }
    Ok(call_plan)
}
fn scalar_register(placement: &ValuePlacement) -> bool {
    [
        ValueShape::float(4),
        ValueShape::float(8),
        ValueShape::integer(1, 1),
        ValueShape::integer(2, 2),
        ValueShape::integer(4, 4),
        ValueShape::integer(8, 8),
    ]
    .contains(&placement.shape)
        && matches!(placement.locations.as_slice(),
            [ValueLocation::Register {value_byte_offset: 0, byte_size, ..}]
            if *byte_size == placement.shape.byte_size)
}

fn scalar_stack(placement: &ValuePlacement) -> bool {
    [
        ValueShape::float(4),
        ValueShape::float(8),
        ValueShape::integer(1, 1),
        ValueShape::integer(2, 2),
        ValueShape::integer(4, 4),
        ValueShape::integer(8, 8),
    ]
    .contains(&placement.shape)
        && matches!(placement.locations.as_slice(),
            [ValueLocation::Stack { stack_byte_offset, value_byte_offset: 0, byte_size, alignment }]
                if *byte_size == placement.shape.byte_size && *alignment >= placement.shape.alignment
                    && alignment.is_power_of_two() && stack_byte_offset.is_multiple_of(u32::from(*alignment)))
}
pub(super) fn i64_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 64).expect("I64")
}
pub(super) fn integer_type(scalar: ScalarType) -> Option<IntegerType> {
    match scalar {
        ScalarType::Integer(integer)
            if integer == u64_type()
                || integer == i64_type()
                || integer == u32_type()
                || integer == i32_type() =>
        {
            Some(integer)
        }
        _ => None,
    }
}

/// Call results need defined full-register contents before scalar evaluation.
pub(super) fn integer_call_shape(scalar: ScalarType) -> Option<ValueShape> {
    match scalar {
        ScalarType::Integer(_) => scalar_shape(scalar),
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
