//! The installed function rows: each function's retained facts are canonical
//! and its text span follows the previous function's, then the forwarded
//! descriptor adapters and the compiler-private function continue the text,
//! which must end exactly at the recorded text size.
//!
//! `validate_function_row` checks one row's site and hands its
//! continuations to `continuations`, its scalar custody facts to
//! `scalar_custody`, and its Unit affine cleanup to `affine_cleanup_shape`.

mod affine_cleanup_shape;
mod continuations;
mod scalar_custody;

use super::affine_cleanups::{
    validate_scalar_affine_cleanup_shape, validate_scalar_control_affine_cleanup_shape,
};
use crate::image_emission::installation_record::{
    InstallationError, InstallationRecord, InstalledFunction, MachineId, StructuralTypeId,
    installed_unit_scalar_transport,
};

/// Walk the functions in canonical order, then the adapters and the private
/// function, carrying the expected text offset across all of them.
pub(super) fn validate_function_rows(record: &InstallationRecord) -> Result<(), InstallationError> {
    if record.functions.is_empty() {
        return Err(InstallationError::NoInstalledFunctions);
    }
    let mut expected_text_offset = 0_usize;
    let mut previous_function = None;
    let attachments = record
        .functions
        .iter()
        .map(|function| (function.machine, function.attachment))
        .collect::<std::collections::BTreeMap<_, _>>();
    for function in &record.functions {
        validate_function_row(
            record,
            function,
            previous_function,
            expected_text_offset,
            &attachments,
        )?;
        expected_text_offset = expected_text_offset
            .checked_add(function.byte_count)
            .ok_or(InstallationError::FunctionOffsetNotRepresentable)?;
        previous_function = Some(function.machine);
    }
    let mut adapter_identities = std::collections::BTreeSet::new();
    for adapter in &record.forwarded_dynamic_descriptor_adapters {
        if adapter.application_commitment.is_zero()
            || adapter.byte_count == 0
            || adapter.text_offset != expected_text_offset
            || !adapter_identities.insert((
                adapter.application_commitment,
                adapter.row_index,
                adapter.realization,
            ))
        {
            return Err(InstallationError::InvalidForwardedDynamicDescriptorAdapter);
        }
        expected_text_offset = expected_text_offset
            .checked_add(adapter.byte_count)
            .ok_or(InstallationError::FunctionOffsetNotRepresentable)?;
    }
    if record.private_functions.len() > 1 {
        return Err(InstallationError::TooManyCompilerPrivateFunctions);
    }
    for private in &record.private_functions {
        if private.identity.callback_thunk_placement_index().is_none()
            || !private.identity.is_valid()
            || private.byte_count == 0
            || private.text_offset != expected_text_offset
            || !installed_unit_scalar_transport::installed_scalar_abi_is_canonical(
                &private.scalar_abi,
                record.target,
            )
        {
            return Err(InstallationError::InvalidCompilerPrivateFunction);
        }
        expected_text_offset = expected_text_offset
            .checked_add(private.byte_count)
            .ok_or(InstallationError::CompilerPrivateFunctionOffsetNotRepresentable)?;
    }
    if expected_text_offset != record.image_sections.text_byte_count {
        return Err(InstallationError::InvalidImageSectionLayout);
    }
    Ok(())
}

/// One installed function: continuation and Unit-body custody, canonical
/// text placement, stack facts and parameter homes, then the affine cleanups
/// it retains.
fn validate_function_row(
    record: &InstallationRecord,
    function: &InstalledFunction,
    previous_function: Option<MachineId>,
    expected_text_offset: usize,
    attachments: &std::collections::BTreeMap<MachineId, Option<StructuralTypeId>>,
) -> Result<(), InstallationError> {
    let function_unit_calls = record
        .internal_unit_calls
        .iter()
        .filter(|call| call.machine == function.machine)
        .map(|call| call.custody.clone())
        .collect::<Vec<_>>();
    let continuation_discards =
        crate::image_emission::object_artifact::replay::unit::continuations::completed_roots(
            &function.unit_parameter_homes,
            &function_unit_calls,
            &function.unit_continuations,
            function.unit_affine_cleanup.as_ref(),
        )
        .ok_or(InstallationError::InvalidUnitAffineCleanup(
            function.machine,
        ))?;
    continuations::validate_continuation_shape(record, function, &function_unit_calls)?;
    if function.byte_count == 0
        || function.text_offset != expected_text_offset
        || previous_function.is_some_and(|previous| previous >= function.machine)
    {
        return Err(InstallationError::NonCanonicalInstalledFunctions);
    }
    let custody = scalar_custody::scalar_custody_facts(record, function, &function_unit_calls);
    if scalar_custody::row_facts_disagree(record, function, attachments, &custody) {
        return Err(InstallationError::InvalidUnitAffineCleanup(
            function.machine,
        ));
    }
    if let Some(cleanup) = &function.unit_affine_cleanup {
        affine_cleanup_shape::validate_unit_affine_cleanup_shape(
            record,
            function,
            cleanup,
            attachments,
            &function_unit_calls,
            &continuation_discards,
        )?;
    }
    if let Some(cleanup) = &function.scalar_affine_cleanup {
        validate_scalar_affine_cleanup_shape(record, function, cleanup, true)?;
    }
    if custody.has_scalar_control_cleanup {
        validate_scalar_control_affine_cleanup_shape(record, function)?;
    }
    Ok(())
}
