//! Native publication: validate Terminal custody, admit providers, emit, and replay.
//!
//! The request supplies realization authority separately from the portable
//! program, including the retained behavior-exclusion union; selected
//! optimizations do not choose a different publication route.

mod callback_thunks;
mod object_emission;
pub(crate) mod realization_diagnostics;

use compilation_products::{NativeRealizationRequest, RequestedNativeArtifactError};
use diagnostics::Diagnostic;
use native_artifact::RequestedNativeArtifact;

use self::object_emission::emit_realization_object;
use terminal_psi_to_abstract_operations::{AdmittedNativeRealization, admit_native_realization};

/// Image and the exact source-free runtime provisions admitted beside it.
/// The backend image does not own host interpretation policy or runtime state.
pub struct RealizedNativeProduct {
    pub artifact: RequestedNativeArtifact,
    pub runtime_provisions: Option<compilation_products::TerminalRuntimeProvisions>,
}

/// Realize one Terminal artifact using explicit image, custody, reuse, and
/// behavior-exclusion inputs. The request's retained behavior-exclusion union
/// is adjudicated against the mechanism-closure review by stage 00,
/// so a requested absence holds whether or not the request carries a receiver
/// permission policy, and a violated exclusion publishes no product. Failure
/// returns the exact image request; no product is silently substituted.
pub fn realize_native_artifact(
    artifact: terminal_codec::CanonicalTerminalArtifact,
    request: NativeRealizationRequest<'_>,
) -> Result<RealizedNativeProduct, RequestedNativeArtifactError> {
    realize_image(artifact, &request).map_err(|diagnostics| request.into_failure(diagnostics))
}

fn realize_image(
    artifact: terminal_codec::CanonicalTerminalArtifact,
    request: &NativeRealizationRequest<'_>,
) -> Result<RealizedNativeProduct, Vec<Diagnostic>> {
    let AdmittedNativeRealization {
        input,
        mut providers,
        boundary_application_coverage,
    } = admit_native_realization(&artifact, &request.admission)?;
    let receiver_settlement = native_artifact::admit_executable_entry_receiver(
        &input,
        &artifact,
        request.admission.providers.program_entry,
        request.admission.providers.target,
    )?;
    // Capture before emission takes the installation. These are the admitted
    // rows of this same image request, not reconstructed provider-name hints.
    let entry = request.admission.providers.program_entry;
    let runtime_provisions = entry
        .checked_entry
        .map(|checked| {
            compilation_products::TerminalRuntimeProvisions::capture_native_admission(
                checked,
                entry.fused_service_establishments(),
                providers
                    .installation
                    .as_ref()
                    .map(|installation| installation.psi_installation()),
                providers
                    .settlements
                    .iter()
                    .map(
                        |settlement| compilation_products::TerminalRuntimeHostProvision {
                            boundary: settlement.boundary,
                            realization: settlement.realization.clone(),
                        },
                    )
                    .collect(),
                &input.plan().boundary_machines,
                request.admission.providers.selected_provider_plans,
                entry.source().target_slot(),
            )
        })
        .transpose()
        .map_err(|message| vec![Diagnostic::error(message)])?;
    let emitted = emit_realization_object(
        input,
        providers.installation.take(),
        &providers.settlements,
        &providers.native_callbacks,
        boundary_application_coverage.as_ref(),
        receiver_settlement.as_ref(),
        &artifact,
        request,
    )?;
    // Build output configuration supplies the loader; the emitted object's own
    // normalized ELF imports decide whether the output links dynamically.
    let image_request = request
        .image_request
        .clone()
        .settle_elf_loader(&emitted.object, request.elf_interpreter)
        .map_err(|error| vec![error.diagnostic().clone()])?;
    let artifact = RequestedNativeArtifact::emit_admitted(
        request.admission.providers.target,
        artifact,
        emitted,
        image_request,
        request.admission.providers.selected_provider_plans,
        providers,
        boundary_application_coverage,
    )?;
    Ok(RealizedNativeProduct {
        artifact,
        runtime_provisions,
    })
}
