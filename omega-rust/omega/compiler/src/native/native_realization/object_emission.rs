//! One abstract-to-object sequence, independent of optimization selection.

use crate::native::native_realization::callback_thunks::lower_callback_thunks;
use crate::native::native_realization::optimization_stage::lower_realization_optimization_stage;
use crate::native::native_realization::physical_stage::lower_realization_physical_stage;
use crate::native::native_realization::realization_request::{
    NativeRealizationInput, NativeRealizationRequest,
};
use crate::native::native_realization::target_stage::lower_realization_target_stage;
use abstract_operations_to_target_operations::AdmittedBoundarySettlement;
use boundary_applications::TerminalBoundaryApplicationCoverage;
use diagnostics::Diagnostic;
use native_artifact::{
    FragmentObjectPublicationRequest, PublishedFragmentObject, publish_fragment_object,
};
use terminal_psi_to_abstract_operations::AdmittedProviderInstallation;

pub(crate) fn emit_realization_object(
    input: NativeRealizationInput,
    provider_installation: Option<AdmittedProviderInstallation>,
    settlements: &[AdmittedBoundarySettlement<'_>],
    boundary_application_coverage: Option<&TerminalBoundaryApplicationCoverage>,
    hosted_receiver: Option<&crate::native::ValidatedNativeProgramEntrySettlement>,
    terminal: &terminal_codec::CanonicalTerminalArtifact,
    request: &NativeRealizationRequest<'_>,
) -> Result<PublishedFragmentObject, Vec<Diagnostic>> {
    let abstract_stage = lower_realization_optimization_stage(input, request)?;
    // Thunks are independent canonical artifacts: materialize them once the
    // request-level callback/selection custody check has passed, before the
    // shared instruction pipeline runs the program plan.
    let private_functions = lower_callback_thunks(request)?;
    let target_stage = lower_realization_target_stage(
        abstract_stage,
        provider_installation,
        settlements,
        request,
    )?;
    let physical = lower_realization_physical_stage(target_stage, request)?;
    publish_fragment_object(
        physical.into_function_fragment_emission_source(),
        FragmentObjectPublicationRequest {
            terminal,
            boundary_application_coverage,
            hosted_receiver,
            private_functions: &private_functions,
        },
    )
}
