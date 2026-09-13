//! Product evidence joins for the shared fragment-to-object publication path.

use diagnostics::Diagnostic;

pub(super) struct OptimizedFragmentPublicationRequest<'request> {
    pub(super) hosted_receiver: Option<crate::NativeProgramEntrySettlement<'request>>,
    pub(super) boundary_application_coverage:
        Option<&'request boundary_applications::TerminalBoundaryApplicationCoverage>,
    pub(super) optimized_plan: &'request abstract_operations::AbstractOperationPlan,
    pub(super) terminal: terminal_psi::TerminalPsiIdentity,
    pub(super) validation: optimization_core::OptimizedAbstractPlanProjectionIdentity,
    pub(super) final_unit: optimization_core::OptimizationUnitIdentity,
}

pub(super) fn emit_optimized_fragments(
    physical: crate::StagedOptimizedVerifiedPhysicalPipeline,
    request: OptimizedFragmentPublicationRequest<'_>,
) -> Result<
    (
        image_emission::ObjectArtifact,
        native_artifact::NativePhysicalEvidenceScope,
    ),
    Vec<Diagnostic>,
> {
    let source = std::sync::Arc::new(stage_fragment_object(physical)?);
    let mut object =
        image_emission::build_function_fragment_object_artifact(std::sync::Arc::clone(&source))
            .map_err(|error| {
                super::diagnostics::realization_error("fragment object publication", error)
            })?;
    // The publication receipt retains the complete immutable object, including
    // entry metadata. Bind before capture; later equality must still reject
    // any replacement, omission or mutation of that checked binding.
    if let Some(entry) = request.hosted_receiver {
        let contract = entry
            .storage_entry()
            .and_then(|storage| storage.physical_contract())
            .ok_or_else(|| {
                super::diagnostics::realization_error(
                    "ProgramEntry receiver provisioning",
                    "missing exact hosted physical contract",
                )
            })?;
        let demand =
            image_emission::derive_stack_demand(&object, object.entry()).map_err(|error| {
                super::diagnostics::realization_error("ProgramEntry receiver stack demand", error)
            })?;
        image_emission::bind_macos_hosted_receiver(
            &mut object,
            entry.source(),
            contract,
            entry.fused_service_establishments(),
            &demand,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
    }
    let scope = match request.boundary_application_coverage {
        Some(coverage) => {
            native_artifact::NativePhysicalEvidenceScope::from_validated_fragment_publication(
                request.optimized_plan,
                request.terminal,
                request.validation,
                request.final_unit,
                coverage,
                &source,
                &object,
            )
            .map_err(|error| {
                super::diagnostics::realization_error(
                    "fragment physical-evidence projection",
                    error,
                )
            })?
        }
        None => native_artifact::NativePhysicalEvidenceScope::Unavailable,
    };
    Ok((object, scope))
}

/// Empty and selected phases publish through the same frame/text/object owners.
pub(super) fn stage_fragment_object(
    physical: crate::StagedOptimizedVerifiedPhysicalPipeline,
) -> Result<object_file::StagedOptimizedRelocationFreeObjectContainer, Vec<Diagnostic>> {
    let emitted = machine_emission::stage_optimized_function_fragment_emission(
        physical.into_function_fragment_emission_source(),
    )
    .map_err(|error| super::diagnostics::realization_error("function-fragment emission", error))?;
    let applied =
        machine_emission::stage_function_fragment_frame_application(emitted).map_err(|error| {
            super::diagnostics::realization_error("function-fragment frame application", error)
        })?;
    let text = machine_emission::stage_optimized_fixed_frame_text_section(applied)
        .map_err(|error| super::diagnostics::realization_error("framed text placement", error))?;
    object_file::stage_optimized_relocation_free_object_container(text)
        .map_err(|error| super::diagnostics::realization_error("fragment object placement", error))
}
