//! Product evidence joins for the shared fragment-to-object publication path.

use diagnostics::Diagnostic;

pub(super) struct OptimizedFragmentPublicationRequest<'request> {
    /// The validated admission custody the binder must satisfy exactly. This
    /// is the replayed settlement, not the caller-supplied declaration: its
    /// source signature, storage contract, and Fused establishment roster were
    /// already joined against the canonical artifact before reaching emission.
    pub(super) hosted_receiver: Option<&'request crate::ValidatedNativeProgramEntrySettlement>,
    pub(super) boundary_application_coverage:
        Option<&'request boundary_applications::TerminalBoundaryApplicationCoverage>,
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
    let emission = physical.into_function_fragment_emission_source();
    // Physical construction already retains the admitted abstract program.
    // Share that allocation before consuming emission; do not clone the plan
    // or accept independently supplied publication identities. The publication
    // validator still replays the emitted object and checks this exact join.
    let optimized = emission.optimized_target().optimized();
    let plan = optimized.shared_program();
    let validation = optimized.validation();
    let source = std::sync::Arc::new(stage_fragment_object(emission)?);
    let mut object =
        image_emission::build_function_fragment_object_artifact(std::sync::Arc::clone(&source))
            .map_err(|error| {
                super::realization_diagnostics::realization_error(
                    "fragment object publication",
                    error,
                )
            })?;
    // The publication receipt retains the complete immutable object, including
    // entry metadata. Bind before capture; later equality must still reject
    // any replacement, omission or mutation of that checked binding.
    if let Some(entry) = request.hosted_receiver {
        let contract = entry
            .storage_entry()
            .and_then(|storage| storage.physical_contract())
            .ok_or_else(|| {
                super::realization_diagnostics::realization_error(
                    "ProgramEntry receiver provisioning",
                    "missing exact hosted physical contract",
                )
            })?;
        let demand =
            image_emission::derive_stack_demand(&object, object.entry()).map_err(|error| {
                super::realization_diagnostics::realization_error(
                    "ProgramEntry receiver stack demand",
                    error,
                )
            })?;
        image_emission::bind_hosted_receiver(
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
                &plan,
                validation.psi(),
                validation.identity(),
                validation.final_unit(),
                coverage,
                &source,
                &object,
            )
            .map_err(|error| {
                super::realization_diagnostics::realization_error(
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
    emission: machine_emission::StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<object_file::StagedOptimizedRelocationFreeObjectContainer, Vec<Diagnostic>> {
    let emitted = machine_emission::stage_optimized_function_fragment_emission(emission).map_err(
        |error| {
            super::realization_diagnostics::realization_error("function-fragment emission", error)
        },
    )?;
    let applied =
        machine_emission::stage_function_fragment_frame_application(emitted).map_err(|error| {
            super::realization_diagnostics::realization_error(
                "function-fragment frame application",
                error,
            )
        })?;
    let text =
        machine_emission::stage_optimized_fixed_frame_text_section(applied).map_err(|error| {
            super::realization_diagnostics::realization_error("framed text placement", error)
        })?;
    object_file::stage_optimized_relocation_free_object_container(text).map_err(|error| {
        super::realization_diagnostics::realization_error("fragment object placement", error)
    })
}
