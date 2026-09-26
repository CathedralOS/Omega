//! Shape validation for a decoded installation record: every retained row
//! must be canonical and self-consistent before the record may be compared
//! against object or image evidence. `validate_record_shape` checks the
//! record-wide preconditions, then walks each row family in order through
//! the owner beside it.

use super::{
    InstallationError, InstallationRecord, InstalledFunction, MachineId, ObjectFormat,
    can_emit_executable_image, semantic_code_attribution,
    validate_installed_unit_dynamic_descriptor_joins, validate_installed_unit_scalar_calls,
    validate_installed_unit_structural_scalar_field_stores,
    validate_installed_unit_write_only_primitive_stores,
};

mod affine_cleanups;
mod boundary_settlements;
mod dynamic_conformance;
mod function_rows;
mod internal_unit_calls;
mod stack_facts;
mod structural_returns;

#[cfg(test)]
pub(super) use affine_cleanups::scalar_control_affine_cleanups_are_canonical;
#[cfg(test)]
pub(super) use stack_facts::{installed_stack_facts_are_canonical, is_partial_cleanup_path};

pub(super) fn validate_record_shape(record: &InstallationRecord) -> Result<(), InstallationError> {
    validate_record_preconditions(record)?;
    function_rows::validate_function_rows(record)?;
    let function_by_machine = record
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    validate_installed_unit_scalar_calls(record, &function_by_machine)?;
    validate_installed_unit_structural_scalar_field_stores(record, &function_by_machine)?;
    validate_installed_unit_write_only_primitive_stores(record, &function_by_machine)?;
    structural_returns::validate_structural_returns(record, &function_by_machine)?;
    internal_unit_calls::validate_internal_unit_calls(record, &function_by_machine)?;
    validate_semantic_code_attribution(record, &function_by_machine)?;
    validate_port_effects(record, &function_by_machine)?;
    boundary_settlements::validate_boundary_settlements(record, &function_by_machine)?;
    Ok(())
}

/// Record-wide preconditions: a valid derivation digest, an emittable target,
/// the dynamic tables and descriptor joins, the subsystem the object format
/// requires, and a selected provider-plan set that closes over every
/// settlement and foreign call the record retains.
fn validate_record_preconditions(record: &InstallationRecord) -> Result<(), InstallationError> {
    if !record
        .compiler_text_validation
        .has_valid_derivation_digest()
    {
        return Err(InstallationError::InvalidCompilerTextDerivationDigest);
    }
    if !can_emit_executable_image(record.target) {
        return Err(InstallationError::UnsupportedTarget(record.target));
    }
    dynamic_conformance::validate_installed_dynamic_conformance(record)?;
    validate_installed_unit_dynamic_descriptor_joins(record)?;
    match record.target.object_format {
        ObjectFormat::Coff if record.subsystem.is_none() => {
            return Err(InstallationError::MissingCoffSubsystem);
        }
        ObjectFormat::Elf | ObjectFormat::MachO if record.subsystem.is_some() => {
            return Err(InstallationError::UnexpectedSubsystem);
        }
        ObjectFormat::Coff | ObjectFormat::Elf | ObjectFormat::MachO => {}
    }
    if record
        .selected_provider_plans
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(InstallationError::NonCanonicalProviderPlanOrder);
    }
    let selected = record
        .selected_provider_plans
        .iter()
        .map(|provider| provider.get())
        .collect::<std::collections::BTreeSet<_>>();
    let required = record
        .boundary_settlements
        .iter()
        .filter_map(|settlement| {
            let machine_code::BoundaryExecutionRecord::AdmittedProvider(execution) =
                settlement.settlement.execution
            else {
                return None;
            };
            Some(execution.provider_plan_report_identity)
        })
        .chain(
            record
                .functions
                .iter()
                .flat_map(|function| &function.foreign_call_stacks)
                .map(|call| call.provider_plan_report_identity),
        )
        .collect::<std::collections::BTreeSet<_>>();
    if !required.is_subset(&selected) {
        return Err(InstallationError::ProviderSettlementClosureMismatch);
    }
    Ok(())
}

/// Semantic code attribution rows are ordered and sit inside the text span
/// of the function they name.
fn validate_semantic_code_attribution(
    record: &InstallationRecord,
    function_by_machine: &std::collections::BTreeMap<MachineId, &InstalledFunction>,
) -> Result<(), InstallationError> {
    semantic_code_attribution::validate_order(&record.semantic_code_attribution)?;
    for installed in &record.semantic_code_attribution {
        let function = function_by_machine.get(&installed.machine).ok_or(
            InstallationError::SemanticCodeAttributionMachineMissing(installed.machine),
        )?;
        let expected = function
            .text_offset
            .checked_add(installed.attribution.code_offset)
            .ok_or(InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?;
        let end = installed
            .attribution
            .code_offset
            .checked_add(installed.attribution.byte_count)
            .ok_or(InstallationError::SemanticCodeAttributionOffsetNotRepresentable)?;
        if installed.text_offset != expected || end > function.byte_count {
            return Err(InstallationError::InvalidSemanticCodeAttribution {
                machine: installed.machine,
                site: installed.attribution.site,
            });
        }
    }
    Ok(())
}

/// Port effect rows are ordered, unique per operation, and sit inside the
/// text span of the function they name.
fn validate_port_effects(
    record: &InstallationRecord,
    function_by_machine: &std::collections::BTreeMap<MachineId, &InstalledFunction>,
) -> Result<(), InstallationError> {
    let mut previous_port = None;
    let mut port_operations = std::collections::BTreeSet::new();
    for installed in &record.port_effects {
        let function = function_by_machine
            .get(&installed.machine)
            .ok_or(InstallationError::EffectMachineMissing(installed.machine))?;
        let expected = function
            .text_offset
            .checked_add(installed.effect.code_offset)
            .ok_or(InstallationError::PortEffectOffsetNotRepresentable)?;
        let end = installed
            .effect
            .code_offset
            .checked_add(installed.effect.byte_count)
            .ok_or(InstallationError::PortEffectOffsetNotRepresentable)?;
        if installed.text_offset != expected
            || end > function.byte_count
            || installed.effect.byte_count
                != x86_encoding::encode_immediate_port_write(
                    installed.effect.port,
                    installed.effect.value,
                )
                .len()
        {
            return Err(InstallationError::InvalidPortEffectOffset {
                machine: installed.machine,
                operation: installed.effect.psi_operation,
            });
        }
        let key = (
            installed.machine,
            installed.text_offset,
            installed.effect.operation_ordinal,
        );
        if previous_port.is_some_and(|previous| previous >= key) {
            return Err(InstallationError::NonCanonicalPortEffectOrder);
        }
        if !port_operations.insert((installed.machine, installed.effect.psi_operation)) {
            return Err(InstallationError::DuplicatePortEffectOperation {
                machine: installed.machine,
                operation: installed.effect.psi_operation,
            });
        }
        previous_port = Some(key);
    }
    Ok(())
}
