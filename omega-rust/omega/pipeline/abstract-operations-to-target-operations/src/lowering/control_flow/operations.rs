//! Ordered graph operations and their durable scalar result homes.
use super::LiveDefinitions;
use super::observations;
use crate::lowering::shared::*;

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
    native_callbacks: &BTreeMap<OperationId, target_operations::TargetNativeCallbackArgument>,
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
    match operation {
        AbstractOperation::CallStructuralScalarWithDynamicArguments { .. } => {
            let home = crate::lowering::unit::lower_dynamic_argument_scalar_call(
                operation,
                function,
                target,
                functions,
                structural_types,
                parameters_by_place,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
                &mut live.integers,
                operations,
                provenance,
            )?;
            if home.scalar_type == ScalarType::Boolean {
                live.scalar_homes.insert(home.source_value, home);
            }
            Ok(())
        }
        AbstractOperation::CallUnitWithDynamicArguments { .. } => {
            crate::lowering::unit::lower_dynamic_argument_unit_call(
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
            )
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
        AbstractOperation::EstablishScalarArray { .. } => super::scalar_arrays::establish(
            operation,
            function,
            structural_types,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::EstablishScalarCase { .. } => {
            super::aggregate_results::establish_scalar_case(
                operation,
                function,
                structural_types,
                live,
                operations,
                provenance,
            )
        }
        AbstractOperation::CallStructural { .. } => super::aggregate_results::call(
            operation,
            function,
            target,
            functions,
            structural_types,
            prepared,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::EstablishPrimitiveLocal { .. }
        | AbstractOperation::PrimitiveLocalStore { .. }
        | AbstractOperation::PrimitiveScalarRead { .. } => super::primitive_storage::lower(
            operation,
            function,
            structural_types,
            prepared,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::CallStructuralScalar { .. } => super::primitive_calls::lower(
            operation,
            function,
            target,
            functions,
            structural_types,
            prepared,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::CallUnit {
            structural_arguments,
            ..
        } if structural_arguments.iter().any(|argument| {
            live.structural_homes.contains_key(&argument.place)
                || (argument.access == StructuralAccess::Owned
                    && function.structural_parameters.iter().any(|parameter| {
                        parameter.place == argument.place
                            && super::scalar_arrays::is_owned_parameter(parameter, structural_types)
                    }))
        }) =>
        {
            super::primitive_calls::lower(
                operation,
                function,
                target,
                functions,
                structural_types,
                prepared,
                live,
                operations,
                provenance,
            )
        }
        AbstractOperation::ByteSequenceWrite { .. } => super::byte_write::lower(
            operation,
            function,
            structural_types,
            prepared,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::StructuralScalarFieldStore { .. } => {
            crate::lowering::unit::lower_field_store(
                operation,
                function,
                structural_types,
                parameters_by_place,
                &live.integers,
                &live.booleans,
                &live.ieee_float_constants,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
                operations,
                provenance,
            )
        }
        AbstractOperation::WriteOnlyPrimitiveStore { .. } => {
            crate::lowering::unit::write_only_primitive_store::lower_write_only_primitive_store(
                operation,
                function,
                structural_types,
                parameters_by_place,
                &live.integers,
                &live.booleans,
                &live.ieee_float_constants,
                &live.scalar_homes,
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
                    )
                )
            }) =>
        {
            crate::lowering::unit::boundary_call::lower_boundary_call(
                operation,
                function,
                target,
                functions,
                structural_types,
                boundary_machines,
                settlements,
                installed_calls,
                native_callbacks,
                parameters_by_place,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
                &BTreeMap::new(),
                &mut live.integers,
                operations,
                provenance,
                &mut live.nonreturning,
            )?;
            if let Some(TargetUnitOperation::BoundarySettlement {
                result: target_operations::TargetBoundaryResult::Structural(home),
                ..
            }) = operations.last()
                && live
                    .structural_homes
                    .insert(home.place(), home.clone())
                    .is_some()
            {
                return Err(LoweringError::UnsupportedControlFlow(function.machine));
            }
            Ok(())
        }
        AbstractOperation::ByteSequenceLength { .. }
        | AbstractOperation::ByteSequenceRead { .. }
        | AbstractOperation::ByteSequenceSubslice { .. }
        | AbstractOperation::IntegerEqual { .. }
        | AbstractOperation::IntegerLessThan { .. }
        | AbstractOperation::IntegerLessOrEqual { .. }
        | AbstractOperation::BooleanNot { .. }
        | AbstractOperation::BooleanEqual { .. }
        | AbstractOperation::ExactIntegerAdd { .. }
        | AbstractOperation::ExactIntegerSubtract { .. }
        | AbstractOperation::IntegerExactCast { .. } => observations::lower(
            operation,
            function,
            structural_types,
            prepared,
            live,
            operations,
            provenance,
        ),
        AbstractOperation::Call { .. } => {
            let call = crate::lowering::unit::scalar_call::lower_scalar_call(
                operation,
                target,
                functions,
                scalar_abis,
                |value| super::scalar_sources::source(value, function, live),
            )?;
            let TargetUnitOperation::ScalarCall {
                psi_operation,
                result_home: home,
                ..
            } = &call
            else {
                unreachable!("scalar call planner returns its owned row")
            };
            if matches!(
                home.scalar_type,
                ScalarType::Boolean | ScalarType::IeeeFloat(_)
            ) {
                if live.scalar_homes.insert(home.source_value, *home).is_some() {
                    return Err(LoweringError::DuplicateValue(home.source_value));
                }
            } else {
                crate::lowering::unit::scalar_call::insert_known_unit_integer(
                    &mut live.integers,
                    home.source_value,
                    super::KnownUnitInteger::Home(*home),
                )?;
            }
            provenance.operations.push(*psi_operation);
            operations.push(call);
            Ok(())
        }
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
        AbstractOperation::CallUnit {
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        } if claim_transfers.is_empty()
            && requirement_obligations.is_empty()
            && crash_continuations.is_empty() =>
        {
            crate::lowering::unit::structural_call::lower_structural_unit_call(
                operation,
                function,
                target,
                functions,
                structural_types,
                parameters_by_place,
                &BTreeMap::new(),
                &live.views,
                &live.integers,
                &BTreeMap::new(),
                &live.booleans,
                &live.ieee_float_constants,
                &live.scalar_block_parameters,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
                operations,
                provenance,
            )
        }
        _ => Err(LoweringError::UnsupportedControlFlow(function.machine)),
    }
}
