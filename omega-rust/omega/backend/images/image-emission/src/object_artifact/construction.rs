//! The public builder entries and the object construction route: validate
//! every retained machine-code function (`function_validation`), size the
//! object (`layout`), emit `.text` (`text_emission`) and `.data`
//! (`data_tables`), then bind every relocation record (`relocations`).

use crate::object_artifact::private_functions::validate_private_functions;
use crate::object_artifact::replay::dynamic::forwarded_descriptor::validate_forwarded_dynamic_descriptors;
use crate::object_artifact::replay::dynamic::forwarded_parameter::validate_forwarded_dynamic_parameter_calls;
use crate::object_artifact::{ObjectArtifact, ObjectError};
use machine_code::{
    CompilerPrivateMachineCodeFunction, MachineCodePlan, MachineCodePlanWithPrivateFunctions,
};

mod data_tables;
mod function_validation;
mod layout;
mod relocations;
pub(crate) mod text_emission;

/// Construct a self-contained object plan and exact text carrier.
///
/// Function order is semantic-artifact order and must already be canonical by
/// `MachineId`; this boundary rejects alternate ordering rather than silently
/// normalizing it. Each function gets exactly one symbol and one retained Psi
/// provenance row.
pub fn build_object_artifact(plan: &MachineCodePlan) -> Result<ObjectArtifact, ObjectError> {
    build_object_artifact_with_x86_feature_profile(plan, &[], None, None)
}

/// Construct an object that owns semantic program functions and a disjoint,
/// placement-identified compiler-private callback-function roster.
pub fn build_object_artifact_with_private_functions(
    plan: &MachineCodePlanWithPrivateFunctions,
) -> Result<ObjectArtifact, ObjectError> {
    build_object_artifact_with_x86_feature_profile(&plan.plan, &plan.private_functions, None, None)
}

/// Construct the bounded source-free object seam for feature-requiring scalar
/// x86 FMA. The profile is explicit because `NativeTarget` deliberately
/// collapses Windows and UEFI x86-64 physical layouts.
pub fn build_feature_required_x86_fma_object_artifact(
    plan: &MachineCodePlan,
    profile: target::TargetProfile,
) -> Result<ObjectArtifact, ObjectError> {
    if !plan
        .functions
        .iter()
        .any(|function| !function.x86_scalar_fma.is_empty())
    {
        return Err(ObjectError::MissingX86ScalarFmaFragment);
    }
    build_object_artifact_with_x86_feature_profile(plan, &[], Some(profile), None)
}

/// Consume exact deployment-feature and differential authority while building
/// an object whose generic F32/F64 FMA slots may enter executable emission.
/// Ordinary and feature-required-only builders retain their fail-closed
/// baseline behavior.
pub fn build_admitted_x86_fma_object_artifact(
    plan: &MachineCodePlan,
    provider: target::AdmittedX86ScalarFmaProvider,
) -> Result<ObjectArtifact, ObjectError> {
    if !provider.has_canonical_identity() {
        return Err(ObjectError::InvalidX86ScalarFmaProviderAdmission);
    }
    if provider.profile().native_target() != plan.target
        || plan
            .functions
            .iter()
            .flat_map(|function| &function.x86_scalar_fma)
            .any(|fragment| {
                let slot = match fragment.format {
                    machine_code::X86ScalarFmaFormat::Binary32 => {
                        target::X86ScalarFmaSlot::Binary32
                    }
                    machine_code::X86ScalarFmaFormat::Binary64 => {
                        target::X86ScalarFmaSlot::Binary64
                    }
                };
                !provider.admits(fragment.requirement, slot)
            })
    {
        return Err(ObjectError::InvalidX86ScalarFmaProviderAdmission);
    }
    if !plan
        .functions
        .iter()
        .any(|function| !function.x86_scalar_fma.is_empty())
    {
        return Err(ObjectError::MissingX86ScalarFmaFragment);
    }
    build_object_artifact_with_x86_feature_profile(
        plan,
        &[],
        Some(provider.profile()),
        Some(provider),
    )
}

pub(crate) fn same_dynamic_table_application(
    left: &terminal_psi::ClosedConformanceApplication,
    right: &terminal_psi::ClosedConformanceApplication,
) -> bool {
    left.commitment == right.commitment
        && left.declaration_identity == right.declaration_identity
        && left.telescope == right.telescope
        && left.subject_identity == right.subject_identity
        && left.trait_identity == right.trait_identity
        && left.trait_lifetime_arguments == right.trait_lifetime_arguments
        && left.trait_arguments == right.trait_arguments
        && left.realization_callables == right.realization_callables
        && left.rows == right.rows
        && left.report_fingerprint == right.report_fingerprint
}

pub(crate) fn build_object_artifact_with_x86_feature_profile(
    plan: &MachineCodePlan,
    private_functions: &[CompilerPrivateMachineCodeFunction],
    x86_feature_profile: Option<target::TargetProfile>,
    x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
) -> Result<ObjectArtifact, ObjectError> {
    if plan.functions.is_empty() {
        return Err(ObjectError::EmptyPlan);
    }
    let forwarded_dynamic_applications =
        validate_forwarded_dynamic_descriptors(plan.target, &plan.functions)?;
    validate_forwarded_dynamic_parameter_calls(plan.target, &plan.functions)?;
    let validated_private_functions = validate_private_functions(plan.target, private_functions)?;
    let validation = function_validation::validate_functions(
        plan,
        x86_feature_profile,
        x86_scalar_fma_provider,
    )?;
    let layout::ObjectLayout {
        mut object,
        dynamic_applications,
        text_size,
        dynamic_data_size,
        foreign_call_count,
    } = layout::plan_object_layout(
        plan,
        private_functions,
        &validated_private_functions,
        &forwarded_dynamic_applications,
        validation.text_size,
    )?;

    let mut text_bytes = Vec::with_capacity(text_size);
    let emitted = text_emission::emit_functions(
        plan,
        validation,
        foreign_call_count,
        &mut object,
        &mut text_bytes,
    )?;
    let adapters = text_emission::emit_forwarded_adapters(
        &forwarded_dynamic_applications,
        &emitted.symbols_by_machine,
        &mut object,
        &mut text_bytes,
    )?;

    let mut data_bytes = Vec::with_capacity(dynamic_data_size);
    let dynamic_conformance_tables = data_tables::emit_dynamic_conformance_tables(
        dynamic_applications,
        &emitted.symbols_by_machine,
        &mut object,
        &mut data_bytes,
    )?;
    let forwarded_dynamic_descriptor_tables = data_tables::emit_forwarded_descriptor_tables(
        &forwarded_dynamic_applications,
        &adapters.symbols,
        &mut object,
        &mut data_bytes,
    )?;

    let object_private_functions = text_emission::emit_private_functions(
        validated_private_functions,
        &mut object,
        &mut text_bytes,
    )?;

    let import_symbols = relocations::declare_foreign_imports(plan, &mut object)?;
    let relocations = relocations::plan_relocations(relocations::RelocationInputs {
        plan,
        object: &object,
        functions: &emitted.functions,
        symbols_by_machine: &emitted.symbols_by_machine,
        private_functions: &object_private_functions,
        dynamic_conformance_tables: &dynamic_conformance_tables,
        forwarded_dynamic_descriptor_tables: &forwarded_dynamic_descriptor_tables,
        forwarded_dynamic_descriptor_adapters: &adapters.records,
        import_symbols: &import_symbols,
    })?;

    let text_emission::EmittedFunctions {
        functions,
        symbols_by_machine: _,
        semantic_code_attribution,
        port_effects,
        boundary_settlements,
        foreign_calls,
    } = emitted;
    Ok(ObjectArtifact {
        hosted_receiver: None,
        requires_graph_storage_replay: false,
        fragment_replay: None,
        psi: plan.psi,
        target: plan.target,
        x86_feature_profile,
        x86_scalar_fma_provider,
        entry: plan.entry,
        object,
        relocations,
        text_bytes,
        data_bytes,
        dynamic_conformance_tables,
        forwarded_dynamic_descriptor_adapters: adapters.records,
        forwarded_dynamic_descriptor_tables,
        functions,
        private_functions: object_private_functions,
        semantic_code_attribution,
        port_effects,
        boundary_settlements,
        foreign_calls,
    })
}
