//! Installation-shape validation for retained fixed-integer ABI and attached-
//! Unit scalar-call transport. Native byte replay remains object-owned.

use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use omega_target::NativeTarget;
use omega_target_operations::CallSiteOwner;
use psi_core::MachineId;

use super::{InstallationError, InstallationRecord, InstalledFunction};

fn fixed_integer_shape(integer: psi_core::IntegerType) -> Option<ValueShape> {
    if integer.is_address() || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let bytes = integer.bits() / 8;
    Some(ValueShape::integer(bytes, bytes))
}

pub(super) fn installed_function_scalar_transport_is_canonical(
    function: &InstalledFunction,
    target: NativeTarget,
) -> bool {
    let abi_is_canonical = function
        .fixed_integer_scalar_abi
        .as_ref()
        .is_none_or(|abi| {
            let Some(parameter_shapes) = abi
                .parameters
                .iter()
                .map(|parameter| fixed_integer_shape(parameter.scalar_type))
                .collect::<Option<Vec<_>>>()
            else {
                return false;
            };
            let Some(result_shape) = fixed_integer_shape(abi.result.scalar_type) else {
                return false;
            };
            let Ok(expected_plan) = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: parameter_shapes.clone(),
                    result: Some(result_shape),
                },
            ) else {
                return false;
            };
            expected_plan == abi.call_plan
                && abi.parameters.len() == abi.call_plan.parameters.len()
                && abi
                    .parameters
                    .iter()
                    .zip(&parameter_shapes)
                    .zip(&abi.call_plan.parameters)
                    .all(|((parameter, shape), placement)| {
                        parameter.placement == *placement && placement.shape == *shape
                    })
                && abi.call_plan.result.as_ref() == Some(&abi.result.placement)
                && abi.result.placement.shape == result_shape
                && abi
                    .parameters
                    .iter()
                    .map(|parameter| parameter.value)
                    .chain(std::iter::once(abi.result.value))
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    == abi.parameters.len() + 1
        });
    let homes_are_canonical = function
        .unit_scalar_homes
        .iter()
        .enumerate()
        .all(|(index, home)| {
            fixed_integer_shape(home.scalar_type) == Some(home.shape)
                && function.unit_scalar_homes[..index].iter().all(|prior| {
                    prior.defining_operation != home.defining_operation
                        && prior.source_value != home.source_value
                        && prior.byte_offset != home.byte_offset
                })
                && index.checked_sub(1).is_none_or(|previous| {
                    function.unit_scalar_homes[previous]
                        .byte_offset
                        .checked_add(8)
                        == Some(home.byte_offset)
                })
        });
    let constants_are_canonical =
        function
            .unit_integer_constants
            .iter()
            .enumerate()
            .all(|(index, constant)| {
                fixed_integer_shape(constant.scalar_type).is_some()
                    && constant.scalar_type.admits(constant.value)
                    && function.unit_integer_constants[..index]
                        .iter()
                        .all(|prior| {
                            prior.defining_operation != constant.defining_operation
                                && prior.source_value != constant.source_value
                                && prior.operation_ordinal < constant.operation_ordinal
                        })
            });
    abi_is_canonical && homes_are_canonical && constants_are_canonical
}

pub(super) fn validate_installed_unit_scalar_calls(
    record: &InstallationRecord,
    functions: &std::collections::BTreeMap<MachineId, &InstalledFunction>,
) -> Result<(), InstallationError> {
    let mut previous_key = None;
    for installed in &record.internal_unit_scalar_calls {
        let function = functions.get(&installed.machine).copied().ok_or(
            InstallationError::InvalidInternalUnitScalarCall(installed.machine),
        )?;
        let custody = &installed.custody;
        let target_abi = functions
            .get(&custody.target)
            .and_then(|target| target.fixed_integer_scalar_abi.as_ref())
            .ok_or(InstallationError::InvalidInternalUnitScalarCall(
                installed.machine,
            ))?;
        let CallSiteOwner::Operation(owner) = custody.owner else {
            return Err(InstallationError::InvalidInternalUnitScalarCall(
                installed.machine,
            ));
        };
        let key = (
            installed.machine,
            custody.operation_ordinal,
            custody.code_offset,
        );
        let expected_text_offset = function
            .text_offset
            .checked_add(custody.code_offset)
            .ok_or(InstallationError::InstalledScalarOffsetNotRepresentable)?;
        let call_end = custody
            .code_offset
            .checked_add(custody.byte_count)
            .ok_or(InstallationError::InstalledScalarOffsetNotRepresentable)?;
        let result_end = custody
            .result
            .code_offset
            .checked_add(custody.result.byte_count)
            .ok_or(InstallationError::InstalledScalarOffsetNotRepresentable)?;
        let result_home_is_exact = function
            .unit_scalar_homes
            .iter()
            .any(|home| home == &custody.result.home);
        let arguments_are_exact = custody.arguments.len() == target_abi.parameters.len()
            && custody
                .arguments
                .iter()
                .enumerate()
                .all(|(index, argument)| {
                    let Some(parameter) = target_abi.parameters.get(index) else {
                        return false;
                    };
                    let source_is_exact = match argument.source {
                    omega_machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                        defining_operation,
                        source_value,
                        scalar_type,
                        value,
                    } => function.unit_integer_constants.iter().any(|constant| {
                        constant.defining_operation == defining_operation
                            && constant.source_value == source_value
                            && constant.scalar_type == scalar_type
                            && constant.value == value
                            && constant.operation_ordinal < custody.operation_ordinal
                    }),
                    omega_machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) => {
                        function.unit_scalar_homes.iter().any(|candidate| candidate == &home)
                            && record.internal_unit_scalar_calls.iter().any(|prior| {
                                prior.machine == installed.machine
                                    && prior.custody.operation_ordinal < custody.operation_ordinal
                                    && prior.custody.result.home == home
                            })
                    }
                };
                    let argument_end = argument.code_offset.checked_add(argument.byte_count);
                    u32::try_from(index) == Ok(argument.parameter_index)
                        && argument.destination == parameter.placement
                        && argument.source.scalar_type() == parameter.scalar_type
                        && source_is_exact
                        && argument.byte_count != 0
                        && argument.code_offset >= custody.code_offset
                        && argument_end.is_some_and(|end| end <= call_end)
                });
        if previous_key.is_some_and(|previous| previous >= key)
            || installed.text_offset != expected_text_offset
            || custody.call_plan != target_abi.call_plan
            || custody.byte_count == 0
            || call_end > function.byte_count
            || custody.result.byte_count == 0
            || custody.result.code_offset < custody.code_offset
            || result_end > call_end
            || custody.result.home.defining_operation != owner
            || custody.result.home.scalar_type != target_abi.result.scalar_type
            || custody.result.source != target_abi.result.placement
            || !result_home_is_exact
            || !arguments_are_exact
        {
            return Err(InstallationError::InvalidInternalUnitScalarCall(
                installed.machine,
            ));
        }
        previous_key = Some(key);
    }
    Ok(())
}
