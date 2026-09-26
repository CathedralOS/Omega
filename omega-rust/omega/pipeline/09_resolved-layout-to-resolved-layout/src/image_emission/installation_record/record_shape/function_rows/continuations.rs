//! The Unit continuations one installed function row may carry.

use crate::image_emission::installation_record::{
    InstallationError, InstallationRecord, InstalledFunction,
};

/// The row's Unit continuations are well formed: entry register spills fit
/// the frame, the continuations and cleanup are attributed exactly, a
/// function with continuations is a Unit body without scalar custody, and
/// only a function with continuations records entry spills.
pub(super) fn validate_continuation_shape(
    record: &InstallationRecord,
    function: &InstalledFunction,
    function_unit_calls: &[post_allocation_machine_to_selected_form_encoding::machine_code::InternalUnitCallRecord],
) -> Result<(), InstallationError> {
    if !function.unit_continuations.is_empty()
        && crate::image_emission::object_artifact::replay::unit::scalar_call_custody::entry_spills::validate_shape(
            record.target,
            &function.unit_parameter_homes,
            function.parameter_abi.as_ref(),
            true,
            function
                .unit_stack
                .as_ref()
                .map_or(0, |stack| stack.frame_bytes),
        )
        .is_none_or(|end| {
            function.unit_stack.as_ref().is_none_or(|stack| {
                end > stack.frame_bytes
                    || (function_unit_calls
                        .iter()
                        .all(|call| call.structural_result.is_none())
                        && !crate::image_emission::object_artifact::replay::unit::call_custody::result_home::exact_frame(
                            record.target,
                            end,
                            stack.frame_bytes,
                            None,
                        ))
            })
        })
    {
        return Err(InstallationError::InvalidUnitAffineCleanup(
            function.machine,
        ));
    }
    if function.unit_body
        && !crate::image_emission::object_artifact::replay::unit::continuations::exact_attribution(
            &function.unit_continuations,
            function.unit_affine_cleanup.as_ref(),
            function_unit_calls.len(),
            &record
                .semantic_code_attribution
                .iter()
                .filter(|row| row.machine == function.machine)
                .map(|row| row.attribution)
                .collect::<Vec<_>>(),
        )
    {
        return Err(InstallationError::InvalidUnitAffineCleanup(
            function.machine,
        ));
    }
    if !function.unit_continuations.is_empty()
        && (!function.unit_body
            || function.scalar_stack.is_some()
            || function.scalar_abi.is_some()
            || function.scalar_affine_cleanup.is_some()
            || !function.scalar_control_affine_cleanups.is_empty()
            || function.structural_call_scalar_return.is_some()
            || !crate::image_emission::object_artifact::replay::unit::continuations::exact_scalar_bindings(
                function.parameter_abi.as_ref(),
                &function.unit_continuations,
            )
            || !function.unit_scalar_homes.is_empty()
            || !function.unit_integer_constants.is_empty()
            || !function.unit_affine_scalar_records.is_empty()
            || !function.unit_structural_scalar_field_stores.is_empty()
            || !function.unit_write_only_primitive_stores.is_empty()
            || record
                .boundary_settlements
                .iter()
                .any(|settlement| settlement.machine == function.machine))
    {
        return Err(InstallationError::InvalidUnitAffineCleanup(
            function.machine,
        ));
    }
    if function.unit_continuations.is_empty()
        && function
            .parameter_abi
            .as_ref()
            .is_some_and(|abi| !abi.entry_register_spills.is_empty())
    {
        return Err(InstallationError::InvalidUnitAffineCleanup(
            function.machine,
        ));
    }
    Ok(())
}
