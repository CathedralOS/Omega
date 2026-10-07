//! The public builder entries and the object construction route: validate
//! every retained machine-code function (`function_validation`), size the
//! object (`layout`), emit `.text` (`text_emission`), then bind every
//! relocation record (`relocations`).

use crate::object_artifact::private_functions::validate_private_functions;
use crate::object_artifact::{ObjectArtifact, ObjectError};
use machine_code::{
    CompilerPrivateMachineCodeFunction, MachineCodePlan, MachineCodePlanWithPrivateFunctions,
};

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

pub(crate) fn build_object_artifact_with_x86_feature_profile(
    plan: &MachineCodePlan,
    private_functions: &[CompilerPrivateMachineCodeFunction],
    x86_feature_profile: Option<target::TargetProfile>,
    x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
) -> Result<ObjectArtifact, ObjectError> {
    if plan.functions.is_empty() {
        return Err(ObjectError::EmptyPlan);
    }
    let validated_private_functions = validate_private_functions(plan.target, private_functions)?;
    let validation = function_validation::validate_functions(
        plan,
        x86_feature_profile,
        x86_scalar_fma_provider,
    )?;
    let layout::ObjectLayout {
        mut object,
        text_size,
        foreign_call_count,
    } = layout::plan_object_layout(
        plan,
        private_functions,
        &validated_private_functions,
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
        // The machine-code route emits no initialized data.
        data_bytes: Vec::new(),
        functions,
        private_functions: object_private_functions,
        semantic_code_attribution,
        port_effects,
        boundary_settlements,
        foreign_calls,
        // The machine-code route carries no selected byte-literal addresses.
        byte_literal_pool: None,
        implementation_table_pool: None,
    })
}
