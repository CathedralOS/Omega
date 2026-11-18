//! Compilation metadata, Build outputs, and the completed phase-10 product.

use crate::{
    CompileReportSettlement, NativeProductRequest, OptimizationRollbackReceipt,
    RetainedNativeArtifact, RetainedTerminalArtifact, SelectedIndependentComponent,
};
use artifacts::compile_timings::PhaseTiming;
use optimization_requests::OptimizationRollbackSettlement;
pub use product_publication::{
    CompileOutputKind, RetainedNativeProgram, RetainedNativeProgramIdentity,
};
use product_publication::{
    CompletedProduct, ExecutablePublicationReceipt, NativePackagePublicationReceipt,
    RetainedDynamicElfArtifact,
};
use provider_admission::provider_admission::TerminalAuthorityPermissionPolicy;
use std::path::{Path, PathBuf};
use target::NativeApplicationMetadata;

/// The Product phase's completed scalar work and its unchanged provider inputs.
/// Native completion adds application roots and demanded values to this owner.
/// It is never encoded in saved native inputs.
pub struct ProductScalarCompletion {
    pub completed: source_compiler::evaluation::ScalarApplicationCompletion,
    pub proposals: Vec<backend_plan::TerminalTargetProviderProposal>,
}

pub struct PendingNativeProduct {
    pub terminal: RetainedTerminalArtifact,
    pub scalar_completion: ProductScalarCompletion,
    pub retained_native_inputs: Option<Vec<u8>>, // optional bytes from checked producer, not a saved native admission
    pub components: Result<Vec<SelectedIndependentComponent>, String>,
    pub accepted_package_permissions: TerminalAuthorityPermissionPolicy,
    pub request: NativeProductRequest,
    pub rollback: OptimizationRollbackSettlement,
    pub subsystem: u16,
    pub settlement: CompileReportSettlement,
    pub tests: Vec<crate::PendingTestProduct>, // Registered test roots, in roster order; run before this application succeeds.
    pub test_notes: Vec<crate::TestRosterEntry>, // Disabled or empty groups, reported beside the executed roster.
}

#[derive(Debug)]
pub struct CompileReport {
    source_completion_accounting: source_compiler::evaluation::SourceCompletionAccounting,
    product: CompletedProduct,
    build_outputs: Option<crate::RetainedBuildOutputs>,
    build_observation: Option<package_compilation::BuildObservationIdentity>,
    root_path: PathBuf,
    pub source_file_count: usize,
    build_outputs_published: bool,
    timings: Vec<PhaseTiming>,
    test_roster: Vec<crate::TestRosterEntry>, // Registered tests run before this report succeeded.
    advisories: Vec<diagnostics::Diagnostic>, // Warnings and their notes about the accepted source; none is an error.
}

impl CompileReport {
    #[must_use]
    pub fn with_test_roster(mut self, roster: Vec<crate::TestRosterEntry>) -> Self {
        self.test_roster = roster;
        self
    }

    pub fn test_roster(&self) -> &[crate::TestRosterEntry] {
        &self.test_roster
    }

    #[must_use]
    pub fn with_advisories(mut self, advisories: Vec<diagnostics::Diagnostic>) -> Self {
        self.advisories = advisories;
        self
    }

    pub fn advisories(&self) -> &[diagnostics::Diagnostic] {
        &self.advisories
    }

    pub(crate) fn from_completed_product(
        root_path: PathBuf,
        source_file_count: usize,
        product: CompletedProduct,
    ) -> Self {
        Self {
            source_completion_accounting: Default::default(),
            product,
            build_outputs: None,
            build_observation: None,
            root_path,
            source_file_count,
            build_outputs_published: false,
            test_roster: Vec::new(),
            advisories: Vec::new(),
            timings: Vec::new(),
        }
    }

    pub(crate) fn with_source_completion_accounting(
        mut self,
        accounting: source_compiler::evaluation::SourceCompletionAccounting,
    ) -> Self {
        self.source_completion_accounting = accounting;
        self
    }

    pub fn source_completion_accounting(
        &self,
    ) -> &source_compiler::evaluation::SourceCompletionAccounting {
        &self.source_completion_accounting
    }

    pub(crate) fn from_build_outputs(
        root_path: PathBuf,
        source_file_count: usize,
        outputs: crate::RetainedBuildOutputs,
    ) -> Result<Self, &'static str> {
        let mut report = Self::from_completed_product(
            root_path,
            source_file_count,
            CompletedProduct::build_artifacts(),
        );
        report.build_observation = Some(outputs.observation_identity());
        report.build_outputs = Some(outputs);
        Ok(report)
    }

    pub(crate) fn with_build_outputs(
        mut self,
        outputs: Option<crate::RetainedBuildOutputs>,
        expected_observation: Option<package_compilation::BuildObservationIdentity>,
    ) -> Result<Self, &'static str> {
        if self.output_kind() == CompileOutputKind::CheckOnly || self.build_outputs.is_some() {
            return Err("completed outputs require a product with no previous output set");
        }
        self.build_outputs = outputs;
        self.build_observation = expected_observation;
        Ok(self)
    }

    pub fn build_outputs(&self) -> Option<&crate::RetainedBuildOutputs> {
        self.build_outputs.as_ref()
    }

    pub fn root_path(&self) -> &std::path::Path {
        &self.root_path
    }

    pub fn with_timings(mut self, timings: Vec<PhaseTiming>) -> Self {
        self.timings = timings;
        self
    }

    pub fn timings(&self) -> &[PhaseTiming] {
        &self.timings
    }

    pub fn summary(&self) -> String {
        format!(
            "compiled {} source file(s) from {}; wrote_output={}",
            self.source_file_count,
            self.root_path.display(),
            self.wrote_output()
        )
    }

    pub fn check_only(root_path: PathBuf, source_file_count: usize) -> Result<Self, &'static str> {
        Self::checked(
            root_path,
            source_file_count,
            false,
            CompileOutputKind::CheckOnly,
            None,
        )
    }

    pub fn checked(
        root_path: PathBuf,
        source_file_count: usize,
        wrote_output: bool,
        output_kind: CompileOutputKind,
        executable_publication: Option<ExecutablePublicationReceipt>,
    ) -> Result<Self, &'static str> {
        let product = CompletedProduct::checked(wrote_output, output_kind, executable_publication)?;
        Ok(Self::from_completed_product(
            root_path,
            source_file_count,
            product,
        ))
    }

    pub const fn wrote_output(&self) -> bool {
        self.product.wrote_output() || self.build_outputs_published
    }

    pub fn with_runtime_provisions(
        mut self,
        provisions: Option<crate::TerminalRuntimeProvisions>,
    ) -> Result<Self, &'static str> {
        self.product = self.product.with_runtime_provisions(provisions)?;
        Ok(self)
    }

    pub(crate) fn publish(mut self, build_dir: &Path) -> Result<Self, String> {
        if self.wrote_output() {
            return Err("publication requires an unpublished product".into());
        }
        self.product = self.product.publish(&self.root_path, build_dir)?;
        if let Some(outputs) = &mut self.build_outputs {
            outputs.publish(build_dir)?;
            if self.product.output_kind() != CompileOutputKind::BuildArtifacts
                && !self.product.wrote_output()
            {
                return Err(
                    "completed-output publication requires a checked, published primary product"
                        .into(),
                );
            }
            if outputs.published_directory().is_none() {
                return Err("completed-output publication requires installed files".into());
            }
            self.build_outputs_published = true;
        }
        Ok(self)
    }

    pub const fn output_kind(&self) -> CompileOutputKind {
        self.product.output_kind()
    }

    pub const fn retained_native_artifact(&self) -> Option<&RetainedNativeArtifact> {
        self.product.retained_native_artifact()
    }

    pub fn retained_native_program(&self) -> Option<RetainedNativeProgram<'_>> {
        self.product.retained_native_program()
    }

    pub const fn retained_dynamic_elf_artifact(&self) -> Option<&RetainedDynamicElfArtifact> {
        self.product.retained_dynamic_elf_artifact()
    }

    pub const fn optimization_rollback_receipt(&self) -> Option<&OptimizationRollbackReceipt> {
        self.product.optimization_rollback_receipt()
    }

    pub fn validate_component_product_inputs(&self) -> Result<(), String> {
        self.product.validate_component_product_inputs()
    }

    pub fn validate_component_process_inputs(
        &self,
        executable: &std::path::Path,
    ) -> Result<(), String> {
        self.product.validate_component_process_inputs(executable)
    }

    pub fn runtime_provisions(&self) -> Option<&crate::RuntimeExecutionProvisions> {
        self.product
            .runtime_provisions()
            .map(|provisions| provisions.execution())
    }

    pub const fn published_runtime_provisions(&self) -> Option<&crate::TerminalRuntimeProvisions> {
        self.product.runtime_provisions()
    }

    pub fn application_metadata(&self) -> Option<&NativeApplicationMetadata> {
        self.product.application_metadata()
    }

    pub fn application_name(&self) -> Option<&str> {
        self.product.application_name()
    }

    pub fn application_intent(&self) -> Option<target::HostedApplicationIntent> {
        self.product.application_intent()
    }

    pub fn application_identifier(&self) -> Option<&target::ApplicationIdentifier> {
        self.product.application_identifier()
    }

    pub fn artifact(&self) -> Option<&terminal_codec::CanonicalTerminalArtifact> {
        self.product.artifact()
    }

    pub fn into_retained_terminal_artifact(self) -> Option<RetainedTerminalArtifact> {
        self.product.into_retained_terminal_artifact()
    }

    pub fn checked_native_executable_path(&self) -> Option<&std::path::Path> {
        self.product.checked_native_executable_path()
    }

    pub fn checked_native_package_path(&self) -> Option<&std::path::Path> {
        self.product.checked_native_package_path()
    }

    pub fn package_publication(&self) -> Option<&NativePackagePublicationReceipt> {
        self.product.package_publication()
    }
}
