//! Product evidence joins for the shared fragment-to-object publication path.

use diagnostics::Diagnostic;

use crate::compiler::native::optimized_semantic_wrapper_object::{
    bind_semantic_contract, receiver_layout,
};

pub(super) struct OptimizedFragmentPublicationRequest<'request> {
    /// The canonical Terminal artifact the emitted object seals. The semantic
    /// wrapper route re-stages the fragment container under this exact
    /// artifact so its custody replay binds the same canonical bytes the
    /// realized product carries; `None` only where no wrapper route can run.
    pub(super) terminal: Option<&'request terminal_codec::CanonicalTerminalArtifact>,
    /// The validated admission custody the binder must satisfy exactly. This
    /// is the replayed settlement, not the caller-supplied declaration: its
    /// source signature, storage contract, and Fused establishment roster were
    /// already joined against the canonical artifact before reaching emission.
    pub(super) hosted_receiver:
        Option<&'request crate::compiler::native::ValidatedNativeProgramEntrySettlement>,
    pub(super) boundary_application_coverage:
        Option<&'request resolved_layout_to_resolved_layout::boundary_applications::TerminalBoundaryApplicationCoverage>,
    /// The materialized compiler-private callback functions, in placement
    /// order. Each record carries real emitted bytes produced by the shared
    /// physical pipeline inside this same realization.
    pub(super) private_functions: &'request [post_allocation_machine_to_selected_form_encoding::machine_code::CompilerPrivateMachineCodeFunction],
}

pub(super) fn emit_optimized_fragments(
    physical: crate::compiler::native::StagedOptimizedVerifiedPhysicalPipeline,
    request: OptimizedFragmentPublicationRequest<'_>,
) -> Result<
    (
        resolved_layout_to_resolved_layout::image_emission::ObjectArtifact,
        resolved_layout_to_resolved_layout::native_artifact::NativePhysicalEvidenceScope,
        Option<
            crate::compiler::native::StagedValidatedOptimizedProgramStorageSemanticWrapperObject,
        >,
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
    let mut object = if request.private_functions.is_empty() {
        resolved_layout_to_resolved_layout::image_emission::build_function_fragment_object_artifact(std::sync::Arc::clone(&source))
    } else {
        resolved_layout_to_resolved_layout::image_emission::build_function_fragment_object_artifact_with_private_functions(
            source.clone(),
            request.private_functions,
        )
    }
    .map_err(|error| {
        super::realization_diagnostics::realization_error("fragment object publication", error)
    })?;
    // The publication receipt retains the complete immutable object, including
    // entry metadata. Bind before capture; later equality must still reject
    // any replacement, omission or mutation of that checked binding.
    let mut semantic_wrapper_object = None;
    if let Some(entry) = request.hosted_receiver {
        // The settlement is the boundary that would lend each bound
        // placed-view referent; neither the hosted receiver bridge nor the
        // semantic wrapper carries that custody yet, so a nonempty bound set
        // fails closed here rather than publishing an entry whose loans
        // nobody emits.
        if !entry.placed_view_establishments().is_empty() {
            return Err(super::realization_diagnostics::realization_error(
                "ProgramEntry placed-view custody",
                "bound placed-view establishments require the hosted entry boundary to lend each referent; this bridge does not carry them yet",
            ));
        }
        if entry.source().target_slot().owner == target::TargetProfile::UefiX64 {
            // The UEFI entry is the authored semantic-entry route: the
            // compiler-owned wrapper provisions `self` inside its own frame
            // and calls the semantic child by private symbol. Stage the whole
            // evidence chain over the same relocation-free container this
            // object publishes — authored contract, semantic wrapper plan,
            // x86-64 template, joined object, manifest and custody receipt —
            // and retain it for the settlement's caller.
            let terminal = request.terminal.ok_or_else(|| {
                super::realization_diagnostics::realization_error(
                    "semantic entry object staging",
                    "semantic wrapper custody requires the canonical artifact",
                )
            })?;
            let source_artifact = resolved_layout_to_resolved_layout::object_file::stage_validated_optimized_object_artifact_shared(
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&terminal.to_bytes())
                    .map_err(|error| {
                        super::realization_diagnostics::realization_error(
                            "semantic entry object staging",
                            format!("canonical artifact reproduction failed: {error:?}"),
                        )
                    })?,
                std::sync::Arc::clone(&source),
            )
            .map_err(|error| {
                super::realization_diagnostics::realization_error(
                    "semantic entry object staging",
                    format!("emitted object artifact replay failed: {error:?}"),
                )
            })?;
            let contract = bind_semantic_contract(entry).map_err(|error| {
                super::realization_diagnostics::realization_error(
                    "semantic entry contract",
                    format!("settlement did not bind its retained semantic contract: {error:?}"),
                )
            })?;
            let receiver =
                receiver_layout(&source_artifact, entry, &contract).map_err(|error| {
                    super::realization_diagnostics::realization_error(
                        "semantic entry receiver",
                        format!("selected receiver shape is not wrapper-provisionable: {error:?}"),
                    )
                })?;
            let plan = resolved_layout_to_resolved_layout::program_entry_plan::plan_optimized_program_storage_semantic_wrapper(
                contract, receiver,
            )
            .map_err(|error| {
                super::realization_diagnostics::realization_error(
                    "semantic entry wrapper plan",
                    format!("semantic wrapper recipe rejected: {error:?}"),
                )
            })?;
            let encoding =
                resolved_layout_to_resolved_layout::program_entry_plan::select_optimized_program_storage_semantic_wrapper_encoding(
                    plan,
                )
                .map_err(|error| {
                    super::realization_diagnostics::realization_error(
                        "semantic entry wrapper encoding",
                        format!("target encoding replay rejected: {error:?}"),
                    )
                })?;
            semantic_wrapper_object = Some(
                crate::compiler::native::stage_validated_optimized_program_storage_semantic_wrapper_object(
                    entry.clone(),
                    source_artifact,
                    encoding,
                )
                .map_err(|error| {
                    super::realization_diagnostics::realization_error(
                        "semantic entry object staging",
                        format!("wrapper object join rejected: {error:?}"),
                    )
                })?,
            );
        } else {
            let contract = entry
                .storage_entry()
                .and_then(|storage| storage.physical_contract())
                .ok_or_else(|| {
                    super::realization_diagnostics::realization_error(
                        "ProgramEntry receiver provisioning",
                        "missing exact hosted physical contract",
                    )
                })?;
            let demand = resolved_layout_to_resolved_layout::image_emission::derive_stack_demand(
                &object,
                object.entry(),
            )
            .map_err(|error| {
                super::realization_diagnostics::realization_error(
                    "ProgramEntry receiver stack demand",
                    error,
                )
            })?;
            // The checked eligibility decides whether the receiver's nominal
            // cleanup must occupy its hosted extent through completion. The
            // binding records that occupancy so the extent is tracked through
            // the installation ledger rather than silently dropping it.
            let cleanup_occupancy =
                entry
                    .checked_entry()
                    .receiver_eligibility()
                    .is_some_and(|eligibility| {
                        eligibility.cleanup()
                        == terminal_psi::CheckedProgramEntryReceiverCleanup::OccupiesHostedExtent
                    });
            resolved_layout_to_resolved_layout::image_emission::bind_hosted_receiver(
                &mut object,
                entry.source(),
                contract,
                entry.fused_service_establishments(),
                &demand,
                cleanup_occupancy,
            )
            .map_err(|diagnostic| vec![diagnostic])?;
        }
    }
    let scope = match request.boundary_application_coverage {
        Some(coverage) => {
            resolved_layout_to_resolved_layout::native_artifact::NativePhysicalEvidenceScope::from_validated_fragment_publication(
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
        None => resolved_layout_to_resolved_layout::native_artifact::NativePhysicalEvidenceScope::Unavailable,
    };
    Ok((object, scope, semantic_wrapper_object))
}

/// Empty and selected phases publish through the same frame/text/object owners.
pub(super) fn stage_fragment_object(
    emission: resolved_layout_to_resolved_layout::machine_emission::StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<
    resolved_layout_to_resolved_layout::object_file::StagedOptimizedRelocationFreeObjectContainer,
    Vec<Diagnostic>,
> {
    let emitted = resolved_layout_to_resolved_layout::machine_emission::stage_optimized_function_fragment_emission(emission).map_err(
        |error| {
            super::realization_diagnostics::realization_error("function-fragment emission", error)
        },
    )?;
    let applied =
        resolved_layout_to_resolved_layout::machine_emission::stage_function_fragment_frame_application(emitted).map_err(|error| {
            super::realization_diagnostics::realization_error(
                "function-fragment frame application",
                error,
            )
        })?;
    let text =
        resolved_layout_to_resolved_layout::machine_emission::stage_optimized_fixed_frame_text_section(applied).map_err(|error| {
            super::realization_diagnostics::realization_error("framed text placement", error)
        })?;
    resolved_layout_to_resolved_layout::object_file::stage_optimized_relocation_free_object_container(text).map_err(|error| {
        super::realization_diagnostics::realization_error("fragment object placement", error)
    })
}
