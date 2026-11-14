/*
Native realization: validate Terminal custody, admit providers and program entry,
emit objects and images, and capture Terminal-runtime provisions.

`admit_realization` runs Omega 00 provider/entry admission and captures the
admitted `TerminalRuntimeProvisions` before emission consumes the installation,
so both `realize_native_artifact` and `admit_terminal_runtime` (`omega run
--engine psi`) settle the exact same providers, entry, and host
realizations.
*/

mod callable_objects;
mod callback_thunks;
mod component_calls;
mod component_exports;
pub(crate) use component_calls::ComponentNativeCall;
pub(crate) use component_exports::{
    ComponentExportImage, ComponentExportRequest, emit_component_exports,
};
mod object_emission;

use crate::request::NativeRealizationRequest;
use diagnostics::Diagnostic;
use native_artifact::RequestedNativeArtifact;

use self::object_emission::emit_realization_object;
use terminal_psi_to_abstract_operations::provider_admission::realization_error;
use terminal_psi_to_abstract_operations::{
    AdmittedNativeRealization, admit_prepared_native_realization,
};

pub struct RealizedNativeProduct {
    pub artifact: RequestedNativeArtifact,
    pub runtime_provisions: Option<native_artifact::TerminalRuntimeProvisions>,
}

struct AdmittedRealization<'request> {
    admitted: AdmittedNativeRealization<'request>,
    entry_settlement:
        Option<terminal_psi_to_abstract_operations::ValidatedNativeProgramEntrySettlement>,
    runtime_provisions: Option<native_artifact::TerminalRuntimeProvisions>,
}

fn admit_realization<'request>(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    input: terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput,
    request: &NativeRealizationRequest<'request>,
) -> Result<AdmittedRealization<'request>, Vec<Diagnostic>> {
    let admitted = admit_prepared_native_realization(input, artifact, &request.admission)?;
    let AdmittedNativeRealization {
        input, providers, ..
    } = &admitted;
    let entry_settlement = native_artifact::admit_executable_entry(
        input,
        artifact,
        request.program_entry,
        request.admission.providers.target(),
    )?;
    let entry = request.program_entry;
    let runtime_provisions = entry
        .checked_entry
        .map(|_| {
            native_artifact::TerminalRuntimeProvisions::capture_native_admission(
                entry,
                input,
                providers
                    .installation
                    .as_ref()
                    .map(|installation| installation.terminal_selection()),
                providers
                    .settlements
                    .iter()
                    .map(|settlement| native_artifact::TerminalRuntimeHostProvision {
                        boundary: settlement.boundary,
                        realization: settlement.realization.clone(),
                    })
                    .collect(),
                request.admission.providers.selected_provider_plans,
                request.admission.providers.external_binding_rows,
            )
        })
        .transpose()
        .map_err(|message| vec![Diagnostic::error(message)])?;
    Ok(AdmittedRealization {
        admitted,
        entry_settlement,
        runtime_provisions,
    })
}

pub(crate) fn realize_native_artifact(
    artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    input: terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput,
    request: NativeRealizationRequest<'_>,
) -> Result<RealizedNativeProduct, Vec<Diagnostic>> {
    let AdmittedRealization {
        admitted:
            AdmittedNativeRealization {
                input,
                mut providers,
                boundary_application_coverage,
            },
        entry_settlement,
        runtime_provisions,
    } = admit_realization(&artifact, input, &request)?;
    let private_entry_ram = input.private_entry_ram().to_vec();
    let program_provider_context = providers.program_provider_context().clone();
    let emitted = emit_realization_object(
        input,
        providers.installation.take(),
        &providers.settlements,
        entry_settlement.as_ref(),
        &program_provider_context,
        &request,
    )?;
    let runtime_provisions = runtime_provisions.map(|provisions| {
        provisions
            .with_private_entry_ram(private_entry_ram)
            .with_borrowed_storage_layouts(emitted.borrowed_storage_layouts)
            .with_target_provider_proposals(request.target_provider_proposals.to_vec())
    });
    let emitted = emitted.object;
    let image_request = request
        .image_request
        .clone()
        .settle_elf_loader(&emitted.object, request.elf_interpreter)
        .map_err(|error| vec![error.diagnostic().clone()])?;
    let artifact = {
        request
            .observation
            .enter(crate::observations::NativePipelineStage::ImageEmission);
        RequestedNativeArtifact::emit_admitted(
            request.admission.providers.target(),
            artifact,
            emitted,
            image_request,
            request.admission.providers.selected_provider_plans,
            providers,
            boundary_application_coverage,
        )
    }?;
    Ok(RealizedNativeProduct {
        artifact,
        runtime_provisions,
    })
}

pub(crate) fn admit_terminal_runtime(
    artifact: &std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    input: terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput,
    request: NativeRealizationRequest<'_>,
) -> Result<Option<native_artifact::TerminalRuntimeProvisions>, Vec<Diagnostic>> {
    let AdmittedRealization {
        admitted:
            AdmittedNativeRealization {
                input,
                mut providers,
                ..
            },
        runtime_provisions,
        ..
    } = admit_realization(artifact, input, &request)?;
    let private_entry_ram = input.private_entry_ram().to_vec();
    let borrowed_storage_layouts = object_emission::lower_borrowed_storage_layouts(
        input,
        providers.installation.take(),
        &providers.settlements,
        &request,
    )?;
    Ok(runtime_provisions.map(|provisions| {
        provisions
            .with_private_entry_ram(private_entry_ram)
            .with_borrowed_storage_layouts(borrowed_storage_layouts)
            .with_target_provider_proposals(request.target_provider_proposals.to_vec())
    }))
}

pub(crate) fn selected_physical_pipeline_failed(
    selections: &optimization_core::OptimizationSelections,
    error: impl std::fmt::Display,
) -> Vec<Diagnostic> {
    if selections.is_empty() {
        return realization_error("identity physical pipeline", error);
    }
    let names = selections
        .as_slice()
        .iter()
        .map(|optimization| optimization.build_case_name())
        .collect::<Vec<_>>()
        .join("`, `");
    vec![Diagnostic::error(format!(
        "selected optimization{} `{names}` failed in the complete verified optimizer pipeline during selected/physical validation: {error}; no alternate compiler route was run and no output was installed",
        if selections.as_slice().len() == 1 {
            ""
        } else {
            "s"
        },
    ))]
}
