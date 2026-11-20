//! Capture the admitted host and callable provisions for the Terminal interpreter.
//! Foreign ABI adapters use the target calling contract recorded with each admission.
pub(crate) mod foreign_adapters;

use crate::driver::AdmittedRealization;
use crate::driver::observations::NativePipelineObservation;
use crate::driver::request::native_optimization_budget;
use artifact_lowering::AdmittedNativeRealization;
use diagnostics::Diagnostic;
use provider_admission::realization_error;
use verified_optimization_input::runtime_provisions::AdmittedRuntimeForeignCall;
pub use verified_optimization_input::runtime_provisions::{
    NativeBindingRuntimeInputs, NativeRuntimePlan, TerminalCallableRuntimeProvisions,
    TerminalRuntimeHostProvision, TerminalRuntimeHostProvisions,
};
use verified_optimization_input::{AdmittedRuntimeInput, NativeProgramEntrySettlement};

/// An admitted auxiliary native root. Capturing runtime metadata records this
/// request; the native compiler plans and encodes it before publishing provisions.
#[derive(Debug, Clone)]
pub struct NativeForeignAdapterRequest {
    bridge: calling_conventions::NativeForeignBridgeRequest,
    locator: target::NormalizedForeignLocator,
    application: [u8; 32],
}

impl NativeForeignAdapterRequest {
    #[cfg(feature = "test-support")]
    pub(crate) fn for_test(
        bridge: calling_conventions::NativeForeignBridgeRequest,
        locator: target::NormalizedForeignLocator,
        application: [u8; 32],
    ) -> Self {
        Self {
            bridge,
            locator,
            application,
        }
    }

    pub fn into_parts(
        self,
    ) -> (
        calling_conventions::NativeForeignBridgeRequest,
        target::NormalizedForeignLocator,
        [u8; 32],
    ) {
        (self.bridge, self.locator, self.application)
    }
}

fn plan_adapter(call: AdmittedRuntimeForeignCall) -> Result<NativeForeignAdapterRequest, String> {
    let (formal_count, formals, application, locator) = call.into_parts();
    let scalar_parameters = application.foreign_scalar_parameters(formal_count, &formals)?;
    Ok(NativeForeignAdapterRequest {
        bridge: calling_conventions::NativeForeignBridgeRequest {
            target: application.target,
            foreign: application.plan,
            scalar_parameters,
        },
        locator,
        application: application.commitment,
    })
}

pub fn plan_host_provisions(
    installation: Option<&psi_provider_selections::AdmittedProviderSelections>,
    host: Vec<TerminalRuntimeHostProvision>,
    input: AdmittedRuntimeInput<'_>,
    selected: &selected_provider_plans::SelectedProviderPlanFacts,
    external_bindings: &[calling_conventions::ExternalBindingRow],
    native: NativeBindingRuntimeInputs<'_>,
) -> Result<TerminalRuntimeHostProvisions<NativeForeignAdapterRequest>, String> {
    let admitted = TerminalRuntimeHostProvisions::admit(
        installation,
        host,
        input,
        selected,
        external_bindings,
        native,
    )?;
    Ok(admitted.map_adapters(plan_adapter))
}

pub fn plan_root_provisions(
    program_entry: NativeProgramEntrySettlement<'_>,
    input: AdmittedRuntimeInput<'_>,
    installation: Option<&psi_provider_selections::AdmittedProviderSelections>,
    host: Vec<TerminalRuntimeHostProvision>,
    execution: &provider_realization::native_execution::PreparedNativeExecution,
    native: NativeBindingRuntimeInputs<'_>,
) -> Result<NativeRuntimePlan<NativeForeignAdapterRequest>, String> {
    let admitted = NativeRuntimePlan::admit(
        program_entry,
        input,
        installation,
        host,
        execution.checked_providers(),
        execution.external_bindings(),
        native,
    )?;
    Ok(admitted.map_adapters(plan_adapter))
}

pub(crate) fn prepare_terminal_runtime(
    realization: AdmittedRealization<'_>,
    target: target::NativeTarget,
    selections: &optimization_core::PostTerminalOptimizationSelections,
    observation: &NativePipelineObservation,
) -> Result<NativeRuntimePlan<object_file::ForeignAdapterObject>, Vec<Diagnostic>> {
    let AdmittedRealization {
        admitted: AdmittedNativeRealization { input, .. },
        runtime_provisions,
        ..
    } = realization;
    let borrowed_storage_layouts = crate::driver::compile_body::plan_runtime_storage(
        input,
        target,
        selections,
        native_optimization_budget(),
        observation,
    )
    .map_err(|error| match error {
        crate::driver::compile_body::NativeBodyPlanningError::AbstractOptimization(error) => {
            realization_error("canonical abstract optimization", error)
        }
        crate::driver::compile_body::NativeBodyPlanningError::Target(error) => {
            realization_error("runtime borrowed storage layout", error)
        }
    })?;
    let provisions = runtime_provisions.ok_or_else(|| {
        vec![Diagnostic::error(
            "runtime admission requires a checked program entry",
        )]
    })?;
    Ok(provisions
        .map_adapters(crate::driver::runtime::foreign_adapters::compile)
        .with_borrowed_storage_layouts(borrowed_storage_layouts))
}
