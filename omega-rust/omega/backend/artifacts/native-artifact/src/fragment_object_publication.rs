//! Publish machine fragments as an object with entry custody and physical evidence.

use diagnostics::Diagnostic;

use crate::NativePhysicalEvidenceScope;
use crate::optimized_semantic_wrapper_object::{
    StagedValidatedOptimizedProgramStorageSemanticWrapperObject, bind_semantic_contract,
    receiver_layout, stage_validated_optimized_program_storage_semantic_wrapper_object,
};
use terminal_psi_to_abstract_operations::ValidatedNativeProgramEntrySettlement;

/// Entry custody and companion functions for one emitted object.
pub struct FragmentObjectPublicationRequest<'request> {
    /// The canonical Terminal artifact the emitted object seals. The semantic
    /// wrapper route re-stages the fragment container under this exact
    /// artifact so its custody replay binds the same canonical bytes the
    /// realized product carries.
    pub terminal: &'request terminal_codec::CanonicalTerminalArtifact,
    /// The validated admission custody the binder must satisfy exactly. This
    /// is the replayed settlement, not the caller-supplied declaration: its
    /// source signature, storage contract, and Fused establishment roster were
    /// already joined against the canonical artifact before reaching emission.
    pub hosted_receiver: Option<&'request ValidatedNativeProgramEntrySettlement>,
    pub boundary_application_coverage:
        Option<&'request boundary_applications::TerminalBoundaryApplicationCoverage>,
    /// The materialized compiler-private callback functions, in placement
    /// order. Each record carries real emitted bytes produced by the shared
    /// physical pipeline inside this same realization.
    pub private_functions: &'request [machine_code::CompilerPrivateMachineCodeFunction],
}

/// One emitted object and the custody retained by its publication checks.
pub struct PublishedFragmentObject {
    pub object: image_emission::ObjectArtifact,
    pub physical_evidence_scope: NativePhysicalEvidenceScope,
    /// The validated semantic-entry object custody when the admitted entry is
    /// a source-authored semantic shell (the UEFI receiver route). Retained
    /// for the settlement's caller; absent on every ordinary fragment object.
    pub semantic_wrapper_object:
        Option<StagedValidatedOptimizedProgramStorageSemanticWrapperObject>,
}

/// Consume physical emission custody, emit and place the fragments, and bind
/// the admitted entry before capturing publication evidence. Failures in
/// emission, entry construction, or independent replay publish no object.
pub fn publish_fragment_object(
    emission: machine_emission::StagedOptimizedFunctionFragmentEmissionSource,
    request: FragmentObjectPublicationRequest<'_>,
) -> Result<PublishedFragmentObject, Vec<Diagnostic>> {
    // Physical construction already retains the admitted abstract program.
    // Share that allocation before consuming emission; do not clone the plan
    // or accept independently supplied publication identities. The publication
    // validator still replays the emitted object and checks this exact join.
    let optimized = emission.optimized_target().optimized();
    let plan = optimized.shared_program();
    let validation = optimized.validation();
    let source = std::sync::Arc::new(stage_fragment_object(emission)?);
    let mut object =
        image_emission::build_function_fragment_object_artifact_with_private_functions(
            std::sync::Arc::clone(&source),
            request.private_functions,
        )
        .map_err(|error| publication_error("fragment object publication", error))?;
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
            return Err(publication_error(
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
            let terminal = request.terminal;
            let source_artifact = object_file::stage_validated_optimized_object_artifact_shared(
                terminal_codec::CanonicalTerminalArtifact::from_bytes(&terminal.to_bytes())
                    .map_err(|error| {
                        publication_error(
                            "semantic entry object staging",
                            format!("canonical artifact reproduction failed: {error:?}"),
                        )
                    })?,
                std::sync::Arc::clone(&source),
            )
            .map_err(|error| {
                publication_error(
                    "semantic entry object staging",
                    format!("emitted object artifact replay failed: {error:?}"),
                )
            })?;
            let contract = bind_semantic_contract(entry).map_err(|error| {
                publication_error(
                    "semantic entry contract",
                    format!("settlement did not bind its retained semantic contract: {error:?}"),
                )
            })?;
            let receiver =
                receiver_layout(&source_artifact, entry, &contract).map_err(|error| {
                    publication_error(
                        "semantic entry receiver",
                        format!("selected receiver shape is not wrapper-provisionable: {error:?}"),
                    )
                })?;
            let plan = program_entry_plan::plan_optimized_program_storage_semantic_wrapper(
                contract, receiver,
            )
            .map_err(|error| {
                publication_error(
                    "semantic entry wrapper plan",
                    format!("semantic wrapper recipe rejected: {error:?}"),
                )
            })?;
            let encoding =
                program_entry_plan::select_optimized_program_storage_semantic_wrapper_encoding(
                    plan,
                )
                .map_err(|error| {
                    publication_error(
                        "semantic entry wrapper encoding",
                        format!("target encoding replay rejected: {error:?}"),
                    )
                })?;
            semantic_wrapper_object = Some(
                stage_validated_optimized_program_storage_semantic_wrapper_object(
                    entry.clone(),
                    source_artifact,
                    encoding,
                )
                .map_err(|error| {
                    publication_error(
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
                    publication_error(
                        "ProgramEntry receiver provisioning",
                        "missing exact hosted physical contract",
                    )
                })?;
            let demand = image_emission::derive_stack_demand(&object, object.entry())
                .map_err(|error| publication_error("ProgramEntry receiver stack demand", error))?;
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
            image_emission::bind_hosted_receiver(
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
    let physical_evidence_scope = match request.boundary_application_coverage {
        Some(coverage) => NativePhysicalEvidenceScope::from_validated_fragment_publication(
            &plan,
            validation.psi(),
            validation.identity(),
            validation.final_unit(),
            coverage,
            &source,
            &object,
        )
        .map_err(|error| publication_error("fragment physical-evidence projection", error))?,
        None => NativePhysicalEvidenceScope::Unavailable,
    };
    validate_emitted_receiver_binding(
        &object,
        semantic_wrapper_object.is_some(),
        request.hosted_receiver,
    )?;
    Ok(PublishedFragmentObject {
        object,
        physical_evidence_scope,
        semantic_wrapper_object,
    })
}

/// Empty and selected phases publish through the same frame/text/object owners.
fn stage_fragment_object(
    emission: machine_emission::StagedOptimizedFunctionFragmentEmissionSource,
) -> Result<object_file::StagedOptimizedRelocationFreeObjectContainer, Vec<Diagnostic>> {
    let emitted = machine_emission::stage_optimized_function_fragment_emission(emission)
        .map_err(|error| publication_error("function-fragment emission", error))?;
    let applied = machine_emission::stage_function_fragment_frame_application(emitted)
        .map_err(|error| publication_error("function-fragment frame application", error))?;
    let text = machine_emission::stage_optimized_fixed_frame_text_section(applied)
        .map_err(|error| publication_error("framed text placement", error))?;
    object_file::stage_optimized_relocation_free_object_container(text)
        .map_err(|error| publication_error("fragment object placement", error))
}

/// Admission is demand, not construction. The emitted object must carry a
/// hosted receiver binding iff one was admitted, and that binding must be the
/// validated settlement's exact source signature and physical contract — a
/// missing binding is bypassed provisioning, and a substituted one binds a
/// different continuation, receiver, or arrival contract than the checked
/// source selected.
fn validate_emitted_receiver_binding(
    object: &image_emission::ObjectArtifact,
    semantic_wrapper_object: bool,
    admitted: Option<&ValidatedNativeProgramEntrySettlement>,
) -> Result<(), Vec<Diagnostic>> {
    let binding = object.hosted_receiver_binding();
    let Some(settlement) = admitted else {
        return if binding.is_none() && !semantic_wrapper_object {
            Ok(())
        } else {
            Err(publication_error(
                "ProgramEntry receiver provisioning",
                "emitted object carries a hosted receiver binding that admission never granted",
            ))
        };
    };
    if settlement.source().target_slot().owner == target::TargetProfile::UefiX64 {
        // The authored semantic entry binds through the staged wrapper object's
        // custody, not a hosted shim: the emitted object must stay
        // receiver-binding-free and the wrapper object must have been staged.
        return if binding.is_none() && semantic_wrapper_object {
            Ok(())
        } else {
            Err(publication_error(
                "ProgramEntry receiver provisioning",
                "admitted UEFI semantic entry did not reach a staged wrapper object",
            ))
        };
    }
    let Some(binding) = binding else {
        return Err(publication_error(
            "ProgramEntry receiver provisioning",
            "admitted receiver provisioning did not reach the emitted object",
        ));
    };
    let admitted_contract = settlement
        .storage_entry()
        .and_then(|storage| storage.physical_contract());
    if binding.source() != settlement.source()
        || admitted_contract != Some(binding.physical_contract())
    {
        return Err(publication_error(
            "ProgramEntry receiver provisioning",
            "emitted hosted receiver binding does not carry the admitted source signature and physical contract",
        ));
    }
    let admitted_occupancy = settlement
        .checked_entry()
        .receiver_eligibility()
        .is_some_and(|eligibility| {
            eligibility.cleanup()
                == terminal_psi::CheckedProgramEntryReceiverCleanup::OccupiesHostedExtent
        });
    if binding.cleanup_occupancy() != admitted_occupancy {
        return Err(publication_error(
            "ProgramEntry receiver provisioning",
            "emitted hosted receiver binding does not track the admitted cleanup occupancy",
        ));
    }
    Ok(())
}

fn publication_error(context: &str, error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "native artifact {context} failed: {error}"
    ))]
}
