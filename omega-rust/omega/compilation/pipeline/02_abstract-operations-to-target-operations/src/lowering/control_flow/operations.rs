//! Ordered graph operations and their durable scalar result homes.
use super::observations;
use super::{KnownUnitInteger, LiveDefinitions};
use crate::LoweringError;
use crate::lowering::selected_boundary_calls::SelectedBoundaryCall;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use calling_conventions::ValueShape;
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId, PlaceId, ScalarType};
use std::collections::{BTreeMap, BTreeSet};
use target::NativeTarget;
use target_operations::{
    BoundaryRealization, BoundarySettlementBinding, ScalarFunctionAbi, TargetStructuralParameter,
    TargetUnitOperation, TargetUnitScalarHomeRequirement, TerminalPsiProvenance,
};
use terminal_psi::StructuralTypeShape;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_operation(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    structural_types: &StructuralTypeLookup<'_>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    selected_calls: &BTreeMap<(MachineId, OperationId, BoundaryMachineId), SelectedBoundaryCall>,
    scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
    native_callbacks: &BTreeMap<OperationId, Vec<target_operations::TargetNativeCallbackArgument>>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let selected = if let AbstractOperation::BoundaryCall {
        psi_operation,
        boundary,
        ..
    } = operation
    {
        selected_calls.get(&(function.machine, *psi_operation, *boundary))
    } else {
        None
    };
    let resolved = selected
        .map(|selected| super::selected_boundary_calls::resolve(operation, selected, functions))
        .transpose()?;
    let operation = resolved
        .as_ref()
        .map_or(operation, |(operation, _)| operation);
    // Opaque foreign ABI calls cannot consume reference-carrier storage. This
    // is a native representation restriction, not a second borrow judgment.
    // Ordinary operations go straight to their storage/address owner below;
    // Psi owns access legality and the corpus independently audits custody.
    let boundary_arguments: &[terminal_psi::StructuralArgument] = match operation {
        AbstractOperation::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments.as_slice(),
        _ => &[],
    };
    if !super::references::moved_argument_leaves(live, boundary_arguments).is_empty() {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let first_output = operations.len();
    match operation {
        AbstractOperation::AtomicEvent {
            psi_operation,
            event,
            ..
        } => super::atomic_access::lower(
            *psi_operation,
            event,
            function,
            structural_types,
            prepared,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::FloatResultPolicy {
            psi_operation,
            result,
            format,
            policy,
        } => {
            // Resolve every input from the same live environment as the
            // preceding call. The adapter never invokes its provider again.
            let source = |value| super::scalar_sources::source(value, function, live);
            let policy = match policy {
                terminal_psi::FloatResultPolicy::Trapping { raw_result } => {
                    target_operations::TargetFloatResultPolicy::Trapping {
                        raw_result: source(*raw_result)?,
                    }
                }
                terminal_psi::FloatResultPolicy::Saturating {
                    raw_result,
                    eligible,
                } => target_operations::TargetFloatResultPolicy::Saturating {
                    eligible: source(*eligible)?,
                    raw_result: source(*raw_result)?,
                },
            };
            let scalar_type = ScalarType::IeeeFloat(*format);
            if !policy
                .semantic_policy()
                .accepts_operand_types(*format, |value| {
                    policy
                        .operands()
                        .find(|operand| operand.source_value() == value)
                        .map(|operand| operand.scalar_type())
                })
            {
                return Err(LoweringError::ValueTypeMismatch(*result));
            }
            let result_home = TargetUnitScalarHomeRequirement {
                defining_operation: *psi_operation,
                source_value: *result,
                scalar_type,
                shape: crate::lowering::scalar_abi::fixed_native_scalar_shape(scalar_type)
                    .ok_or(LoweringError::ValueTypeMismatch(*result))?,
            };
            if live.scalar_homes.insert(*result, result_home).is_some() {
                return Err(LoweringError::DuplicateValue(*result));
            }
            provenance.operations.push(*psi_operation);
            operations.push(TargetUnitOperation::FloatResultPolicy {
                psi_operation: *psi_operation,
                result_home,
                format: *format,
                policy,
            });
            Ok(())
        }
        AbstractOperation::IeeeFloatCompare {
            psi_operation,
            result,
            comparison,
            format,
            left,
            right,
        } => {
            let left = super::scalar_sources::source(*left, function, live)?;
            let right = super::scalar_sources::source(*right, function, live)?;
            if left.scalar_type() != ScalarType::IeeeFloat(*format)
                || right.scalar_type() != ScalarType::IeeeFloat(*format)
            {
                return Err(LoweringError::ValueTypeMismatch(*result));
            }
            let result_home = TargetUnitScalarHomeRequirement {
                defining_operation: *psi_operation,
                source_value: *result,
                scalar_type: ScalarType::Boolean,
                shape: ValueShape::integer(1, 1),
            };
            live.scalar_homes.insert(*result, result_home);
            provenance.operations.push(*psi_operation);
            operations.push(TargetUnitOperation::IeeeFloatCompare {
                result_home,
                comparison: *comparison,
                format: *format,
                left,
                right,
            });
            Ok(())
        }
        AbstractOperation::ScalarRepresentationView {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } => {
            let operand = super::scalar_sources::source(*operand, function, live)?;
            if operand.scalar_type() != *source_type
                || terminal_semantics::ScalarRepresentationViewPair::classify(
                    *source_type,
                    *target_type,
                )
                .is_none()
            {
                return Err(LoweringError::ValueTypeMismatch(*result));
            }
            let result_home = TargetUnitScalarHomeRequirement {
                defining_operation: *psi_operation,
                source_value: *result,
                scalar_type: *target_type,
                shape: crate::lowering::scalar_abi::fixed_native_scalar_shape(*target_type)
                    .ok_or(LoweringError::ValueTypeMismatch(*result))?,
            };
            let duplicate = match target_type {
                ScalarType::Integer(_) => live
                    .integers
                    .insert(*result, KnownUnitInteger::Home(result_home))
                    .is_some(),
                ScalarType::Boolean | ScalarType::IeeeFloat(_) => {
                    live.scalar_homes.insert(*result, result_home).is_some()
                }
            };
            if duplicate {
                return Err(LoweringError::DuplicateValue(*result));
            }
            provenance.operations.push(*psi_operation);
            operations.push(TargetUnitOperation::ScalarRepresentationView {
                result_home,
                operand,
            });
            Ok(())
        }
        AbstractOperation::EstablishScalarArray { .. } => super::scalar_arrays::establish(
            operation,
            function,
            structural_types,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::EstablishRecord { .. }
        | abstract_operations::structural_array_or_case_pattern!(..) => super::records::establish(
            operation,
            function,
            structural_types,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::EstablishPlacedReference { .. } => super::references::establish_placed(
            operation,
            function,
            structural_types,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::EstablishReference { .. } => super::references::establish(
            operation,
            function,
            structural_types,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::ReleaseReference { .. } => {
            super::references::release(operation, function, live, operations, provenance)
        }
        AbstractOperation::SelectImplementation {
            psi_operation,
            result,
            table,
        } => {
            if dynamic_dispatch.table(*table).is_none() {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let result_home =
                super::primitive_storage::retain_result(*psi_operation, *result, live)?;
            provenance.operations.push(*psi_operation);
            operations.push(TargetUnitOperation::SelectImplementation {
                result_home,
                table: *table,
            });
            Ok(())
        }
        // Direct and selected calls share argument transport and result homes;
        // the call target alone determines how invocation finds executable code.
        abstract_operations::call_or_call_structural_pattern!(..) => super::calls::lower_call(
            operation,
            function,
            target,
            functions,
            dynamic_dispatch,
            structural_types,
            scalar_abis,
            prepared,
            parameters_by_place,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::StructuralCaseMembership { .. } => super::structural_case::observe(
            operation,
            function,
            structural_types,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::StructuralLeafCopy { .. } => super::leaf_copy::copy(
            operation,
            function,
            structural_types,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::MoveStructuralField { .. } => super::borrowed_windows::move_field(
            operation,
            function,
            structural_types,
            prepared,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::StoreStructuralField { .. } => super::borrowed_windows::store_field(
            operation,
            function,
            structural_types,
            prepared,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::EstablishZeroedStructural { .. } => {
            super::records::establish_zeroed_structural(
                operation,
                function,
                structural_types,
                live,
                operations,
                provenance,
            )
        }
        AbstractOperation::EstablishStructuralLocal { .. } => {
            super::records::establish_structural_local(
                operation,
                function,
                structural_types,
                live,
                operations,
                provenance,
            )
        }
        AbstractOperation::EstablishTrivialAffineLocal { .. } => {
            super::records::establish_trivial_affine_local(
                operation,
                function,
                structural_types,
                live,
                operations,
                provenance,
            )
        }
        AbstractOperation::EstablishPrimitiveLocal { .. }
        | AbstractOperation::PrimitiveLocalStore { .. }
        | AbstractOperation::PrimitiveScalarRead { .. }
        | abstract_operations::byte_len_or_field_read_pattern!(..)
        | AbstractOperation::StructuralScalarFieldRead { .. } => super::primitive_storage::lower(
            operation,
            function,
            structural_types,
            prepared,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::StructuralByteSequenceFieldByteStore { .. } => {
            super::byte_write::replace_byte(
                operation,
                function,
                structural_types,
                prepared,
                live,
                operations,
                provenance,
            )
        }
        AbstractOperation::StructuralByteViewFieldStore { .. } => {
            super::byte_write::replace_view_descriptor(
                operation,
                function,
                structural_types,
                prepared,
                live,
                operations,
                provenance,
            )
        }
        abstract_operations::byte_sequence_store_pair_pattern!(..) => super::byte_write::replace(
            operation,
            function,
            structural_types,
            prepared,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::ByteSequenceWrite { .. } => super::byte_write::lower(
            operation,
            function,
            structural_types,
            prepared,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::ElementViewWrite { .. } => super::element_view_write::lower(
            operation,
            function,
            structural_types,
            prepared,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::EstablishByteSequenceConcatenation { .. } => {
            super::byte_concatenation::lower(
                operation,
                function,
                structural_types,
                prepared,
                live,
                operations,
                provenance,
            )
        }
        AbstractOperation::EstablishByteSequenceLiteral {
            psi_operation,
            place,
            structural_type,
            bytes,
            qualifications,
        } => {
            if !matches!(
                place.kind,
                semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type: declared,
                    ..
                } if declared == structural_type.id
            ) || !matches!(
                structural_type.shape,
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                )
            ) || structural_types.get(&structural_type.id).copied() != Some(structural_type)
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            if live
                .views
                .insert(place.id, (*psi_operation, structural_type.id))
                .is_some()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            operations.push(TargetUnitOperation::EstablishByteSequenceLiteral {
                psi_operation: *psi_operation,
                place: *place,
                structural_type: structural_type.clone(),
                bytes: bytes.clone(),
                qualifications: qualifications.clone(),
            });
            provenance.operations.push(*psi_operation);
            Ok(())
        }
        AbstractOperation::EstablishElementView {
            psi_operation,
            result,
            destination,
            ..
        } => {
            let values = observations::scalar_values(live, &prepared.scalar_parameters)?;
            let (view, _) = super::super::scalar::element_views::element_view_for_place(
                function,
                structural_types,
                &prepared.parameters,
                live.element_view_backings(),
                &values,
                &live.lengths,
                *destination,
                &mut Vec::new(),
            )?;
            if live
                .views
                .insert(*destination, (*psi_operation, result.structural_type))
                .is_some()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            // Both shared and mutable establishments own their descriptor,
            // while the backing access remains the view's separate permission.
            // Record the exact producer contract for forwarding copyable shared
            // descriptors as well as cleaning up affine mutable descriptors.
            let home = super::aggregate_results::home(*psi_operation, result, structural_types)?;
            if live.structural_homes.insert(*destination, home).is_some() {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            operations.push(TargetUnitOperation::EstablishElementView {
                result: result.clone(),
                view,
            });
            provenance.operations.push(*psi_operation);
            Ok(())
        }
        AbstractOperation::StructuralScalarFieldStore { .. } => {
            let reference_destination = match operation {
                AbstractOperation::StructuralScalarFieldStore {
                    destination, path, ..
                } => super::references::referent_projection(
                    function,
                    live,
                    structural_types,
                    destination.place(),
                    path,
                    terminal_psi::StructuralAccess::WriteOnlyBorrow,
                ),
                _ => None,
            };
            crate::lowering::unit::lower_field_store(
                operation,
                function,
                structural_types,
                parameters_by_place,
                &live.structural_homes,
                reference_destination,
                &super::scalar_sources::ScalarSources::from(&*live),
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
                operations,
                provenance,
            )
        }
        AbstractOperation::WriteOnlyPrimitiveStore { value, .. } => {
            crate::lowering::unit::write_only_primitive_store::lower_write_only_primitive_store(
                operation,
                function,
                structural_types,
                parameters_by_place,
                &live.structural_homes,
                &live.integers,
                &live.booleans,
                &live.ieee_float_constants,
                &live.scalar_homes,
                live.scalar_block_parameters.get(&value.value).copied(),
                &super::scalar_sources::ScalarSources::from(&*live),
                operations,
                provenance,
            )
        }
        AbstractOperation::BoundaryCall { boundary, .. }
            if settlements.get(boundary).is_some_and(|binding| {
                matches!(
                    binding.realization,
                    target_operations::BoundarySettlementRealization::Builtin(
                        BoundaryRealization::PlacedScalarAccess(_)
                    )
                )
            }) =>
        {
            super::placed_scalar_access::lower(
                operation,
                function,
                structural_types,
                settlements,
                live,
                operations,
                provenance,
            )
        }
        AbstractOperation::BoundaryCall { boundary, .. }
            if super::ieee_float_arithmetic::is_inline_arithmetic(*boundary, settlements) =>
        {
            super::ieee_float_arithmetic::lower(
                operation,
                function,
                structural_types,
                target,
                boundary_machines,
                settlements,
                live,
                operations,
                provenance,
            )
        }
        AbstractOperation::BoundaryCall { boundary, .. }
            if settlements.get(boundary).is_some_and(|binding| {
                matches!(
                    binding.realization,
                    target_operations::BoundarySettlementRealization::Builtin(
                        BoundaryRealization::HostedWriteByteI32(_)
                            | BoundaryRealization::HostedExitProcessI32(_)
                            | BoundaryRealization::HostedReadByte(_)
                    ) | target_operations::BoundarySettlementRealization::NormalizedForeignCall(_)
                )
            }) =>
        {
            crate::lowering::unit::boundary_call::lower_boundary_call(
                operation,
                function,
                target,
                structural_types,
                boundary_machines,
                settlements,
                native_callbacks,
                parameters_by_place,
                &live.structural_homes,
                &live.address_joins,
                &live.block_views,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
                &BTreeMap::new(),
                &mut live.integers,
                &mut live.scalar_homes,
                &live.booleans,
                &live.ieee_float_constants,
                &live.scalar_block_parameters,
                operations,
                provenance,
            )?;
            if let Some(
                TargetUnitOperation::BoundarySettlement {
                    result: target_operations::TargetBoundaryResult::Structural(home),
                    ..
                }
                | TargetUnitOperation::NormalizedForeignCall {
                    result: target_operations::TargetBoundaryResult::Structural(home),
                    ..
                },
            ) = operations.last()
                && live
                    .structural_homes
                    .insert(home.place(), home.clone())
                    .is_some()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            Ok(())
        }
        AbstractOperation::ByteSequenceLength { .. }
        | abstract_operations::byte_read_predicate_equal_pattern!(..)
        | abstract_operations::element_view_or_byte_subslice_pattern!(..)
        | AbstractOperation::ElementViewLength { .. }
        | AbstractOperation::ElementViewRead { .. }
        | abstract_operations::integer_equal_less_pair_pattern!(..)
        | AbstractOperation::IntegerLessOrEqual { .. }
        | abstract_operations::boolean_not_or_equal_pair_pattern!(..)
        | AbstractOperation::IntegerArithmetic { .. }
        | AbstractOperation::TrappingInteger { .. }
        | AbstractOperation::IntegerBitwiseAnd { .. }
        | abstract_operations::bitwise_or_xor_pair_pattern!(..)
        | AbstractOperation::IntegerBitwiseNot { .. }
        | AbstractOperation::IntegerShift { .. }
        | AbstractOperation::IntegerExactCast { .. } => observations::lower(
            operation,
            function,
            structural_types,
            prepared,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type: ScalarType::Integer(scalar_type),
            value,
        } => crate::lowering::unit::scalar_definitions::lower_integer_constant(
            function.machine,
            *psi_operation,
            *result,
            *scalar_type,
            *value,
            false,
            &mut BTreeMap::new(),
            &mut live.integers,
            operations,
            provenance,
        ),
        AbstractOperation::BooleanConstant {
            psi_operation,
            result,
            value,
        } => crate::lowering::unit::scalar_definitions::lower_boolean_constant(
            function.machine,
            *psi_operation,
            *result,
            *value,
            false,
            &mut live.booleans,
            operations,
            provenance,
        ),
        AbstractOperation::IeeeFloatConstant {
            psi_operation,
            result,
            value,
        } => crate::lowering::unit::scalar_definitions::lower_ieee_float_constant(
            *psi_operation,
            *result,
            *value,
            false,
            &mut live.ieee_float_constants,
            operations,
            provenance,
        ),
        AbstractOperation::IntegerWiden { .. } => {
            crate::lowering::unit::scalar_definitions::lower_integer_widen(
                operation,
                function.machine,
                &prepared.scalar_parameters,
                false,
                &mut live.integers,
                operations,
                provenance,
            )
        }
        _ => Err(LoweringError::unsupported_control_flow(function.machine)),
    }?;
    if let (Some(selected), Some((_, origin))) = (selected, resolved) {
        super::selected_boundary_calls::retain_origin(
            &mut operations[first_output..],
            selected,
            origin,
        )?;
    }
    Ok(())
}
