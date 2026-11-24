//! Compile an authorized Psi program for one receiving target.
#![forbid(unsafe_code)]

mod driver;

#[cfg(test)]
#[path = "tests/physical_publication/mod.rs"]
mod physical_publication_tests;

#[path = "verification/stage_validators.rs"]
pub mod stage_validators;

pub use driver::inputs::components::ComponentSourcePublication;
pub use driver::inputs::published_application::{
    SavedProgramTarget, SourceNativeTarget, prepare_saved_program, prepare_source_program,
    review_source_providers,
};
pub use host_provider_inputs::{InstalledPackageProviders, TargetProviderInstallation};
pub use native_artifact::AdmittedRuntimeProgram;
pub use provider_realization::{
    SelectedProviderResult, published_application::NativeApplicationSettings,
};

use diagnostics::Diagnostic;
use driver::request::NativeCompileRequest;
use provider_admission::current_terminal_authority_policy;
use psi_entry_memory::PrivateEntryRamInitialization;
use verified_optimization_input::RetainedTerminalArtifact;

/// Observe compiler stages and exercise foreign ABI adapter emission in integration tests.
#[cfg(feature = "test-support")]
pub mod test_support {
    pub use crate::driver::observations::{
        NativePipelineInvocation, NativePipelineUnit, native_pipeline_invocations,
    };
    pub use crate::driver::runtime::foreign_adapters::compile_runtime_foreign_adapter;
}

/// The emitted image and the checks requested against its stage inputs.
pub struct NativeEmission {
    pub image: native_artifact::FinishedNativeImage,
    pub validations: Vec<stage_validators::StageValidationRow>,
}

/// A native invocation whose target provider demand was matched to its authorized
/// program. Its reviewed assumptions travel to the same entry and installation checks.
pub struct NativeProgram {
    terminal: RetainedTerminalArtifact,
    suppliers: driver::inputs::components::VerifiedSuppliers,
    permissions: provider_admission::TerminalAuthorityPermissionPolicy,
    isolated_execution_grant: Option<semantic_vocabulary::TerminalAuthorityDisposition>,
    installation_assumptions: Vec<installation_conditions::InstallationEnvironmentAssumption>,
}

impl NativeProgram {
    pub fn check_target(&self) -> Result<(), Vec<Diagnostic>> {
        driver::inputs::check_target_inputs(&self.terminal, &mut Vec::new()).map(|_| ())
    }

    /// The settled permissions and binding selections used by both execution paths.
    pub fn native_realization_proposal(
        &self,
    ) -> &verified_optimization_input::TerminalNativeRealizationProposal {
        self.terminal.native_realization_proposal()
    }

    /// Portable supplier descriptions proposed to the receiving source Product.
    pub fn component_descriptions(
        &self,
    ) -> impl Iterator<
        Item = (
            semantic_vocabulary::PackageKeyIdentity,
            &component_description::VerifiedComponent,
        ),
    > {
        self.suppliers
            .iter()
            .map(|(&package, supplier)| (package, supplier.component.as_ref()))
    }

    pub fn emit(
        self,
        entry_memory: &[PrivateEntryRamInitialization],
        validators: &stage_validators::StageValidatorRequest,
    ) -> Result<NativeEmission, stage_validators::NativeValidationError> {
        let profile = self.native_realization_proposal().target_profile();
        let (image, originals) = self.finish(
            entry_memory,
            &mut Vec::new(),
            !validators.is_empty(),
            |terminal, request| driver::compile(&terminal, request),
        )?;
        let validations = validators.check(profile, &image, &originals)?;
        Ok(NativeEmission { image, validations })
    }

    pub fn admit_runtime(
        self,
        entry_memory: &[PrivateEntryRamInitialization],
        accounting: &mut impl Extend<native_scalar_inputs::EvaluationReceipt>,
    ) -> Result<AdmittedRuntimeProgram, Vec<Diagnostic>> {
        self.finish(entry_memory, accounting, false, driver::admit_runtime)
    }

    fn finish<T>(
        self,
        private_entry_ram: &[PrivateEntryRamInitialization],
        accounting: &mut impl Extend<native_scalar_inputs::EvaluationReceipt>,
        retain_native_stage_inputs: bool,
        finish: impl FnOnce(
            RetainedTerminalArtifact,
            NativeCompileRequest<'_>,
        ) -> Result<T, Vec<Diagnostic>>,
    ) -> Result<T, Vec<Diagnostic>> {
        let Self {
            terminal,
            suppliers,
            permissions,
            isolated_execution_grant,
            installation_assumptions,
        } = self;
        let mut receipts = Vec::new();
        let request = NativeCompileRequest {
            suppliers: &suppliers,
            retain_native_stage_inputs,
            installation_assumptions: &installation_assumptions,
            private_entry_ram,
            isolated_execution_grant: isolated_execution_grant.as_ref(),
            terminal_authority_policy: current_terminal_authority_policy(),
            accepted_package_terminal_authority_permission_policy: permissions,
            terminal_authority_permission_policy: None,
            image_request: driver::request::image_request(terminal.native_realization_proposal())?,
            scalar_receipts: &mut receipts,
        };
        let product = finish(terminal, request);
        accounting.extend(receipts);
        product
    }
}
