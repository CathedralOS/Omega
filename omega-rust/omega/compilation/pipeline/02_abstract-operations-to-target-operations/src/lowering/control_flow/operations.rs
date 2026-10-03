//! Ordered graph operations and their durable scalar result homes.
use super::observations;
use super::{KnownUnitInteger, LiveDefinitions};
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractDynamicDispatch, AbstractFunction, AbstractOperation};
use calling_conventions::ValueShape;
use installation_evidence::InstalledProviderCallEvidence;
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
    structural_types: &StructuralTypeLookup<'_>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderCallEvidence,
    >,
    scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
    native_callbacks: &BTreeMap<OperationId, Vec<target_operations::TargetNativeCallbackArgument>>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    if live.nonreturning {
        return Err(LoweringError::InvalidHostedExitProcessShape(
            function.machine,
        ));
    }
    let installed = if let AbstractOperation::BoundaryCall {
        psi_operation,
        boundary,
        ..
    } = operation
    {
        installed_calls.get(&(function.machine, *psi_operation, *boundary))
    } else {
        None
    };
    let resolved = installed
        .map(|installed| super::installed_calls::resolve(operation, installed, functions))
        .transpose()?;
    let operation = resolved
        .as_ref()
        .map_or(operation, |(operation, _)| operation);
    // Reference custody: reads, writes and establishments cannot touch a
    // suspended referent, and a live carrier is never owned data. Call-shaped
    // operations route through the custody-aware argument lowering below.
    let (accessed_root, referent_access) = match operation {
        AbstractOperation::AtomicEvent { event, .. } => (event.place(), false),
        AbstractOperation::PrimitiveScalarRead { source, .. }
        | AbstractOperation::StructuralCaseMembership { source, .. }
        | AbstractOperation::StructuralCase { source, .. }
        | AbstractOperation::StructuralScalarFieldRead { source, .. } => {
            // A scalar or case observation spelled on a live reference
            // carrier reads through the loan's own authority — the referent
            // observation the borrow exists to serve, not owned-data use of
            // the carrier place. A write-only loan has no read authority.
            let referent_read = live.references.iter().any(|((carrier, path), custody)| {
                *carrier == *source
                    && path.is_empty()
                    && !live
                        .references
                        .values()
                        .any(|child| child.parent.as_ref() == Some(&custody.identity))
                    && structural_types
                        .get(&custody.result.structural_type)
                        .is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                StructuralTypeShape::Reference {
                                    access: terminal_psi::StructuralAccess::SharedBorrow
                                        | terminal_psi::StructuralAccess::MutableBorrow,
                                    ..
                                }
                            )
                        })
            });
            (Some(*source), referent_read)
        }
        AbstractOperation::StructuralLeafCopy { source, .. } => (Some(*source), false),
        AbstractOperation::PrimitiveLocalStore { destination, .. } => (Some(*destination), false),
        AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } => {
            (Some(destination.place()), false)
        }
        AbstractOperation::StructuralScalarFieldStore {
            destination, path, ..
        } => (
            Some(destination.place()),
            super::references::writable_referent_destination(
                function,
                live,
                structural_types,
                destination.place(),
                path,
            )
            .is_some(),
        ),
        AbstractOperation::EstablishPrimitiveLocal { result, .. } => (Some(result.place), false),
        AbstractOperation::EstablishStructuralLocal { result, .. } => (Some(result.place), false),
        AbstractOperation::MoveStructuralField {
            source: root, path, ..
        }
        | AbstractOperation::StoreStructuralField {
            destination: root,
            path,
            ..
        } => (
            Some(root.place()),
            super::borrowed_windows::window_referent(function, live, structural_types, root, path)?
                .is_some(),
        ),
        _ => (None, false),
    };
    if !referent_access
        && accessed_root.is_some_and(|place| {
            super::references::check_operation_access(
                function,
                live,
                structural_types,
                operation,
                place,
            )
        })
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    // Call paths that do not lower reference-aware arguments reject any
    // structural argument that touches live custody outright.
    let blind_arguments: &[terminal_psi::StructuralArgument] = match operation {
        AbstractOperation::CallStructural {
            structural_arguments,
            dynamic_arguments,
            ..
        } if !dynamic_arguments.is_empty() => structural_arguments.as_slice(),
        AbstractOperation::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments.as_slice(),
        AbstractOperation::CallDynamic {
            dispatch: AbstractDynamicDispatch::Rebound(dynamic_dispatch),
            ..
        } => std::slice::from_ref(&dynamic_dispatch.rebound.source),
        _ => &[],
    };
    if blind_arguments
        .iter()
        .any(|argument| super::references::touches(function, structural_types, live, argument))
    {
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
            live,
            operations,
            provenance,
        ),
        AbstractOperation::StoreDynamicDescriptor { psi_operation, .. } => {
            crate::lowering::unit::dynamic::lower_stored_descriptor(
                operation,
                function,
                target,
                functions,
                structural_types,
                parameters_by_place,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
                operations,
                provenance,
            )?;
            if !live.stored_descriptors.insert(*psi_operation) {
                return Err(LoweringError::InvalidDynamicDispatch {
                    machine: function.machine,
                    operation: *psi_operation,
                });
            }
            Ok(())
        }
        // Descriptor dispatches and direct calls that forward descriptors lower
        // to one `CallDynamic` row. This arm must stay ahead of the direct-call
        // arm below, which admits no forwarded descriptors.
        AbstractOperation::CallDynamic { .. } | AbstractOperation::CallStructural { .. }
            if crate::lowering::unit::dynamic::is_dynamic_call(operation) =>
        {
            let home = crate::lowering::unit::dynamic::lower_dynamic_call(
                operation,
                function,
                target,
                functions,
                structural_types,
                parameters_by_place,
                &prepared.dynamic_parameters,
                &live.stored_descriptors,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
                &mut live.integers,
                operations,
                provenance,
            )?;
            if let Some(home) = home
                && home.scalar_type == ScalarType::Boolean
            {
                live.scalar_homes.insert(home.source_value, home);
            }
            Ok(())
        }
        // Descriptor parameters are interface declarations: they produce no
        // graph operation, but the declared row must be the same one the
        // signature lane bound at this ordinal.
        AbstractOperation::DynamicDescriptorParameter { parameter } => {
            if prepared
                .dynamic_parameters
                .get(usize::try_from(parameter.ordinal).unwrap_or(usize::MAX))
                .is_none_or(|abi| abi.parameter != *parameter)
            {
                return Err(LoweringError::InvalidDynamicDescriptorParameter {
                    machine: function.machine,
                    ordinal: parameter.ordinal,
                });
            }
            Ok(())
        }
        AbstractOperation::FloatResultPolicy {
            psi_operation,
            result,
            format,
            policy,
        } => {
            // Resolve every input from the same live environment as the
            // preceding call. The adapter never invokes its provider again.
            let source = |value| super::scalar_sources::source(value, function, live);
            let policy = match *policy {
                terminal_psi::FloatResultPolicy::Trapping { raw_result } => {
                    target_operations::TargetFloatResultPolicy::Trapping {
                        raw_result: source(raw_result)?,
                    }
                }
                terminal_psi::FloatResultPolicy::Saturating {
                    raw_result,
                    left,
                    right,
                    rule,
                } => target_operations::TargetFloatResultPolicy::Saturating {
                    left: source(left)?,
                    right: source(right)?,
                    raw_result: source(raw_result)?,
                    rule,
                },
            };
            let scalar_type = ScalarType::IeeeFloat(*format);
            if policy
                .operands()
                .any(|operand| operand.scalar_type() != scalar_type)
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
        | AbstractOperation::EstablishStructuralArray { .. }
        | AbstractOperation::EstablishCase { .. } => super::records::establish(
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
        // Every other direct call, whatever its result, lowers to one `Call`
        // row through the shared direct-call lowering.
        AbstractOperation::CallStructural { .. } | AbstractOperation::Call { .. } => {
            super::direct_calls::lower_direct_call(
                operation,
                function,
                target,
                functions,
                structural_types,
                scalar_abis,
                prepared,
                parameters_by_place,
                live,
                operations,
                provenance,
            )
        }
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
        | AbstractOperation::StructuralByteSequenceFieldLength { .. }
        | AbstractOperation::StructuralByteSequenceFieldRead { .. }
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
        AbstractOperation::ByteSequenceStore { .. }
        | AbstractOperation::StructuralByteSequenceFieldStore { .. } => super::byte_write::replace(
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
                crate::lowering::scalar::element_views::ElementViewBackings::new(
                    &live.structural_homes,
                    &live.address_joins,
                ),
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
            // The mutable loan's affine custody ends at scope exit: record
            // the view place's result home so edge and return cleanup admit
            // its no-code discard alongside other affine result homes.
            if result.multiplicity == terminal_psi::StructuralMultiplicity::Affine {
                let home =
                    super::aggregate_results::home(*psi_operation, result, structural_types)?;
                if live.structural_homes.insert(*destination, home).is_some() {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
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
                } => super::references::writable_referent_destination(
                    function,
                    live,
                    structural_types,
                    destination.place(),
                    path,
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
            if super::ieee_float_arithmetic::is_inline_arithmetic(*boundary, settlements) =>
        {
            super::ieee_float_arithmetic::lower(
                operation,
                function,
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
                &mut live.nonreturning,
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
        | AbstractOperation::ByteSequencePredicate { .. }
        | AbstractOperation::ByteSequenceEqual { .. }
        | AbstractOperation::ByteSequenceRead { .. }
        | AbstractOperation::ByteSequenceSubslice { .. }
        | AbstractOperation::ElementViewLength { .. }
        | AbstractOperation::ElementViewRead { .. }
        | AbstractOperation::ElementViewSubslice { .. }
        | AbstractOperation::IntegerEqual { .. }
        | AbstractOperation::IntegerLessThan { .. }
        | AbstractOperation::IntegerLessOrEqual { .. }
        | AbstractOperation::BooleanNot { .. }
        | AbstractOperation::BooleanEqual { .. }
        | AbstractOperation::IntegerArithmetic { .. }
        | AbstractOperation::TrappingInteger { .. }
        | AbstractOperation::IntegerBitwiseAnd { .. }
        | AbstractOperation::IntegerBitwiseOr { .. }
        | AbstractOperation::IntegerBitwiseXor { .. }
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
    if let (Some(installed), Some((_, origin))) = (installed, resolved) {
        super::installed_calls::retain_origin(&mut operations[first_output..], installed, origin)?;
    }
    Ok(())
}
