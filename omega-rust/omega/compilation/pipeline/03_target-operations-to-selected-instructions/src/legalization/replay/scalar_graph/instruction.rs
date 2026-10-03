//! Replays one legalized scalar instruction against the abstract operation
//! it legalizes: `validate` checks the short pairs here and hands the long
//! ones to `scalar_instructions`, `storage_instructions` and
//! `call_instructions`.

use super::{AbstractOperationPlan, Error, PsiOptimizationUnit, TargetOperationPlan};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use abstract_operations::{
    AbstractOperation, AbstractStructuralCallResult, IntegerArithmeticOperator,
    IntegerOverflowPolicy, IntegerShiftDirection, IntegerShiftPolicy,
};
use legalized_operations::{
    LegalizedOperationPlan, LegalizedScalarArgument, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, LegalizedValueDefinition, NativeCallOrigin,
};
use semantic_vocabulary::{IntegerValue, ScalarType};
mod aggregate_results;
mod call_instructions;
mod normalized_foreign;
mod scalar_instructions;
mod storage_instructions;
#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    actual: &LegalizedScalarInstruction,
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed_plan: &LegalizedOperationPlan,
    custody: &scalar_graph_input::reference_custody::Custody,
) -> Result<(), LegalizationError> {
    if let Some((operation, origin)) =
        scalar_graph_input::call_origin::installed_operation(node, optimized, native, plan)?
    {
        let LegalizedScalarInstructionKind::Call(call) = &actual.kind else {
            return Err(Error::NonCanonicalLegalizedPlan);
        };
        if call.source != origin {
            return Err(Error::NonCanonicalLegalizedPlan);
        }
        call.validate_source(&node.ownership)
            .map_err(|_| Error::NonCanonicalLegalizedPlan)?;
        let mut call_node = node.clone();
        call_node.operation = operation;
        let mut ordinary = actual.clone();
        let LegalizedScalarInstructionKind::Call(call) = &mut ordinary.kind else {
            return Err(Error::NonCanonicalLegalizedPlan);
        };
        call.source = NativeCallOrigin::Authored;
        return validate(
            &ordinary,
            &call_node,
            optimized,
            native,
            plan,
            unit,
            proposed_plan,
            custody,
        );
    }
    let invalid = Error::NonCanonicalLegalizedPlan;
    let (operation, result) = scalar_graph_input::instruction(node).ok_or(invalid.clone())?;
    if actual.operation != operation
        || actual.result
            != result.map(|value| LegalizedValueDefinition {
                value,
                scalar_type: node.definitions[0].scalar_type,
                definition_site: node.definitions[0].site,
            })
        || actual.fuel != node.fuel
        || actual.effect != node.effect
        || actual.ownership != node.ownership
    {
        return Err(invalid);
    }
    match (&actual.kind, &node.operation) {
        (
            LegalizedScalarInstructionKind::AtomicAccess {
                location,
                field,
                scalar_type,
                field_byte_offset,
                event,
            },
            AbstractOperation::AtomicEvent { event: source, .. },
        ) => {
            let expected = scalar_graph_input::atomic_access::access(
                source,
                optimized,
                &unit.structural_types,
            )
            .ok_or_else(|| invalid.clone())?;
            if location != &expected.location
                || *field != expected.field
                || *scalar_type != expected.scalar_type
                || *field_byte_offset != expected.field_byte_offset
                || *event != expected.event
            {
                return Err(invalid);
            }
        }
        (
            LegalizedScalarInstructionKind::WrappingAdd { left, right }
            | LegalizedScalarInstructionKind::WrappingSubtract { left, right }
            | LegalizedScalarInstructionKind::WrappingMultiply { left, right },
            AbstractOperation::IntegerArithmetic {
                left: source_left,
                right: source_right,
                scalar_type,
                arithmetic,
                ..
            },
        ) if legalized_arithmetic_pair(&actual.kind)
            == Some((arithmetic.policy(), arithmetic.operator()))
            && left == source_left
            && right == source_right
            && scalar_graph_input::scalar_shape(ScalarType::Integer(*scalar_type)).is_some()
            && [left, right].iter().all(|value| {
                scalar_graph_input::value_type(optimized, **value)
                    == Some(ScalarType::Integer(*scalar_type))
            }) => {}
        // The legalized kind must name the carrier the source operation
        // declares: a kind naming another width would clamp to the wrong
        // bounds while every register-level check still passes.
        (
            LegalizedScalarInstructionKind::SaturatingSubtract {
                carrier,
                left,
                right,
            }
            | LegalizedScalarInstructionKind::SaturatingAdd {
                carrier,
                left,
                right,
            }
            | LegalizedScalarInstructionKind::SaturatingMultiply {
                carrier,
                left,
                right,
            },
            AbstractOperation::IntegerArithmetic {
                left: source_left,
                right: source_right,
                scalar_type,
                arithmetic,
                ..
            },
        ) if legalized_arithmetic_pair(&actual.kind)
            == Some((arithmetic.policy(), arithmetic.operator()))
            && left == source_left
            && right == source_right
            && scalar_graph_input::saturating_carrier(*scalar_type) == Some(*carrier)
            && [left, right].iter().all(|value| {
                scalar_graph_input::value_type(optimized, **value)
                    == Some(ScalarType::Integer(*scalar_type))
            }) => {}
        // A Trapping kind must name exactly the form, the operands in
        // evaluation order, and for a conversion the source carrier the
        // abstract operation declares; there is no obligation to replay.
        (
            LegalizedScalarInstructionKind::TrappingBinary { form, left, right },
            AbstractOperation::TrappingInteger {
                scalar_type,
                operand_type,
                operation,
                ..
            },
        ) if !matches!(
            operation,
            terminal_psi::TrappingIntegerOperation::Convert { .. }
        ) && scalar_graph_input::trapping_form(
            *scalar_type,
            *operand_type,
            operation.primitive(),
        ) == Some(*form)
            && operation.operands() == [*left, *right]
            && scalar_graph_input::trapping_operands_rejoin(
                optimized,
                *scalar_type,
                *operand_type,
                *operation,
            ) => {}
        (
            LegalizedScalarInstructionKind::TrappingConvert {
                form,
                source,
                operand,
            },
            AbstractOperation::TrappingInteger {
                scalar_type,
                operand_type,
                operation:
                    operation @ terminal_psi::TrappingIntegerOperation::Convert {
                        operand: source_operand,
                    },
                ..
            },
        ) if scalar_graph_input::trapping_form(
            *scalar_type,
            *operand_type,
            operation.primitive(),
        ) == Some(*form)
            && scalar_graph_input::saturating_carrier(*operand_type) == Some(*source)
            && operand == source_operand
            && scalar_graph_input::trapping_operands_rejoin(
                optimized,
                *scalar_type,
                *operand_type,
                *operation,
            ) => {}
        (
            LegalizedScalarInstructionKind::SaturatingDivide {
                carrier,
                left,
                right,
                obligation,
                accepted_fact,
            },
            AbstractOperation::IntegerArithmetic {
                psi_operation,
                scalar_type,
                arithmetic,
                left: source_left,
                right: source_right,
                ..
            },
        ) if arithmetic.is(
            IntegerOverflowPolicy::Saturating,
            IntegerArithmeticOperator::Divide,
        ) =>
        {
            let Some(source_obligation) = arithmetic.obligation() else {
                return Err(invalid);
            };
            let source_obligation = &source_obligation;
            let mut facts = unit.accepted_obligation_facts.iter().filter(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *source_obligation
            });
            let fact = facts.next().ok_or(invalid.clone())?;
            if facts.next().is_some()
                || scalar_graph_input::saturating_carrier(*scalar_type) != Some(*carrier)
                || [source_left, source_right].iter().any(|value| {
                    scalar_graph_input::value_type(optimized, **value)
                        != Some(ScalarType::Integer(*scalar_type))
                })
                || left != source_left
                || right != source_right
                || obligation != source_obligation
                || *accepted_fact != fact.identity
                || !optimized.facts.iter().any(|fact| matches!(fact,
                    optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                    if referenced == source_obligation && support == psi_operation))
            {
                return Err(invalid);
            }
        }
        (
            LegalizedScalarInstructionKind::SaturatingRemainder {
                carrier,
                left,
                right,
                obligation,
                accepted_fact,
            },
            AbstractOperation::IntegerArithmetic {
                psi_operation,
                scalar_type,
                arithmetic,
                left: source_left,
                right: source_right,
                ..
            },
        ) if arithmetic.is(
            IntegerOverflowPolicy::Saturating,
            IntegerArithmeticOperator::Remainder,
        ) =>
        {
            let Some(source_obligation) = arithmetic.obligation() else {
                return Err(invalid);
            };
            let source_obligation = &source_obligation;
            let mut facts = unit.accepted_obligation_facts.iter().filter(|fact| {
                fact.machine == optimized.machine
                    && fact.operation == *psi_operation
                    && fact.obligation == *source_obligation
            });
            let fact = facts.next().ok_or(invalid.clone())?;
            if facts.next().is_some()
                || scalar_graph_input::saturating_carrier(*scalar_type) != Some(*carrier)
                || [source_left, source_right].iter().any(|value| {
                    scalar_graph_input::value_type(optimized, **value)
                        != Some(ScalarType::Integer(*scalar_type))
                })
                || left != source_left
                || right != source_right
                || obligation != source_obligation
                || *accepted_fact != fact.identity
                || !optimized.facts.iter().any(|fact| matches!(fact,
                    optimization_unit::OptimizationFact::OperationObligationReference { obligation: referenced, support }
                    if referenced == source_obligation && support == psi_operation))
            {
                return Err(invalid);
            }
        }
        (
            LegalizedScalarInstructionKind::StructuralScalarFieldRead { .. },
            AbstractOperation::StructuralScalarFieldRead { .. },
        )
        | (
            LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength { .. },
            AbstractOperation::StructuralByteSequenceFieldLength { .. },
        ) => storage_instructions::validate_structural_scalar_field_read(
            actual, node, optimized, plan, unit, custody,
        )?,
        (
            LegalizedScalarInstructionKind::BitwiseAnd { left, right },
            AbstractOperation::IntegerBitwiseAnd {
                left: source_left,
                right: source_right,
                ..
            },
        )
        | (
            LegalizedScalarInstructionKind::BitwiseOr { left, right },
            AbstractOperation::IntegerBitwiseOr {
                left: source_left,
                right: source_right,
                ..
            },
        )
        | (
            LegalizedScalarInstructionKind::BitwiseXor { left, right },
            AbstractOperation::IntegerBitwiseXor {
                left: source_left,
                right: source_right,
                ..
            },
        ) if left == source_left && right == source_right => {}
        (
            LegalizedScalarInstructionKind::BitwiseNot { operand },
            AbstractOperation::IntegerBitwiseNot {
                operand: source, ..
            },
        ) if operand == source => {}
        (
            LegalizedScalarInstructionKind::FloatResultPolicy { format, policy },
            AbstractOperation::FloatResultPolicy {
                format: expected_format,
                policy: expected_policy,
                ..
            },
        ) if format == expected_format && policy == expected_policy => {}
        (
            LegalizedScalarInstructionKind::IeeeFloatCompare {
                comparison,
                format,
                left,
                right,
            },
            AbstractOperation::IeeeFloatCompare {
                comparison: expected_comparison,
                format: expected_format,
                left: expected_left,
                right: expected_right,
                ..
            },
        ) if comparison == expected_comparison
            && format == expected_format
            && left == expected_left
            && right == expected_right => {}
        (
            LegalizedScalarInstructionKind::EstablishReference {
                result,
                source,
                shape,
            },
            AbstractOperation::EstablishReference {
                result: expected,
                source: expected_source,
                ..
            },
        ) if result == expected
            && source == expected_source
            && *shape
                == scalar_graph_input::aggregate_results::home_layout(result, plan)?.shape() => {}
        (
            LegalizedScalarInstructionKind::ReleaseReference { source },
            AbstractOperation::ReleaseReference {
                source: expected, ..
            },
        ) if source == expected => {}
        (
            _,
            AbstractOperation::CallStructural {
                result: AbstractStructuralCallResult::Structural { .. },
                ..
            }
            | AbstractOperation::EstablishRecord { .. }
            | AbstractOperation::EstablishScalarArray { .. }
            | AbstractOperation::EstablishCase { .. }
            | AbstractOperation::StructuralCaseMembership { .. }
            | AbstractOperation::StructuralLeafCopy { .. },
        ) => {
            aggregate_results::validate(actual, node, optimized, native, plan, unit, custody)?;
        }
        (
            LegalizedScalarInstructionKind::EstablishZeroedStructural { result, shape },
            AbstractOperation::EstablishZeroedStructural {
                result: expected, ..
            },
        ) if result == expected
            && *shape
                == scalar_graph_input::aggregate_results::home_layout(result, plan)?.shape() => {}
        (
            LegalizedScalarInstructionKind::EstablishStructuralLocal { result, shape },
            AbstractOperation::EstablishStructuralLocal {
                result: expected, ..
            },
        ) if result == expected
            && *shape
                == scalar_graph_input::aggregate_results::home_layout(result, plan)?.shape() => {}
        // A window move replays as the leaf-copy byte copy of the vacated
        // field and its store as the inverse copy; both re-resolve the field
        // extent from the abstract row.
        (
            LegalizedScalarInstructionKind::StructuralLeafCopy {
                result,
                source,
                path,
                byte_offset,
                shape,
                indices,
            },
            AbstractOperation::MoveStructuralField {
                result: expected,
                source: root,
                path: spelled,
                field,
                ..
            },
        ) if result == expected && *source == root.place() && {
            let extent = scalar_graph_input::borrowed_windows::moved(
                optimized, root, spelled, *field, expected, plan, custody,
            )?;
            *path == abstract_operations::StructuralCopyPath::Ordinary(extent.path)
                && (*byte_offset, *shape) == (extent.byte_offset, extent.shape)
                && indices_rejoin(indices, &extent.elements)
        } => {}
        (
            LegalizedScalarInstructionKind::StoreStructuralField {
                destination,
                path,
                field,
                value,
                byte_offset,
                shape,
                indices,
            },
            AbstractOperation::StoreStructuralField {
                destination: expected_destination,
                path: expected_path,
                field: expected_field,
                value: expected_value,
                ..
            },
        ) if destination == expected_destination
            && path == expected_path
            && field == expected_field
            && *value == expected_value.place
            && expected_value.access == terminal_psi::StructuralAccess::Owned
            && expected_value.path.is_empty()
            && {
                let extent = scalar_graph_input::borrowed_windows::extent(
                    optimized,
                    expected_destination,
                    expected_path,
                    *expected_field,
                    plan,
                    custody,
                )?;
                (*byte_offset, *shape) == (extent.byte_offset, extent.shape)
                    && indices_rejoin(indices, &extent.elements)
            } => {}
        (
            LegalizedScalarInstructionKind::EstablishTrivialAffineLocal {
                place,
                structural_type,
            },
            AbstractOperation::EstablishTrivialAffineLocal {
                place: expected_place,
                structural_type: expected_type,
                ..
            },
        ) if place == expected_place && structural_type == expected_type => {}
        (
            LegalizedScalarInstructionKind::EstablishPrimitiveLocal {
                result,
                value,
                shape,
            },
            AbstractOperation::EstablishPrimitiveLocal {
                result: expected,
                value: expected_value,
                ..
            },
        ) if result == expected
            && value == expected_value
            && scalar_graph_input::scalar_shape(value.scalar_type) == Some(*shape) => {}
        (
            LegalizedScalarInstructionKind::PrimitiveLocalStore { destination, value },
            AbstractOperation::PrimitiveLocalStore {
                destination: expected,
                value: expected_value,
                ..
            },
        ) if destination == expected && value == expected_value => {}
        (
            LegalizedScalarInstructionKind::PrimitiveScalarRead { .. },
            AbstractOperation::PrimitiveScalarRead { .. },
        ) => storage_instructions::validate_primitive_scalar_read(
            actual, node, optimized, unit, operation,
        )?,
        (
            LegalizedScalarInstructionKind::HostedReadByte {
                boundary,
                result,
                layout,
            },
            AbstractOperation::BoundaryCall {
                boundary: expected_boundary,
                result: abstract_operations::AbstractBoundaryResult::Structural(expected_result),
                ..
            },
        ) if actual.has_valid_hosted_read_byte_shape()
            && boundary == expected_boundary
            && result == expected_result
            && *layout == scalar_graph_input::read_byte::layout(expected_result, plan)?
            && matches!(
                scalar_graph_input::hosted_realization(native, optimized.machine, operation)?,
                target_operations::BoundaryRealization::HostedReadByte(_)
            ) => {}
        (
            LegalizedScalarInstructionKind::HostedWriteByteI32 { boundary, source },
            AbstractOperation::BoundaryCall {
                boundary: expected,
                arguments,
                ..
            },
        ) if boundary == expected
            && arguments.as_slice() == [*source]
            && matches!(
                scalar_graph_input::hosted_realization(native, optimized.machine, operation)?,
                target_operations::BoundaryRealization::HostedWriteByteI32(_)
            ) => {}
        (
            LegalizedScalarInstructionKind::NearestIeeeFloatArithmetic {
                boundary,
                operation: arithmetic,
                format,
                arguments: actual_arguments,
            },
            AbstractOperation::BoundaryCall {
                boundary: expected,
                arguments,
                ..
            },
        ) if boundary == expected
            && arguments == actual_arguments
            && matches!(
                scalar_graph_input::ieee_float_arithmetic::row(native, optimized.machine, operation)?,
                Some(target_operations::TargetUnitOperation::NearestIeeeFloatArithmetic {
                    boundary: row_boundary,
                    realization,
                    ..
                }) if row_boundary == boundary
                    && realization.operation == *arithmetic
                    && realization.format == *format
            ) => {}
        (
            LegalizedScalarInstructionKind::HostedExitProcessI32 { boundary, source },
            AbstractOperation::BoundaryCall {
                boundary: expected,
                arguments,
                ..
            },
        ) if boundary == expected
            && arguments.as_slice() == [*source]
            && matches!(
                scalar_graph_input::hosted_realization(native, optimized.machine, operation)?,
                target_operations::BoundaryRealization::HostedExitProcessI32(_)
            ) => {}
        (
            LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { .. },
            AbstractOperation::WriteOnlyPrimitiveStore { .. },
        ) => storage_instructions::validate_write_only_primitive_store(
            actual, node, optimized, unit, operation,
        )?,
        (
            LegalizedScalarInstructionKind::StructuralScalarFieldStore { .. },
            AbstractOperation::StructuralScalarFieldStore { .. },
        ) => storage_instructions::validate_structural_scalar_field_store(
            actual, node, optimized, unit, operation, custody,
        )?,
        (
            LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                destination,
                structural_type,
                bytes,
            },
            AbstractOperation::EstablishByteSequenceLiteral {
                place,
                structural_type: expected_type,
                bytes: expected_bytes,
                ..
            },
        ) => {
            if destination != place || structural_type != expected_type || bytes != expected_bytes {
                return Err(invalid);
            }
        }
        (
            LegalizedScalarInstructionKind::ByteSequenceSubslice { .. },
            AbstractOperation::ByteSequenceSubslice { .. },
        ) => storage_instructions::validate_byte_sequence_subslice(
            actual, node, optimized, unit, operation,
        )?,
        (
            LegalizedScalarInstructionKind::ElementViewSubslice { .. },
            AbstractOperation::ElementViewSubslice { .. },
        ) => storage_instructions::validate_element_view_subslice(
            actual, node, optimized, unit, operation,
        )?,
        (
            LegalizedScalarInstructionKind::EstablishElementView {
                result,
                destination,
                source,
                element,
            },
            AbstractOperation::EstablishElementView {
                result: expected_result,
                destination: expected_destination,
                source: expected_source,
                element: expected_element,
                ..
            },
        ) => {
            if result != expected_result
                || destination != expected_destination
                || source != expected_source
                || element != expected_element
            {
                return Err(invalid);
            }
        }
        (
            LegalizedScalarInstructionKind::ElementViewRead { .. },
            AbstractOperation::ElementViewRead { .. },
        ) => storage_instructions::validate_element_view_read(
            actual, node, optimized, unit, operation,
        )?,
        (
            LegalizedScalarInstructionKind::ElementViewLength {
                source,
                length_byte_offset: 8,
            },
            AbstractOperation::ElementViewLength {
                source: expected, ..
            },
        ) if source == expected => {}
        (
            LegalizedScalarInstructionKind::Call(_),
            AbstractOperation::CallStructural {
                result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
                ..
            },
        ) => call_instructions::validate_call(
            actual,
            node,
            optimized,
            native,
            plan,
            unit,
            proposed_plan,
            operation,
            custody,
        )?,
        (
            LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore { .. },
            AbstractOperation::StructuralByteSequenceFieldByteStore { .. },
        ) => storage_instructions::validate_structural_byte_sequence_field_byte_store(
            actual, node, optimized, plan, unit, operation, custody,
        )?,
        (
            LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead { .. },
            AbstractOperation::StructuralByteSequenceFieldRead { .. },
        ) => storage_instructions::validate_structural_byte_sequence_field_read(
            actual, node, optimized, plan, unit, operation,
        )?,
        (
            LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation { .. },
            AbstractOperation::EstablishByteSequenceConcatenation { .. },
        ) => storage_instructions::validate_byte_sequence_concatenation(
            actual, node, optimized, unit,
        )?,
        (
            LegalizedScalarInstructionKind::ByteSequenceStore { .. },
            AbstractOperation::ByteSequenceStore { .. },
        ) => storage_instructions::validate_byte_sequence_store(
            actual, node, optimized, plan, unit, operation, custody,
        )?,
        (
            LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore { .. },
            AbstractOperation::StructuralByteSequenceFieldStore { .. },
        ) => storage_instructions::validate_structural_byte_sequence_field_store(
            actual, node, optimized, plan, unit, operation, custody,
        )?,
        (
            LegalizedScalarInstructionKind::StructuralByteViewFieldStore {
                indices,
                destination,
                field,
                source,
            },
            AbstractOperation::StructuralByteViewFieldStore {
                field: expected_field,
                source: expected_source,
                ..
            },
        ) => {
            if *indices
                != scalar_graph_input::structural_fields::byte_index_operands(
                    optimized,
                    unit,
                    operation,
                    destination,
                    *field,
                )?
            {
                return Err(Error::NonCanonicalLegalizedPlan);
            }
            if Some(destination.clone())
                != scalar_graph_input::structural_fields::view_replacement(
                    optimized,
                    &node.operation,
                    &plan.structural_types,
                    custody,
                )
                || field != expected_field
                || source != expected_source
            {
                return Err(Error::NonCanonicalLegalizedPlan);
            }
        }
        (
            LegalizedScalarInstructionKind::ElementViewWrite { .. },
            AbstractOperation::ElementViewWrite { .. },
        ) => storage_instructions::validate_element_view_write(
            actual, node, optimized, unit, operation,
        )?,
        (
            LegalizedScalarInstructionKind::ByteSequenceWrite { .. },
            AbstractOperation::ByteSequenceWrite { .. },
        ) => storage_instructions::validate_byte_sequence_write(
            actual, node, optimized, unit, operation,
        )?,
        (
            LegalizedScalarInstructionKind::ByteSequenceRead { .. },
            AbstractOperation::ByteSequenceRead { .. },
        ) => storage_instructions::validate_byte_sequence_read(
            actual, node, optimized, unit, operation,
        )?,
        (
            LegalizedScalarInstructionKind::ByteSequencePredicate { operand, predicate },
            AbstractOperation::ByteSequencePredicate {
                operand: expected_operand,
                predicate: expected_predicate,
                ..
            },
        ) if operand == expected_operand && predicate == expected_predicate => {}
        (
            LegalizedScalarInstructionKind::ByteSequenceEqual { left, right },
            AbstractOperation::ByteSequenceEqual {
                left: expected_left,
                right: expected_right,
                ..
            },
        ) if left == expected_left && right == expected_right => {}
        (
            LegalizedScalarInstructionKind::ByteSequenceLength {
                source,
                length_byte_offset,
            },
            AbstractOperation::ByteSequenceLength {
                source: expected, ..
            },
        ) if source == expected
            && *length_byte_offset
                == if scalar_graph_input::byte_views::reads_owner_layout(
                    optimized,
                    *source,
                    &unit.structural_types,
                ) {
                    0
                } else {
                    8
                } => {}
        (
            LegalizedScalarInstructionKind::BooleanNot { operand },
            AbstractOperation::BooleanNot {
                operand: source, ..
            },
        ) if operand == source => {}
        (
            LegalizedScalarInstructionKind::IntegerWiden {
                operand,
                source_type,
            },
            AbstractOperation::IntegerWiden {
                operand: source,
                source_type: source_integer,
                ..
            },
        ) if operand == source && source_type == source_integer => {}
        (
            LegalizedScalarInstructionKind::ScalarRepresentationView {
                operand,
                source_type,
            },
            AbstractOperation::ScalarRepresentationView {
                operand: source,
                source_type: source_carrier,
                target_type,
                ..
            },
        ) if operand == source
            && source_type == source_carrier
            && terminal_semantics::ScalarRepresentationViewPair::classify(
                *source_type,
                *target_type,
            )
            .is_some() => {}
        (
            LegalizedScalarInstructionKind::Constant(actual),
            AbstractOperation::IntegerConstant { value, .. },
        ) if actual == value => {}
        (
            LegalizedScalarInstructionKind::IntegerExactCast { .. },
            AbstractOperation::IntegerExactCast { .. },
        ) => scalar_instructions::validate_integer_exact_cast(actual, node, optimized, unit)?,
        (
            LegalizedScalarInstructionKind::Constant(actual),
            AbstractOperation::IeeeFloatConstant { value, .. },
        ) if *actual
            == IntegerValue::Unsigned(match value {
                semantic_vocabulary::IeeeFloatValue::Binary32(bits) => u128::from(*bits),
                semantic_vocabulary::IeeeFloatValue::Binary64(bits) => u128::from(*bits),
            }) => {}
        (
            LegalizedScalarInstructionKind::Constant(actual),
            AbstractOperation::BooleanConstant { value, .. },
        ) if *actual == IntegerValue::Unsigned(u128::from(*value)) => {}
        (LegalizedScalarInstructionKind::Call(_), AbstractOperation::Call { .. }) => {
            call_instructions::validate_call_call(actual, node, native, plan, unit, proposed_plan)?
        }
        (
            LegalizedScalarInstructionKind::NormalizedForeignCall(_),
            AbstractOperation::BoundaryCall { .. },
        ) => normalized_foreign::validate(actual, node, optimized, native, plan, unit, operation)?,
        (
            LegalizedScalarInstructionKind::DynamicParameterCall(_),
            AbstractOperation::CallDynamic {
                dispatch: abstract_operations::AbstractDynamicDispatch::Parameter(_),
                ..
            },
        ) => call_instructions::validate_dynamic_parameter_call(
            actual, node, optimized, native, operation,
        )?,
        (
            LegalizedScalarInstructionKind::WrappingShiftLeft { .. },
            AbstractOperation::IntegerShift {
                policy: IntegerShiftPolicy::Wrapping,
                direction: IntegerShiftDirection::Left,
                ..
            },
        )
        | (
            LegalizedScalarInstructionKind::WrappingShiftRight { .. },
            AbstractOperation::IntegerShift {
                policy: IntegerShiftPolicy::Wrapping,
                direction: IntegerShiftDirection::Right,
                ..
            },
        )
        | (
            LegalizedScalarInstructionKind::ExactShiftLeft { .. },
            AbstractOperation::IntegerShift {
                policy: IntegerShiftPolicy::Exact { .. },
                direction: IntegerShiftDirection::Left,
                ..
            },
        )
        | (
            LegalizedScalarInstructionKind::ExactShiftRight { .. },
            AbstractOperation::IntegerShift {
                policy: IntegerShiftPolicy::Exact { .. },
                direction: IntegerShiftDirection::Right,
                ..
            },
        ) => scalar_instructions::validate_shift(actual, node, optimized, unit)?,
        (
            LegalizedScalarInstructionKind::WrappingRemainder { .. }
            | LegalizedScalarInstructionKind::WrappingDivide { .. },
            AbstractOperation::IntegerArithmetic { arithmetic, .. },
        ) if legalized_arithmetic_pair(&actual.kind)
            == Some((arithmetic.policy(), arithmetic.operator())) =>
        {
            scalar_instructions::validate_wrapping_division(actual, node, optimized, unit)?
        }
        (
            LegalizedScalarInstructionKind::ExactBinary { .. },
            AbstractOperation::IntegerArithmetic { arithmetic, .. },
        ) if arithmetic.policy() == IntegerOverflowPolicy::Exact => {
            scalar_instructions::validate_exact_binary(actual, node, optimized, unit)?
        }
        (
            LegalizedScalarInstructionKind::Compare { .. },
            AbstractOperation::BooleanEqual { .. }
            | AbstractOperation::IntegerEqual { .. }
            | AbstractOperation::IntegerLessThan { .. }
            | AbstractOperation::IntegerLessOrEqual { .. },
        ) => scalar_instructions::validate_compare(actual, node, optimized)?,
        _ => return Err(invalid),
    }
    Ok(())
}

/// A legalized row's runtime index operands rejoin the extent's runtime
/// elements one for one, in path order: same selector value, obligation,
/// stride and extent. The accepted-fact join is the source projection's own
/// certificate; replay re-derives the elements and checks the carried run.
fn indices_rejoin(
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    elements: &[crate::structural_inputs::structural_reference_input::RuntimeElement],
) -> bool {
    indices.len() == elements.len()
        && indices.iter().zip(elements).all(|(index, element)| {
            index.operand.value == element.index
                && index.obligation == element.obligation
                && index.stride == element.stride
                && index.extent == element.extent
        })
}

/// The integer arithmetic pair a legalized kind realizes, when it realizes
/// one outside the exact family (whose replay owns its operator).
fn legalized_arithmetic_pair(
    kind: &LegalizedScalarInstructionKind,
) -> Option<(IntegerOverflowPolicy, IntegerArithmeticOperator)> {
    use IntegerArithmeticOperator as Operator;
    use IntegerOverflowPolicy as Policy;
    Some(match kind {
        LegalizedScalarInstructionKind::WrappingAdd { .. } => (Policy::Wrapping, Operator::Add),
        LegalizedScalarInstructionKind::WrappingSubtract { .. } => {
            (Policy::Wrapping, Operator::Subtract)
        }
        LegalizedScalarInstructionKind::WrappingMultiply { .. } => {
            (Policy::Wrapping, Operator::Multiply)
        }
        LegalizedScalarInstructionKind::WrappingDivide { .. } => {
            (Policy::Wrapping, Operator::Divide)
        }
        LegalizedScalarInstructionKind::WrappingRemainder { .. } => {
            (Policy::Wrapping, Operator::Remainder)
        }
        LegalizedScalarInstructionKind::SaturatingAdd { .. } => (Policy::Saturating, Operator::Add),
        LegalizedScalarInstructionKind::SaturatingSubtract { .. } => {
            (Policy::Saturating, Operator::Subtract)
        }
        LegalizedScalarInstructionKind::SaturatingMultiply { .. } => {
            (Policy::Saturating, Operator::Multiply)
        }
        _ => return None,
    })
}
