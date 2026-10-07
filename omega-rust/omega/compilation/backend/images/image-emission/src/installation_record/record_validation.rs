//! Validating a decoded installation record against the final image: placed
//! regions, installed calls, private functions, foreign call stacks and
//! scalar sources.

use crate::ExecutableImage;
use crate::installation_record::codec::fingerprint::{
    fingerprint_image, fingerprint_initialized_data, fingerprint_record,
};
use crate::installation_record::record_shape::validate_record_shape;
use crate::installation_record::{
    InstallationError, InstallationFingerprint, InstallationRecord,
    InstalledCompilerPrivateFunction, InstalledForeignCallStack, InstalledFunction,
    InstalledImageSections, InstalledInternalUnitCall, InstalledInternalUnitScalarCall,
    InstalledStructuralReturn, encode_installation_record,
};
use semantic_vocabulary::MachineId;
use target::ObjectFormat;

/// Independently replay the image's retained placed-region inventories over
/// its exact final bytes. A record only claims the sealed inventory digests;
/// this replay proves the retained rows actually classify every byte the
/// image says it installed, that no byte remains unclassified, and that each
/// Mach-O import thunk pairs with its placed binding slot.
fn validate_complete_image_placement(image: &ExecutableImage) -> Result<(), InstallationError> {
    let output = image.output();
    image::validate_placed_executable_region_inventory(
        &output.executable_regions,
        &output.final_text_bytes,
    )
    .map_err(|_| InstallationError::InvalidImagePlacementCustody)?;
    image::validate_placed_data_region_inventory(&output.data_regions, &output.final_data_bytes)
        .map_err(|_| InstallationError::InvalidImagePlacementCustody)?;
    if !output.executable_regions.unclassified_gaps.is_empty()
        || !output.data_regions.unclassified_gaps.is_empty()
    {
        return Err(InstallationError::InvalidImagePlacementCustody);
    }
    if output.final_text_bytes.len() != output.executable_regions.text_byte_count
        || output.final_data_bytes.len() != output.data_regions.data_byte_count
        || output.final_image_layout.text_address != output.executable_regions.text_address
        || output.final_image_layout.data_address != output.data_regions.data_address
    {
        return Err(InstallationError::InvalidImagePlacementCustody);
    }
    if image.target().object_format == ObjectFormat::MachO {
        match image.target().architecture {
            target::Architecture::Aarch64 => {
                image_macho::validate_macho_aarch64_import_binding_pairing(
                    &output.final_text_bytes,
                    &output.executable_regions,
                    &output.data_regions,
                )
                .map_err(|_| InstallationError::InvalidImagePlacementCustody)?;
            }
            target::Architecture::X86_64 => {
                image_macho::validate_macho_x86_64_import_binding_pairing(
                    &output.final_text_bytes,
                    &output.executable_regions,
                    &output.data_regions,
                )
                .map_err(|_| InstallationError::InvalidImagePlacementCustody)?;
            }
        }
    }
    Ok(())
}

pub fn validate_installation_record(
    record: &InstallationRecord,
    image: &ExecutableImage,
) -> Result<(), InstallationError> {
    validate_record_shape(record)?;
    validate_complete_image_placement(image)?;
    let expected_private_functions = image
        .private_functions()
        .iter()
        .map(installed_compiler_private_function)
        .collect::<Result<Vec<_>, _>>()?;
    if record.psi != image.psi()
        || record.target != image.target()
        || record.subsystem != image.subsystem()
        || record.image != fingerprint_image(&image.output().bytes)
        || record.image_sections != installed_image_sections(image)
        || Some(record.compiler_text_validation) != image.output().compiler_text_validation
        || record.semantic_code_attribution != image.semantic_code_attribution()
        || record.port_effects != image.port_effects()
        || record.boundary_settlements != image.boundary_settlements()
        || record.private_functions != expected_private_functions
        || record.structural_returns
            != image
                .functions()
                .iter()
                .filter_map(|function| {
                    function
                        .structural_return
                        .clone()
                        .map(|returned| InstalledStructuralReturn {
                            machine: function.machine,
                            returned,
                        })
                })
                .collect::<Vec<_>>()
        || !internal_unit_calls_match_object(
            &record.internal_unit_calls,
            image.functions().iter().flat_map(|function| {
                // The record retains physical text order while the object's
                // roster follows the selected source block order.
                let mut calls: Vec<_> = function.internal_unit_calls.iter().collect();
                calls.sort_by_key(|custody| custody.code_offset);
                calls
                    .into_iter()
                    .map(move |custody| (function.machine, function.text_offset, custody))
            }),
        )
        || record.internal_unit_scalar_calls
            != image
                .functions()
                .iter()
                .flat_map(|function| {
                    function
                        .internal_unit_scalar_calls
                        .iter()
                        .cloned()
                        .map(|custody| InstalledInternalUnitScalarCall {
                            machine: function.machine,
                            text_offset: function.text_offset + custody.code_offset,
                            custody,
                        })
                })
                .collect::<Vec<_>>()
        || record.functions.len() != image.functions().len()
        || record
            .functions
            .iter()
            .zip(image.functions())
            .any(|(installed, emitted)| {
                installed.machine != emitted.machine
                    || (installed.attachment != emitted.attachment
                        || installed.nominal_cleanup_subject != emitted.nominal_cleanup_subject)
                    || installed.scalar_abi != emitted.scalar_abi
                    || installed.mixed_structural_scalar_abi != emitted.mixed_structural_scalar_abi
                    || installed.parameter_abi != emitted.parameter_abi
                    || installed.structural_call_scalar_return
                        != emitted.structural_call_scalar_return
                    || installed.text_offset != emitted.text_offset
                    || installed.byte_count != emitted.byte_count
                    || installed.unit_stack != emitted.unit_stack
                    || installed.scalar_stack != emitted.scalar_stack
                    || installed.unit_call_stacks != emitted.unit_call_stacks
                    || installed.scalar_call_stacks != emitted.scalar_call_stacks
                    || installed.foreign_call_stacks
                        != installed_foreign_call_stacks(image, emitted.machine)
                    || installed.unit_body != emitted.unit_affine_cleanup.is_some()
                    || installed.unit_parameters != emitted.unit_parameters
                    || installed.unit_parameter_homes != emitted.unit_parameter_homes
                    || installed.unit_scalar_homes != emitted.unit_scalar_homes
                    || installed.unit_integer_constants != emitted.unit_integer_constants
                    || installed.unit_affine_scalar_records != emitted.unit_affine_scalar_records
                    || installed.unit_structural_scalar_field_stores
                        != emitted.unit_structural_scalar_field_stores
                    || installed.unit_write_only_primitive_stores
                        != emitted.unit_write_only_primitive_stores
                    || installed.scalar_structural_scalar_field_stores
                        != emitted.scalar_structural_scalar_field_stores
                    || installed.unit_continuations != emitted.unit_continuations
                    || installed.unit_affine_cleanup != emitted.unit_affine_cleanup
                    || installed.scalar_affine_cleanup != emitted.scalar_affine_cleanup
                    || !installed_scalar_control_cleanups_match_object(
                        &installed.scalar_control_affine_cleanups,
                        &emitted.scalar_control_affine_cleanups,
                    )
                    || installed.scalar_structural_parameters
                        != emitted.scalar_structural_parameters
                    || installed.scalar_structural_parameter_homes
                        != emitted.scalar_structural_parameter_homes
            })
    {
        return Err(InstallationError::ImageBindingMismatch);
    }
    Ok(())
}

/// Joins every custody field, including owned copy spans and bytes, to replayed
/// object records. Candidate ABI geometry is not executable admission.
pub(crate) fn internal_unit_calls_match_object<'call>(
    installed: &[InstalledInternalUnitCall],
    emitted: impl IntoIterator<
        Item = (
            MachineId,
            usize,
            &'call machine_code::InternalUnitCallRecord,
        ),
    >,
) -> bool {
    let mut installed = installed.iter();
    for (machine, function_text_offset, custody) in emitted {
        let Some(actual) = installed.next() else {
            return false;
        };
        if actual.machine != machine
            || Some(actual.text_offset) != function_text_offset.checked_add(custody.code_offset)
            || actual.custody != *custody
        {
            return false;
        }
    }
    installed.next().is_none()
}

// Selected calls follow the source block roster while physical layout may
// reorder blocks; the published record retains physical call order.

pub(crate) fn installed_internal_unit_calls(
    image: &ExecutableImage,
) -> Vec<InstalledInternalUnitCall> {
    image
        .functions()
        .iter()
        .flat_map(|function| {
            let mut calls = function.internal_unit_calls.to_vec();
            calls.sort_by_key(|custody| custody.code_offset);
            calls.into_iter().map(|custody| InstalledInternalUnitCall {
                machine: function.machine,
                text_offset: function.text_offset + custody.code_offset,
                custody,
            })
        })
        .collect()
}

pub(crate) fn installed_image_sections(image: &ExecutableImage) -> InstalledImageSections {
    let text_byte_count = image
        .functions()
        .iter()
        .map(|function| {
            function
                .text_offset
                .checked_add(function.byte_count)
                .expect("validated compiler function text extent")
        })
        .chain(image.private_functions().iter().map(|private| {
            private
                .function
                .text_offset
                .checked_add(private.function.byte_count)
                .expect("validated compiler-private function text extent")
        }))
        .max()
        .unwrap_or(0);
    // The compiler authors no initialized data; every final data byte belongs
    // to the image writer.
    let data_byte_count = 0;
    let final_compiler_data = image
        .output()
        .final_data_bytes
        .get(..data_byte_count)
        .expect("validated image retains its compiler-authored initialized-data prefix");
    InstalledImageSections {
        layout: image.output().final_image_layout,
        text_byte_count,
        data_byte_count,
        final_data_fingerprint: fingerprint_initialized_data(final_compiler_data),
        final_text_byte_count: image.output().final_text_bytes.len(),
        final_data_byte_count: image.output().final_data_bytes.len(),
        executable_inventory_digest: image.output().executable_regions.inventory_digest,
        data_inventory_digest: image.output().data_regions.inventory_digest,
    }
}

pub(crate) fn installed_compiler_private_function(
    emitted: &crate::ObjectCompilerPrivateFunction,
) -> Result<InstalledCompilerPrivateFunction, InstallationError> {
    Ok(InstalledCompilerPrivateFunction {
        identity: emitted.identity,
        source_psi: emitted.source_psi,
        machine: emitted.function.machine,
        scalar_abi: emitted
            .function
            .scalar_abi
            .clone()
            .ok_or(InstallationError::MissingCompilerPrivateFunctionAbi)?,
        text_offset: emitted.function.text_offset,
        byte_count: emitted.function.byte_count,
    })
}

pub(crate) fn installed_scalar_control_cleanups_match_object(
    installed: &[machine_code::UnitAffineCleanupRecord],
    emitted: &[machine_code::ScalarControlAffineCleanupRecord],
) -> bool {
    installed.len() == emitted.len()
        && installed
            .iter()
            .zip(emitted)
            .all(|(installed, emitted)| installed == &emitted.cleanup)
}

pub(crate) fn installed_foreign_call_stacks(
    image: &ExecutableImage,
    machine: MachineId,
) -> Vec<InstalledForeignCallStack> {
    image
        .foreign_calls()
        .iter()
        .filter(|call| call.machine == machine)
        .filter_map(|call| {
            call.binding
                .import()
                .map(|(_, contribution)| InstalledForeignCallStack {
                    owner: call.owner,
                    text_offset: call.text_offset,
                    caller_live_bytes: call.caller_live_bytes,
                    provider_plan_report_identity: contribution.provider_plan_report_identity(),
                    contribution_report_identity: contribution.report_identity(),
                    contribution_commitment: contribution.commitment(),
                    contribution_bytes: contribution.bytes(),
                    contribution_alignment: contribution.alignment(),
                })
        })
        .collect()
}

pub fn installation_fingerprint(
    record: &InstallationRecord,
) -> Result<InstallationFingerprint, InstallationError> {
    let bytes = encode_installation_record(record)?;
    Ok(fingerprint_record(&bytes))
}

pub(crate) fn installed_scalar_source_is_exact(
    record: &InstallationRecord,
    function: &InstalledFunction,
    machine: MachineId,
    consumer: &machine_code::InternalUnitCallRecord,
    source: machine_code::InternalUnitScalarArgumentSourceRecord,
) -> bool {
    match source {
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary { .. }
        | machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit { .. }
        | machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall { .. } => false,
        machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location,
        } => usize::try_from(parameter_index)
            .ok()
            .and_then(|index| {
                function
                    .parameter_abi
                    .as_ref()
                    .and_then(|abi| abi.parameters.get(index))
            })
            .is_some_and(|parameter| {
                let expected_location = function.parameter_abi.as_ref().and_then(|abi| {
                    crate::object_artifact::replay::unit::scalar_call_custody::entry_spills::parameter_location(
                        abi,
                        usize::try_from(parameter_index).ok()?,
                        source_value,
                        consumer.code_offset,
                    )
                });
                parameter.value == source_value
                    && parameter.scalar_type == scalar_type
                    && expected_location == Some(location)
            }),
        machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            function
                .unit_integer_constants
                .iter()
                .filter(|constant| {
                    constant.defining_operation == defining_operation
                        && constant.source_value == source_value
                        && constant.scalar_type == scalar_type
                        && constant.value == value
                        && constant.operation_ordinal < consumer.operation_ordinal
                })
                .count()
                == 1
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
            defining_operation,
            source_value,
            value,
            definition_ordinal,
        } => {
            definition_ordinal < consumer.operation_ordinal
                && record
                    .semantic_code_attribution
                    .iter()
                    .filter(|attribution| {
                        attribution.machine == machine
                            && attribution.attribution.site
                                == machine_code::SemanticCodeSite::Operation(defining_operation)
                            && attribution.attribution.operation_ordinal == definition_ordinal
                            && attribution.attribution.code_offset <= consumer.code_offset
                            && attribution.attribution.byte_count == 0
                    })
                    .count()
                    == 1
                && function.unit_integer_constants.iter().all(|constant| {
                    constant.defining_operation != defining_operation
                        && constant.source_value != source_value
                })
                && function.unit_scalar_homes.iter().all(|home| {
                    home.defining_operation != defining_operation
                        && home.source_value != source_value
                })
                && installed_boolean_call_source_is_consistent(
                    record,
                    machine,
                    defining_operation,
                    source_value,
                    value,
                    definition_ordinal,
                )
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) => {
            function
                .unit_scalar_homes
                .iter()
                .filter(|candidate| **candidate == home)
                .count()
                == 1
                && record
                    .internal_unit_scalar_calls
                    .iter()
                    .filter(|producer| {
                        producer.machine == machine
                            && producer.custody.result.home == home
                            && producer.custody.operation_ordinal < consumer.operation_ordinal
                            && producer
                                .custody
                                .result
                                .code_offset
                                .checked_add(producer.custody.result.byte_count)
                                .is_some_and(|producer_end| producer_end <= consumer.code_offset)
                    })
                    .count()
                    == 1
        }
    }
}

fn installed_boolean_call_source_is_consistent(
    record: &InstallationRecord,
    machine: MachineId,
    defining_operation: semantic_vocabulary::OperationId,
    source_value: semantic_vocabulary::ValueId,
    value: bool,
    definition_ordinal: usize,
) -> bool {
    record
        .internal_unit_calls
        .iter()
        .filter(|call| call.machine == machine)
        .flat_map(|call| &call.custody.scalar_arguments)
        .all(|argument| match argument.source {
            machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
                defining_operation: candidate_operation,
                source_value: candidate_value,
                value: candidate_literal,
                definition_ordinal: candidate_ordinal,
            } if candidate_operation == defining_operation || candidate_value == source_value => {
                candidate_operation == defining_operation
                    && candidate_value == source_value
                    && candidate_literal == value
                    && candidate_ordinal == definition_ordinal
            }
            _ => true,
        })
}
