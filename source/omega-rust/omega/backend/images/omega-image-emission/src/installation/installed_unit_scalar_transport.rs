//! Installation-shape validation for retained fixed-integer ABI and attached-
//! Unit scalar-call transport. Native byte replay remains object-owned.

use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use omega_machine_code::SemanticCodeSite;
use omega_target::NativeTarget;
use omega_target_operations::CallSiteOwner;
use psi_core::MachineId;
use psi_terminal::StructuralAccess;

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
        .is_none_or(|abi| installed_fixed_integer_scalar_abi_is_canonical(abi, target));
    let mixed_abi_is_canonical = function
        .mixed_structural_scalar_abi
        .as_ref()
        .is_none_or(|abi| {
            installed_mixed_structural_scalar_abi_is_canonical(function, abi, target)
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
    abi_is_canonical && mixed_abi_is_canonical && homes_are_canonical && constants_are_canonical
}

pub(super) fn installed_mixed_structural_scalar_abi_is_canonical(
    function: &InstalledFunction,
    abi: &omega_target_operations::MixedStructuralScalarFunctionAbi,
    target: NativeTarget,
) -> bool {
    let Some(scalar_shapes) = abi
        .scalar_parameters
        .iter()
        .map(|parameter| fixed_integer_shape(parameter.scalar_type))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let Some(result_shape) = fixed_integer_shape(abi.result.scalar_type) else {
        return false;
    };
    let Ok(expected) = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_shapes
                .iter()
                .copied()
                .chain(
                    abi.structural_parameters
                        .iter()
                        .map(|parameter| parameter.shape),
                )
                .collect(),
            result: Some(result_shape),
        },
    ) else {
        return false;
    };
    let scalar_count = abi.scalar_parameters.len();
    let structural_count = abi.structural_parameters.len();
    scalar_count != 0
        && structural_count != 0
        && function.fixed_integer_scalar_abi.is_none()
        && function.scalar_stack.is_some()
        && function.unit_stack.is_none()
        && expected == abi.call_plan
        && abi.call_plan.parameters.len() == scalar_count + structural_count
        && abi.call_plan.result.as_ref() == Some(&abi.result.placement)
        && abi.result.placement.shape == result_shape
        && abi
            .scalar_parameters
            .iter()
            .zip(&scalar_shapes)
            .zip(&abi.call_plan.parameters[..scalar_count])
            .all(|((parameter, shape), placement)| {
                parameter.placement == *placement && placement.shape == *shape
            })
        && abi
            .structural_parameters
            .iter()
            .zip(&abi.call_plan.parameters[scalar_count..])
            .all(|(parameter, placement)| {
                parameter.placement == *placement && placement.shape == parameter.shape
            })
        && abi
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.value)
            .chain(std::iter::once(abi.result.value))
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == scalar_count + 1
        && abi
            .structural_parameters
            .iter()
            .map(|parameter| parameter.place)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == structural_count
}

pub(super) fn installed_fixed_integer_scalar_abi_is_canonical(
    abi: &omega_target_operations::FixedIntegerScalarFunctionAbi,
    target: NativeTarget,
) -> bool {
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
                        omega_machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
                            ..
                        } => false,
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
                                        && prior.custody.operation_ordinal
                                            < custody.operation_ordinal
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

pub(super) fn validate_installed_unit_structural_scalar_field_stores(
    record: &InstallationRecord,
    functions: &std::collections::BTreeMap<MachineId, &InstalledFunction>,
) -> Result<(), InstallationError> {
    for function in functions.values().copied() {
        let invalid = || InstallationError::InvalidUnitStructuralScalarFieldStore(function.machine);
        let mut previous = None;
        for store in &function.unit_structural_scalar_field_stores {
            let key = (store.operation_ordinal, store.code_offset);
            let parameter_index =
                usize::try_from(store.destination.position).map_err(|_| invalid())?;
            let parameter = function
                .unit_parameters
                .get(parameter_index)
                .ok_or_else(invalid)?;
            let home = function
                .unit_parameter_homes
                .get(parameter_index)
                .ok_or_else(invalid)?;
            let omega_machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                defining_operation,
                source_value,
                scalar_type,
                value,
            } = store.source
            else {
                return Err(invalid());
            };
            let source_count = function
                .unit_integer_constants
                .iter()
                .filter(|constant| {
                    constant.defining_operation == defining_operation
                        && constant.source_value == source_value
                        && constant.scalar_type == scalar_type
                        && constant.value == value
                        && constant.operation_ordinal < store.operation_ordinal
                })
                .count();
            let width = scalar_type.bits().checked_div(8).ok_or_else(invalid)?;
            let bits = crate::unit_structural_scalar_field_store::integer_bits(scalar_type, value)
                .ok_or_else(invalid)?;
            let expected_bytes = crate::unit_structural_scalar_field_store::expected_store_bytes(
                record.target,
                home,
                store.field_byte_offset,
                width,
                bits,
            )
            .ok_or_else(invalid)?;
            let end = store
                .code_offset
                .checked_add(store.byte_count)
                .ok_or_else(invalid)?;
            let exact_attribution_count = record
                .semantic_code_attribution
                .iter()
                .filter(|attribution| {
                    attribution.machine == function.machine
                        && attribution.attribution.site
                            == SemanticCodeSite::Operation(store.psi_operation)
                        && attribution.attribution.operation_ordinal == store.operation_ordinal
                        && attribution.attribution.code_offset == store.code_offset
                        && attribution.attribution.byte_count == store.byte_count
                })
                .count();
            if previous.is_some_and(|previous| previous >= key)
                || !store.destination.is_self
                || function.attachment != Some(store.destination.structural_type)
                || !matches!(
                    store.destination.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                )
                || store.path.is_empty()
                || parameter.place != store.destination.place
                || parameter.structural_type != store.destination.structural_type
                || parameter.multiplicity != store.destination.multiplicity
                || home.place != parameter.place
                || home.structural_type != parameter.structural_type
                || home.multiplicity != parameter.multiplicity
                || home.shape != parameter.shape
                || store.destination_placement != home.source
                || store.parameter_home_byte_offset != home.byte_offset
                || store.parameter_home_indirect != home.indirect
                || source_count != 1
                || !matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
                || scalar_type.is_address()
                || !scalar_type.admits(value)
                || store
                    .field_byte_offset
                    .checked_add(u32::from(width))
                    .is_none_or(|field_end| field_end > u32::from(parameter.shape.byte_size))
                || store.byte_count == 0
                || store.byte_count != store.bytes.len()
                || store.bytes != expected_bytes
                || end > function.byte_count
                || exact_attribution_count != 1
            {
                return Err(invalid());
            }
            previous = Some(key);
        }
    }
    Ok(())
}
