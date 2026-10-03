//! Input-only custody for ordinary ordered scalar graphs. No legalized output is built here.
//!
//! Matching retains the source-derived block custody it already reconstructed.
//! Construction and replay consume that invocation's snapshots instead of
//! repeating dominance and loan reconstruction over unchanged inputs. Replay
//! still matches the source independently; no producer snapshot crosses into it.
use super::LegalizationError;
use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractOperationPlan,
    AbstractStructuralCallResult, IntegerShiftPolicy,
};
use calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, ValueLocation, ValuePlacement, ValueShape,
    evaluate_call_plan,
};
use legalized_operations::{SaturatingCarrier, TrappingForm, TrappingOperation};
use optimization_unit::{
    OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    ValueDefinitionSite,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, MachineId, ObligationId, OperationId, ScalarType, ValueId,
};
use target_operations::{TargetFunction, TargetOperationPlan};
pub(super) mod call_origin;
pub(in crate::legalization) mod cleanup_calls;
mod control;
mod custody;
pub(super) use custody::validate_unit_custody;
pub(super) mod address_joins;
pub(super) mod aggregate_results;
pub(super) mod atomic_access;
pub(super) mod borrowed_windows;
pub(super) mod byte_views;
mod header;
mod hosted_scalar;
pub(in crate::legalization) mod ieee_float_arithmetic;
pub(in crate::legalization) mod normalized_foreign;
mod place_declarations;
pub(super) mod read_byte;
pub(super) mod scalar_arrays;
pub(super) mod structural_case;
mod unobserved_owned;
pub(super) use hosted_scalar::hosted_realization;
mod literals;
pub(super) mod primitive_locals;
pub(super) mod reference_custody;
pub(super) mod structural_call;
pub(super) mod structural_fields;
fn structural_parameters(
    target: &TargetFunction,
) -> Option<&[target_operations::TargetStructuralParameter]> {
    if let Some(abi) = &target.mixed_structural_scalar_abi {
        Some(&abi.structural_parameters)
    } else {
        (!target.graph.parameters.is_empty()).then_some(target.graph.parameters.as_slice())
    }
}
pub(super) fn structural_contract(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Option<legalized_operations::LegalizedStructuralContract> {
    // Incoming parameter custody cannot be replaced by a local-only contract.
    // Provider metadata alone does not create runtime storage. Each actual
    // local declaration rejoins its own producer regardless of body composition.
    if let Some(parameters) = structural_parameters(target).or_else(|| {
        (abstracted.structural_parameters.is_empty()
            && optimized.structural_parameters.is_empty()
            && !optimized.declared_places.is_empty()
            && place_declarations::matches_producers(optimized))
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
    None
}
pub(in crate::legalization) mod indirect_calls;
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
        // The retained hosted address model uses a 64-bit integer-class slot,
        // while its semantic carrier remains an address throughout replay.
        ScalarType::Integer(integer) if integer.is_address() && integer.bits() == 64 => {
            Some(ValueShape::integer(8, 8))
        }
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
            Some(ValueShape::integer(
                u32::from(integer.bits() / 8),
                integer.bits() / 8,
            ))
        }
        _ => None,
    }
}

/// The saturating carrier a declared integer type is realized as: every
/// fixed 8/16/32/64-bit signed or unsigned width. Address carriers and other
/// widths report the unsupported family.
pub(super) fn saturating_carrier(integer: IntegerType) -> Option<SaturatingCarrier> {
    SaturatingCarrier::from_integer(integer)
}

/// The Trapping form a verified abstract Trapping operation realizes. The
/// result carrier names the form; a conversion adds only its source sign,
/// because normalization makes the source register the exact value whatever
/// its width; a shift count may have any fixed native type, because its
/// normalized register value read unsigned is below the width exactly when
/// the count lies in `0..width`. Binary arithmetic reads two values of the
/// result carrier. Address carriers and other widths have no Trapping form
/// and report the unsupported family.
pub(super) fn trapping_form(
    scalar_type: IntegerType,
    operand_type: IntegerType,
    primitive: terminal_psi::TrappingIntegerPrimitive,
) -> Option<TrappingForm> {
    use terminal_psi::TrappingIntegerPrimitive as P;
    let carrier = saturating_carrier(scalar_type)?;
    let source = saturating_carrier(operand_type)?;
    let operation = match primitive {
        P::Add | P::Subtract | P::Multiply | P::Divide | P::Remainder if source != carrier => {
            return None;
        }
        P::Add => TrappingOperation::Add,
        P::Subtract => TrappingOperation::Subtract,
        P::Multiply => TrappingOperation::Multiply,
        P::Divide => TrappingOperation::Divide,
        P::Remainder => TrappingOperation::Remainder,
        P::ShiftLeft => TrappingOperation::ShiftLeft,
        P::ShiftRight => TrappingOperation::ShiftRight,
        P::Convert => TrappingOperation::Convert {
            source: source.sign(),
        },
    };
    Some(TrappingForm { operation, carrier })
}

/// Whether every operand of a Trapping operation rejoins its declared type
/// in `optimized`: binary operands and a shifted value the result carrier,
/// a shift count and a conversion source the independent operand type.
pub(super) fn trapping_operands_rejoin(
    optimized: &PsiOptimizationFunction,
    scalar_type: IntegerType,
    operand_type: IntegerType,
    operation: terminal_psi::TrappingIntegerOperation,
) -> bool {
    use terminal_psi::TrappingIntegerOperation as T;
    let typed =
        |value, expected| value_type(optimized, value) == Some(ScalarType::Integer(expected));
    match operation {
        T::Add { left, right }
        | T::Subtract { left, right }
        | T::Multiply { left, right }
        | T::Divide { left, right }
        | T::Remainder { left, right } => {
            operand_type == scalar_type && typed(left, scalar_type) && typed(right, scalar_type)
        }
        T::ShiftLeft { value, count } | T::ShiftRight { value, count } => {
            typed(value, scalar_type) && typed(count, operand_type)
        }
        T::Convert { operand } => typed(operand, operand_type),
    }
}

/// Wrapping divide and remainder admit every fixed 8/16/32/64-bit carrier of
/// either sign. Scalar transport keeps narrow operands sign- or
/// zero-normalized in 64-bit registers, where the i64 division of two
/// normalized narrow values cannot fault and is exact; only a signed narrow
/// MIN / -1 quotient leaves its carrier, and selection's carrier
/// normalization truncates it back to the wrapped MIN. The i64 carrier's own
/// MIN / -1 is guarded by the wrapping i64 realization, and u64 takes the
/// unsigned division whose quotient and remainder never leave the carrier.
/// Division by zero stays the accepted obligation in every case. An address
/// shares the 64-bit integer-class transport but is not a count, so it has no
/// wrapping division; the carrier test keeps that transport from admitting it.
pub(super) fn supports_wrapping_division(integer: IntegerType) -> bool {
    integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
        && scalar_shape(ScalarType::Integer(integer)).is_some()
}

/// A proof-bearing exact cast between any two fixed 8/16/32/64-bit carriers.
/// Scalar transport holds every narrow integer full-register normalized, and
/// selection normalizes the result by the narrower carrier
/// (`selection::construction::scalar_graph::integer_conversion`): a widening
/// keeps the source's sign or zero bits, and a narrowing is proven inside the
/// target, whose own normalization is then exact.
pub(crate) fn exact_cast_has_native_carriers(source: IntegerType, target: IntegerType) -> bool {
    scalar_shape(ScalarType::Integer(source)).is_some()
        && scalar_shape(ScalarType::Integer(target)).is_some()
        && source.can_exact_cast_to(target)
}

#[cfg(test)]
mod admission_vocabulary_tests;

pub(super) fn i32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).expect("I32")
}
pub(super) fn u64_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).expect("U64")
}

pub(super) type BlockEntryCustody =
    std::collections::BTreeMap<semantic_vocabulary::BlockId, reference_custody::Custody>;

/// Input facts retained only for the immediately following construction or
/// replay. These snapshots describe the source, not a proposed legalized plan.
#[derive(Debug)]
pub(super) struct MatchedScalarGraph {
    pub(super) call_plan: CallPlan,
    pub(super) custody_entries: BlockEntryCustody,
}

pub(super) fn match_input(
    target: &TargetFunction,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<MatchedScalarGraph, LegalizationError> {
    primitive_locals::validate(optimized, &unit.structural_types)?;
    let call_plan = if aggregate_results::uses(optimized, plan) {
        aggregate_results::header(target, abstracted, optimized, native.target, plan)?
    } else if structural_parameters(target).is_some() {
        byte_views::validate(target, abstracted, optimized, native.target, plan)?
    } else {
        function_abi(native.target, target, abstracted, optimized)?
    };
    header::validate_descriptor_parameters(native.target, target, abstracted, &call_plan)?;
    // Ordered target graph replay rejoins every boundary row to its admitted
    // execution and exact source occurrence, independently of the result ABI.
    if optimized.blocks.is_empty()
        || optimized.entry != abstracted.entry
        || abstracted.block_entries.len() != optimized.blocks.len()
        || abstracted.block_entries[0].operation_offset != 0
    {
        return Err(LegalizationError::custody());
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
            .ok_or(LegalizationError::custody())?;
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
            return Err(LegalizationError::custody());
        }
        nodes::validate(block, optimized, &plan.structural_types)?;
    }
    let entry = optimized
        .blocks
        .iter()
        .find(|block| block.id == optimized.entry)
        .ok_or(LegalizationError::custody())?;
    if !entry.parameters.is_empty() || !entry.structural_parameters.is_empty() {
        return Err(LegalizationError::custody());
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
                && !(Some(placement.shape) == scalar_shape(parameter.scalar_type)
                    && (matches!(placement.locations.as_slice(), [ValueLocation::Register {value_byte_offset:0,byte_size,..}] if *byte_size == placement.shape.byte_size)
                        || scalar_stack(placement)))
        })
    {
        return Err(LegalizationError::custody());
    }
    // Ordered source calls execute even when their result is not returned.
    // Target expression trees witness referenced values, not execution order.
    for node in optimized.blocks.iter().flat_map(|block| &block.nodes) {
        if let AbstractOperation::EstablishByteSequenceLiteral {
            structural_type, ..
        }
        | AbstractOperation::EstablishTrivialAffineLocal {
            structural_type, ..
        } = &node.operation
            && (!plan.structural_types.contains(structural_type)
                || !unit.structural_types.contains(structural_type))
        {
            return Err(LegalizationError::custody());
        }
        if let AbstractOperation::EstablishZeroedStructural { result, .. }
        | AbstractOperation::EstablishStructuralLocal { result, .. } = &node.operation
            && (!plan
                .structural_types
                .iter()
                .any(|declaration| declaration.id == result.structural_type)
                || !unit
                    .structural_types
                    .iter()
                    .any(|declaration| declaration.id == result.structural_type))
        {
            return Err(LegalizationError::custody());
        }
        if let AbstractOperation::EstablishElementView {
            result, element, ..
        } = &node.operation
            && (!unit.structural_types.iter().any(|declaration| {
                declaration.id == result.structural_type
                    && match &declaration.shape {
                        terminal_psi::StructuralTypeShape::ElementView { element: declared } => {
                            declared == element
                        }
                        // A `&[u8]` result declares its view over the `u8`
                        // element instead of an `ElementView` shell.
                        terminal_psi::StructuralTypeShape::ByteSequence(_) => {
                            declaration.shape.is_borrowed_byte_view()
                                && unit.structural_types.iter().any(|element_declaration| {
                                    element_declaration.id == *element
                                        && matches!(
                                            element_declaration.shape,
                                            terminal_psi::StructuralTypeShape::PrimitiveScalar(
                                                semantic_vocabulary::ScalarType::Integer(integer)
                                            ) if Ok(integer)
                                                == semantic_vocabulary::IntegerType::new(
                                                    semantic_vocabulary::IntegerSign::Unsigned,
                                                    8,
                                                )
                                        )
                                })
                        }
                        _ => false,
                    }
            }) || !plan
                .structural_types
                .iter()
                .any(|declaration| declaration.id == result.structural_type)
                || !unit
                    .structural_types
                    .iter()
                    .any(|declaration| declaration.id == *element))
        {
            return Err(LegalizationError::custody());
        }
        if let AbstractOperation::StructuralLeafCopy { result, .. }
        | AbstractOperation::MoveStructuralField { result, .. } = &node.operation
            && (!plan
                .structural_types
                .iter()
                .any(|declaration| declaration.id == result.structural_type)
                || !unit
                    .structural_types
                    .iter()
                    .any(|declaration| declaration.id == result.structural_type))
        {
            return Err(LegalizationError::custody());
        }
        let obligation_custody_missing = |psi_operation: OperationId, obligation: ObligationId| {
            !unit.accepted_obligation_facts.iter().any(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == psi_operation
                    && fact.obligation == obligation
            }) || !optimized.facts.iter().any(|fact| {
                matches!(fact,
                    optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                    if *referenced == obligation && *support == psi_operation)
            })
        };
        // Every obligation-bearing integer operation keeps accepted custody.
        let integer_obligation = match &node.operation {
            AbstractOperation::IntegerShift {
                psi_operation,
                policy: IntegerShiftPolicy::Exact { obligation },
                ..
            } => Some((*psi_operation, *obligation)),
            AbstractOperation::IntegerArithmetic {
                psi_operation,
                arithmetic,
                ..
            } => arithmetic
                .obligation()
                .map(|obligation| (*psi_operation, obligation)),
            _ => None,
        };
        if let Some((psi_operation, obligation)) = integer_obligation
            && obligation_custody_missing(psi_operation, obligation)
        {
            return Err(LegalizationError::custody());
        }
        if let AbstractOperation::ByteSequenceWrite {
            psi_operation,
            obligation,
            ..
        }
        | AbstractOperation::StructuralByteSequenceFieldByteStore {
            psi_operation,
            obligation,
            ..
        }
        | AbstractOperation::StructuralByteSequenceFieldRead {
            psi_operation,
            obligation,
            ..
        }
        | AbstractOperation::EstablishByteSequenceConcatenation {
            psi_operation,
            capacity_obligation: obligation,
            ..
        }
        | AbstractOperation::ByteSequenceStore {
            psi_operation,
            obligation,
            ..
        }
        | AbstractOperation::StructuralByteSequenceFieldStore {
            psi_operation,
            obligation,
            ..
        }
        | AbstractOperation::ByteSequenceRead {
            psi_operation,
            obligation,
            ..
        }
        | AbstractOperation::ByteSequenceSubslice {
            psi_operation,
            obligation,
            ..
        }
        | AbstractOperation::ElementViewRead {
            psi_operation,
            obligation,
            ..
        }
        | AbstractOperation::ElementViewWrite {
            psi_operation,
            obligation,
            ..
        }
        | AbstractOperation::ElementViewSubslice {
            psi_operation,
            obligation,
            ..
        } = &node.operation
            && obligation_custody_missing(*psi_operation, *obligation)
        {
            return Err(LegalizationError::custody());
        }
        if let AbstractOperation::Call {
            callee, arguments, ..
        } = &node.operation
        {
            let call = callee_plan(*callee, native, plan, unit)?;
            if call.result.is_none()
                || call.parameters.len() != arguments.len()
                || !call
                    .parameters
                    .iter()
                    .all(|placement| scalar_register(placement) || scalar_stack(placement))
            {
                return Err(LegalizationError::custody());
            }
        }
        if let AbstractOperation::CallStructural {
            psi_operation,
            callee,
            arguments,
            structural_arguments,
            result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
            ..
        } = &node.operation
        {
            let call = callee_plan(*callee, native, plan, unit)?;
            let called = unit
                .functions
                .iter()
                .find(|function| function.machine == *callee)
                .ok_or(LegalizationError::custody())?;
            if call.result.is_some()
                != matches!(
                    node.operation,
                    AbstractOperation::CallStructural {
                        result: AbstractStructuralCallResult::Scalar(_),
                        ..
                    }
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
                return Err(LegalizationError::custody());
            }
            for (position, argument) in structural_arguments.iter().enumerate() {
                // Every reference crossing needs the ordered live frontier,
                // regardless of its position in the path. The target replay
                // below reconstructs it per node; an empty-custody preflight
                // cannot validate an interior descriptor or its final suffix.
                if argument
                    .path
                    .contains(&terminal_psi::StructuralPathSegment::Referent)
                {
                    continue;
                }
                structural_call::argument_at(
                    argument,
                    position,
                    *psi_operation,
                    optimized,
                    called,
                    &call,
                    native,
                    plan,
                    &reference_custody::Custody::default(),
                )?;
            }
        }
    }
    // Complete-unit custody admits cycles only after independent source proof
    // replay. Per-function matching below retains every executable edge.
    let custody_entries = validate_target(target, abstracted, optimized, native, plan, unit)?;
    Ok(MatchedScalarGraph {
        call_plan,
        custody_entries,
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
        return Err(LegalizationError::custody());
    };
    if !aggregate_results::uses(optimized, plan)
        && (!matches!(abstracted.result, AbstractFunctionResult::Unit)
            && !matches!(abstracted.result, AbstractFunctionResult::Scalar(result) if scalar_shape(result.scalar_type).is_some())
            || abstracted
                .parameters
                .iter()
                .any(|parameter| scalar_shape(parameter.scalar_type).is_none()))
    {
        return Err(LegalizationError::custody());
    }
    let call_plan = if aggregate_results::uses(optimized, plan) {
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
                    && crate::structural_inputs::structural_reference_input::parameter_shape(parameter, &plan.structural_types) == Some(placement.shape)
                    && (placement.locations.iter().all(|location| matches!(location,
                        ValueLocation::Register { byte_size: 1..=8, .. }
                        | ValueLocation::Stack { byte_size: 1..=8, .. }))
                        || crate::structural_inputs::structural_unit_input::owned_indirect_pointer(parameter, placement).is_some()))
            || scalar_stack(placement)
            || crate::structural_inputs::structural_reference_input::stack_pointer_offset(placement).is_some()
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
        return Err(LegalizationError::custody());
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
