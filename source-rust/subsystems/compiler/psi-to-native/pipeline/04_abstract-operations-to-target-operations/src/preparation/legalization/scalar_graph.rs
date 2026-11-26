//! Retained target-operation legalization helpers and boundary checks.

use crate::preparation::LegalizationError;
use calling_conventions::ValueShape;
use legalized_operations::{SaturatingCarrier, TrappingForm, TrappingOperation};
use semantic_vocabulary::{IntegerType, ScalarType};
use target_operations::{TargetFunction, TargetOperationPlan, TargetUnitOperation};

pub(crate) mod call_origin {
    use super::{TargetOperationPlan, TargetUnitOperation};
    use crate::preparation::LegalizationError;
    use target_operations::NativeCallOrigin;

    pub(crate) fn validate_boundary_origins(
        native: &TargetOperationPlan,
        input: crate::preparation::BoundaryOriginFacts<'_>,
    ) -> Result<(), LegalizationError> {
        let profile = input
            .target_context
            .map(|selection| target::TargetProfile::from_canonical_target_name(selection.target()))
            .transpose()
            .map_err(|cause| LegalizationError::custody_with_cause(cause))?;
        if native.profile != profile
            || profile.is_some_and(|profile| profile.native_realization() != Some(native.target))
            || input
                .target_context
                .is_some_and(|selection| selection.terminal_psi() != native.psi)
        {
            return Err(LegalizationError::custody());
        }
        validate_qualified_numeric_executions(
            native,
            &input.module.interfaces.boundary_machines,
            Some(input.module),
        )?;
        for function in &native.functions {
            for operation in function.graph.operations() {
                if let TargetUnitOperation::Call {
                    origin:
                        NativeCallOrigin::ProgramEntry {
                            boundary,
                            entry,
                            source_signature,
                            ..
                        },
                    target,
                    ..
                } = operation
                {
                    let binding = input.program_entry.ok_or_else(LegalizationError::custody)?;
                    if binding.boundary != *boundary
                        || binding.entry != *entry
                        || binding.source_signature != *source_signature
                        || *target != target_operations::NativeCallTarget::Machine(*entry)
                    {
                        return Err(LegalizationError::custody());
                    }
                }
            }
        }
        Ok(())
    }

    /// Check only numeric boundaries consumed by the selected target rows.
    /// Each distinct qualified declaration is authenticated once; occurrences
    /// then compare the full operation, including its conversion policy.
    pub(crate) fn validate_qualified_numeric_executions(
        native: &TargetOperationPlan,
        declarations: &[terminal_psi::BoundaryMachineDeclaration],
        module: Option<&terminal_psi::TerminalModule>,
    ) -> Result<(), LegalizationError> {
        use semantic_vocabulary::BoundaryMachineId;
        use target_operations::{BoundaryExecutionBinding, CompilerBuiltinExecution};
        let mut consumed: Vec<_> = native
            .functions
            .iter()
            .flat_map(|function| function.graph.operations())
            .filter_map(|operation| match operation {
                TargetUnitOperation::NearestIeeeFloatArithmetic { boundary, .. } => Some(*boundary),
                _ => None,
            })
            .collect();
        consumed.sort_unstable();
        consumed.dedup();
        let mut retained: Vec<_> = declarations.iter().collect();
        retained.sort_unstable_by_key(|row| row.id);
        let mut canonical: Vec<_> = module
            .into_iter()
            .flat_map(|module| &module.interfaces.boundary_machines)
            .collect();
        canonical.sort_unstable_by_key(|row| row.id);
        fn unique<'a>(
            rows: &[&'a terminal_psi::BoundaryMachineDeclaration],
            id: BoundaryMachineId,
        ) -> Result<&'a terminal_psi::BoundaryMachineDeclaration, LegalizationError> {
            let index = rows
                .binary_search_by_key(&id, |row| row.id)
                .map_err(|cause| LegalizationError::custody_with_cause(cause))?;
            if index
                .checked_sub(1)
                .is_some_and(|previous| rows[previous].id == id)
                || rows.get(index + 1).is_some_and(|next| next.id == id)
            {
                return Err(LegalizationError::custody());
            }
            Ok(rows[index])
        }
        let mut executions = Vec::new();
        for id in consumed {
            let declaration = unique(&retained, id)?;
            let canonical_declaration = module.map(|_| unique(&canonical, id)).transpose()?;
            let is_qualified = |row: &terminal_psi::BoundaryMachineDeclaration| matches!(row.result, terminal_psi::BoundaryMachineResult::Scalar(result) if !result.qualifications.is_empty());
            if !is_qualified(declaration) && !canonical_declaration.is_some_and(is_qualified) {
                continue;
            }
            let module = module.ok_or_else(LegalizationError::custody)?;
            if canonical_declaration != Some(declaration) {
                return Err(LegalizationError::custody());
            }
            let execution = provider_admission::numeric_boundary_execution(module, declaration)
                .map_err(|cause| LegalizationError::custody_with_cause(cause))?;
            executions.push((id, execution));
        }
        for operation in native
            .functions
            .iter()
            .flat_map(|function| function.graph.operations())
        {
            let TargetUnitOperation::NearestIeeeFloatArithmetic {
                boundary,
                realization,
                execution,
                ..
            } = operation
            else {
                continue;
            };
            let Ok(index) = executions.binary_search_by_key(boundary, |(id, _)| *id) else {
                continue;
            };
            let expected = executions[index].1;
            if expected
                != (CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
                    operation: realization.operation,
                    format: realization.format,
                })
                || *execution != BoundaryExecutionBinding::CompilerBuiltin(expected)
            {
                return Err(LegalizationError::custody());
            }
        }
        Ok(())
    }
}

pub(crate) mod normalized_foreign {
    use super::{TargetOperationPlan, TargetUnitOperation};
    use crate::preparation::LegalizationError;
    use calling_conventions::{BoundaryEntryPlan, ValueShape};
    use semantic_vocabulary::{MachineId, OperationId};
    use target_operations::TargetNativeCallbackArgument;

    pub(crate) fn validate_native_callback_roster(
        native: &TargetOperationPlan,
    ) -> Result<(), LegalizationError> {
        for (index, callback) in native.native_callback_arguments.iter().enumerate() {
            if native.native_callback_arguments[..index]
                .iter()
                .any(|prior| {
                    prior.terminal_operation == callback.terminal_operation
                        && prior.placement_index == callback.placement_index
                })
            {
                return Err(LegalizationError::custody());
            }
            let function = native
                .function(callback.registrar_machine)
                .ok_or_else(LegalizationError::custody)?;
            let row = super::unique_row(function, |row| {
                matches!(
                    row,
                    TargetUnitOperation::NormalizedForeignCall { psi_operation, .. }
                        if *psi_operation == callback.terminal_operation
                )
            })?;
            let Some(TargetUnitOperation::NormalizedForeignCall { binding, .. }) = row else {
                return Err(LegalizationError::custody());
            };
            if binding.boundary_entry_plan != callback.registrar_boundary_entry_plan {
                return Err(LegalizationError::custody());
            }
        }
        Ok(())
    }

    pub(crate) fn native_callbacks_at<'a>(
        native: &'a TargetOperationPlan,
        machine: MachineId,
        operation: OperationId,
        boundary_entry_plan: &BoundaryEntryPlan,
    ) -> Result<Vec<&'a TargetNativeCallbackArgument>, LegalizationError> {
        let materializations = &boundary_entry_plan.call.callback_materializations;
        let matching: Vec<&'a TargetNativeCallbackArgument> = native
            .native_callback_arguments
            .iter()
            .filter(|callback| {
                callback.registrar_machine == machine && callback.terminal_operation == operation
            })
            .collect();
        if materializations.is_empty() {
            return if matching.is_empty() {
                Ok(Vec::new())
            } else {
                Err(LegalizationError::custody())
            };
        }
        if matching.len() != materializations.len() {
            return Err(LegalizationError::custody());
        }
        let pointer_size = u16::try_from(native.target.pointer_size)
            .map_err(|cause| LegalizationError::custody_with_cause(cause))?;
        let pointer_alignment = u16::try_from(native.target.pointer_alignment)
            .map_err(|cause| LegalizationError::custody_with_cause(cause))?;
        for (index, callback) in matching.iter().enumerate() {
            let ordinal = usize::try_from(callback.application.native_ordinal)
                .map_err(|cause| LegalizationError::custody_with_cause(cause))?;
            let access_root = boundary_entry_plan.call.parameters.get(ordinal)
                == Some(&callback.application.placement);
            let destination_admitted = match &callback.destination {
                calling_conventions::NativePlace::Parameter(parameter) => {
                    *parameter == callback.application.parameter
                        && callback.field_placements.is_none()
                        && access_root
                }
                calling_conventions::NativePlace::Field { parameter, .. } => {
                    *parameter == callback.application.parameter
                        && callback.field_placements.is_some()
                        && access_root
                }
            };
            if callback.registrar_boundary_entry_plan != *boundary_entry_plan
                || !callback.callback_function.is_valid()
                || callback.callback_function.callback_thunk_placement_index()
                    != Some(callback.placement_index)
                || callback.application.shape != callback.application.placement.shape
                || callback.application.shape
                    != ValueShape::integer(u32::from(pointer_size), pointer_alignment)
                || !destination_admitted
                || !materializations
                    .iter()
                    .any(|materialization| materialization.destination == callback.destination)
                || matching[..index].iter().any(|prior| {
                    prior.placement_index == callback.placement_index
                        || prior.destination == callback.destination
                })
            {
                return Err(LegalizationError::custody());
            }
        }
        if matching
            .iter()
            .filter(|callback| {
                matches!(
                    callback.destination,
                    calling_conventions::NativePlace::Parameter(_)
                )
            })
            .count()
            > 1
        {
            return Err(LegalizationError::custody());
        }
        Ok(matching)
    }
}

pub(crate) fn structural_parameters(
    target: &TargetFunction,
) -> Option<&[target_operations::TargetStructuralParameter]> {
    if let Some(abi) = &target.mixed_structural_scalar_abi {
        Some(&abi.structural_parameters)
    } else {
        (!target.graph.parameters.is_empty()).then_some(target.graph.parameters.as_slice())
    }
}

pub(crate) fn structural_contract(
    target: &TargetFunction,
) -> Result<Option<legalized_operations::LegalizedStructuralContract>, LegalizationError> {
    let Some(contract) = &target.structural_contract else {
        return Ok(None);
    };
    let parameters = structural_parameters(target).unwrap_or(&[]);
    if parameters.len() != contract.parameters.len() {
        return Err(LegalizationError::custody());
    }
    Ok(Some(legalized_operations::LegalizedStructuralContract {
        structural_types: target.graph.structural_types.clone(),
        parameters: contract
            .parameters
            .iter()
            .zip(parameters)
            .map(
                |(semantic, target)| legalized_operations::LegalizedCallUnitParameter {
                    semantic: semantic.clone(),
                    target: target.clone(),
                },
            )
            .collect(),
        structural_places: contract.places.clone(),
        entry_claims: contract.entry_claims.clone(),
        published_service_ceiling: contract.published_service_ceiling.clone(),
        result: contract.result.clone(),
    }))
}

pub(crate) fn scalar_shape(scalar: ScalarType) -> Option<ValueShape> {
    ValueShape::for_scalar(scalar)
}

pub(crate) fn saturating_carrier(integer: IntegerType) -> Option<SaturatingCarrier> {
    SaturatingCarrier::from_integer(integer)
}

pub(crate) fn trapping_form(
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

pub(crate) fn supports_wrapping_division(integer: IntegerType) -> bool {
    integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed
        && scalar_shape(ScalarType::Integer(integer)).is_some()
}

pub(crate) fn unique_row(
    function: &TargetFunction,
    mut matches: impl FnMut(&TargetUnitOperation) -> bool,
) -> Result<Option<&TargetUnitOperation>, LegalizationError> {
    let mut rows = function.graph.operations().filter(|row| matches(row));
    let row = rows.next();
    if rows.next().is_some() {
        return Err(LegalizationError::custody());
    }
    Ok(row)
}
