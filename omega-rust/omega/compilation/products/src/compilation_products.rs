/*
The compile report, pending native work carrier, and checked publication receipts.

`CompileReport` wraps a common shell (root path, source file count, build-output
companions, and timings) around one `CompileProduct`. Payload-owning variants
carry the evidence only that product kind can hold, so custody checks are joins
over present fields rather than negative-space assertions that unrelated
`Option`s stayed `None`. Dynamically linked ELF outputs share the
retained/published native output kind with direct images while keeping separate
variants so no direct-image custody check can accept dynamic bytes.
*/

/*
`PendingNativeProduct` carries the source-free work remaining after source
admission and Terminal preparation for application-sponsored native completion.
`RetainedNativeProgram` and `RetainedNativeProgramIdentity` provide read-only
replay views over retained native products before publication consumes them.
*/

use crate::{
    CompileReportSettlement, NativeProductRequest, OptimizationRollbackReceipt,
    RetainedNativeArtifact, RetainedTerminalArtifact, SelectedIndependentComponent,
};
use artifacts::compile_timings::PhaseTiming;
use native_artifact::{
    DynamicElfNativeArtifact, DynamicElfNativeArtifactIdentity, DynamicElfPublicationReceipt,
    ExecutablePublicationReceipt, NativeArtifactIdentity, NativePackagePublicationReceipt,
};
use optimization_core::OptimizationRollbackSettlement;
use std::path::PathBuf;
use target::NativeApplicationMetadata;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi_to_abstract_operations::provider_admission::TerminalAuthorityPermissionPolicy;

pub struct PendingNativeProduct {
    pub terminal: RetainedTerminalArtifact,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedNativeProgramIdentity {
    Direct(NativeArtifactIdentity),
    DynamicElf(DynamicElfNativeArtifactIdentity),
}

impl std::fmt::Display for RetainedNativeProgramIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct(identity) => std::fmt::Display::fmt(identity, formatter),
            Self::DynamicElf(identity) => std::fmt::Display::fmt(identity, formatter),
        }
    }
}

pub struct RetainedNativeProgram<'artifact> {
    pub(crate) artifact: &'artifact CanonicalTerminalArtifact,
    pub(crate) identity: RetainedNativeProgramIdentity,
}

impl<'artifact> RetainedNativeProgram<'artifact> {
    pub fn psi_artifact(&self) -> &'artifact CanonicalTerminalArtifact {
        self.artifact
    }

    pub fn identity(&self) -> RetainedNativeProgramIdentity {
        self.identity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileOutputKind {
    CheckOnly,
    TerminalArtifact,
    RetainedNativeArtifact,
    NativeExecutable,
    ObjectContainer,
    BuildArtifacts,
}

#[derive(Debug)]
struct TerminalProduct {
    artifact: RetainedTerminalArtifact,
    optimization_rollback: Option<OptimizationRollbackReceipt>,
    runtime_provisions: Option<crate::TerminalRuntimeProvisions>, // present only when a native request stopped after runtime admission
}

#[derive(Debug)]
struct NativeProduct {
    artifact: RetainedNativeArtifact,
    runtime_provisions: Option<crate::TerminalRuntimeProvisions>,
    optimization_rollback: Option<OptimizationRollbackReceipt>,
    application: NativeApplicationMetadata,
}

#[derive(Debug)]
struct PublishedNativeProduct {
    runtime_provisions: Option<crate::TerminalRuntimeProvisions>,
    executable: ExecutablePublicationReceipt,
    package: Option<NativePackagePublicationReceipt>, // macOS .app package receipt; flat publications retain None
    optimization_rollback: Option<OptimizationRollbackReceipt>,
    application: NativeApplicationMetadata,
}

#[derive(Debug)]
struct DynamicElfProduct {
    artifact: DynamicElfNativeArtifact,
    runtime_provisions: Option<crate::TerminalRuntimeProvisions>,
    optimization_rollback: Option<OptimizationRollbackReceipt>,
    application: NativeApplicationMetadata,
}

#[derive(Debug)]
struct PublishedDynamicElfProduct {
    runtime_provisions: Option<crate::TerminalRuntimeProvisions>,
    executable: DynamicElfPublicationReceipt,
    optimization_rollback: Option<OptimizationRollbackReceipt>,
    application: NativeApplicationMetadata,
}

#[derive(Debug)]
enum CompileProduct {
    CheckOnly,
    BuildArtifacts,
    TerminalArtifact(TerminalProduct),
    RetainedNativeArtifact(NativeProduct),
    NativeExecutable(PublishedNativeProduct),
    RetainedDynamicElfArtifact(DynamicElfProduct),
    DynamicElfExecutable(PublishedDynamicElfProduct),
    ObjectContainer,
}

impl CompileProduct {
    const fn kind(&self) -> CompileOutputKind {
        match self {
            Self::CheckOnly => CompileOutputKind::CheckOnly,
            Self::BuildArtifacts => CompileOutputKind::BuildArtifacts,
            Self::TerminalArtifact(_) => CompileOutputKind::TerminalArtifact,
            Self::RetainedNativeArtifact(_) | Self::RetainedDynamicElfArtifact(_) => {
                CompileOutputKind::RetainedNativeArtifact
            }
            Self::NativeExecutable(_) | Self::DynamicElfExecutable(_) => {
                CompileOutputKind::NativeExecutable
            }
            Self::ObjectContainer => CompileOutputKind::ObjectContainer,
        }
    }

    fn application(&self) -> Option<&NativeApplicationMetadata> {
        match self {
            Self::RetainedNativeArtifact(product) => Some(&product.application),
            Self::NativeExecutable(product) => Some(&product.application),
            Self::RetainedDynamicElfArtifact(product) => Some(&product.application),
            Self::DynamicElfExecutable(product) => Some(&product.application),
            _ => None,
        }
    }

    const fn optimization_rollback(&self) -> Option<&OptimizationRollbackReceipt> {
        match self {
            Self::TerminalArtifact(product) => product.optimization_rollback.as_ref(),
            Self::RetainedNativeArtifact(product) => product.optimization_rollback.as_ref(),
            Self::NativeExecutable(product) => product.optimization_rollback.as_ref(),
            Self::RetainedDynamicElfArtifact(product) => product.optimization_rollback.as_ref(),
            Self::DynamicElfExecutable(product) => product.optimization_rollback.as_ref(),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct CompileReport {
    source_completion_accounting: build_time_evaluation::SourceCompletionAccounting,
    product: CompileProduct,
    build_outputs: Option<crate::RetainedBuildOutputs>,
    build_observation: Option<package_compilation::BuildObservationIdentity>,
    root_path: PathBuf,
    pub source_file_count: usize,
    wrote_output: bool,
    timings: Vec<PhaseTiming>,
    test_roster: Vec<crate::TestRosterEntry>, // Registered tests run before this report succeeded.
    float_realizations: Vec<String>, // Observations from the selected plans required by this product.
}

impl CompileReport {
    pub(crate) fn with_float_realizations(mut self, rows: Vec<String>) -> Self {
        self.float_realizations = rows;
        self
    }

    pub fn float_realizations(&self) -> &[String] {
        &self.float_realizations
    }

    #[must_use]
    pub fn with_test_roster(mut self, roster: Vec<crate::TestRosterEntry>) -> Self {
        self.test_roster = roster;
        self
    }

    pub fn test_roster(&self) -> &[crate::TestRosterEntry] {
        &self.test_roster
    }

    fn shell(root_path: PathBuf, source_file_count: usize, product: CompileProduct) -> Self {
        Self {
            source_completion_accounting: Default::default(),
            product,
            build_outputs: None,
            build_observation: None,
            root_path,
            source_file_count,
            wrote_output: false,
            test_roster: Vec::new(),
            float_realizations: Vec::new(),
            timings: Vec::new(),
        }
    }

    pub(crate) fn with_source_completion_accounting(
        mut self,
        accounting: build_time_evaluation::SourceCompletionAccounting,
    ) -> Self {
        self.source_completion_accounting = accounting;
        self
    }

    pub fn source_completion_accounting(
        &self,
    ) -> &build_time_evaluation::SourceCompletionAccounting {
        &self.source_completion_accounting
    }

    pub(crate) fn from_build_outputs(
        root_path: PathBuf,
        source_file_count: usize,
        outputs: crate::RetainedBuildOutputs,
    ) -> Result<Self, &'static str> {
        let mut report = Self::check_only(root_path, source_file_count)?;
        report.product = CompileProduct::BuildArtifacts;
        report.build_observation = Some(outputs.observation_identity());
        report.build_outputs = Some(outputs);
        Ok(report)
    }

    pub(crate) fn with_build_outputs(
        mut self,
        outputs: Option<crate::RetainedBuildOutputs>,
        expected_observation: Option<package_compilation::BuildObservationIdentity>,
    ) -> Result<Self, &'static str> {
        if matches!(self.product, CompileProduct::CheckOnly) || self.build_outputs.is_some() {
            return Err("completed outputs require a product with no previous output set");
        }
        self.build_outputs = outputs;
        self.build_observation = expected_observation;
        Ok(self)
    }

    pub fn build_outputs(&self) -> Option<&crate::RetainedBuildOutputs> {
        self.build_outputs.as_ref()
    }

    pub(crate) fn build_outputs_mut(&mut self) -> Option<&mut crate::RetainedBuildOutputs> {
        self.build_outputs.as_mut()
    }

    pub const fn build_observation_identity(
        &self,
    ) -> Option<package_compilation::BuildObservationIdentity> {
        self.build_observation
    }

    pub(crate) fn record_build_output_publication(mut self) -> Result<Self, String> {
        if !matches!(self.product, CompileProduct::BuildArtifacts) && !self.wrote_output {
            return Err(
                "completed-output publication requires a checked, published primary product".into(),
            );
        }
        if self
            .build_outputs
            .as_ref()
            .ok_or("completed-output publication requires retained files")?
            .published_directory()
            .is_none()
        {
            return Err("completed-output publication requires installed files".into());
        }
        self.wrote_output = true;
        Ok(self)
    }

    pub(crate) fn record_native_publication(
        mut self,
        receipt: ExecutablePublicationReceipt,
        package: Option<NativePackagePublicationReceipt>,
    ) -> Result<Self, String> {
        let CompileProduct::RetainedNativeArtifact(product) = self.product else {
            return Err("native publication requires exactly one retained native artifact".into());
        };
        if self.wrote_output {
            return Err("native publication requires exactly one retained native artifact".into());
        }
        self.product = CompileProduct::NativeExecutable(PublishedNativeProduct {
            runtime_provisions: product.runtime_provisions,
            executable: receipt,
            package,
            optimization_rollback: product.optimization_rollback,
            application: product.application,
        });
        self.wrote_output = true;
        Ok(self)
    }

    pub(crate) fn record_dynamic_elf_publication(
        mut self,
        receipt: DynamicElfPublicationReceipt,
    ) -> Result<Self, String> {
        let CompileProduct::RetainedDynamicElfArtifact(product) = self.product else {
            return Err(
                "dynamic ELF publication requires exactly one retained dynamic ELF artifact".into(),
            );
        };
        if self.wrote_output {
            return Err(
                "dynamic ELF publication requires exactly one retained dynamic ELF artifact".into(),
            );
        }
        self.product = CompileProduct::DynamicElfExecutable(PublishedDynamicElfProduct {
            runtime_provisions: product.runtime_provisions,
            executable: receipt,
            optimization_rollback: product.optimization_rollback,
            application: product.application,
        });
        self.wrote_output = true;
        Ok(self)
    }

    pub(crate) fn record_terminal_publication(mut self) -> Result<Self, String> {
        if !matches!(self.product, CompileProduct::TerminalArtifact(_)) || self.wrote_output {
            return Err(
                "terminal publication requires exactly one retained terminal artifact".into(),
            );
        }
        self.wrote_output = true;
        Ok(self)
    }

    pub(crate) fn retained_terminal_artifact(&self) -> Option<&RetainedTerminalArtifact> {
        match &self.product {
            CompileProduct::TerminalArtifact(product) => Some(&product.artifact),
            _ => None,
        }
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
        if executable_publication.is_some() && output_kind != CompileOutputKind::NativeExecutable {
            return Err("compiler report retained inconsistent executable publication receipts");
        }
        let product = match output_kind {
            CompileOutputKind::CheckOnly => CompileProduct::CheckOnly,
            CompileOutputKind::ObjectContainer => CompileProduct::ObjectContainer,
            CompileOutputKind::NativeExecutable => match executable_publication {
                Some(executable) => CompileProduct::NativeExecutable(PublishedNativeProduct {
                    runtime_provisions: None,
                    executable,
                    package: None,
                    optimization_rollback: None,
                    application: NativeApplicationMetadata::default(),
                }),
                None => {
                    return Err("a native executable report requires its publication receipt");
                }
            },
            CompileOutputKind::TerminalArtifact
            | CompileOutputKind::RetainedNativeArtifact
            | CompileOutputKind::BuildArtifacts => {
                return Err(
                    "a retained report requires its product payload; use the retained constructors",
                );
            }
        };
        let mut report = Self::shell(root_path, source_file_count, product);
        report.wrote_output = wrote_output;
        Ok(report)
    }

    pub(crate) fn from_retained_native_artifact(
        root_path: PathBuf,
        source_file_count: usize,
        artifact: RetainedNativeArtifact,
        optimization_rollback: Option<OptimizationRollbackReceipt>,
    ) -> Result<Self, &'static str> {
        Ok(Self::shell(
            root_path,
            source_file_count,
            CompileProduct::RetainedNativeArtifact(NativeProduct {
                artifact,
                runtime_provisions: None,
                optimization_rollback,
                application: NativeApplicationMetadata::default(),
            }),
        ))
    }

    pub(crate) fn from_retained_dynamic_elf_artifact(
        root_path: PathBuf,
        source_file_count: usize,
        artifact: DynamicElfNativeArtifact,
        optimization_rollback: Option<OptimizationRollbackReceipt>,
    ) -> Result<Self, &'static str> {
        Ok(Self::shell(
            root_path,
            source_file_count,
            CompileProduct::RetainedDynamicElfArtifact(DynamicElfProduct {
                artifact,
                runtime_provisions: None,
                optimization_rollback,
                application: NativeApplicationMetadata::default(),
            }),
        ))
    }

    pub(crate) fn from_requested_native_artifact(
        root_path: PathBuf,
        source_file_count: usize,
        artifact: native_artifact::RequestedNativeArtifact,
        optimization_rollback: Option<OptimizationRollbackReceipt>,
    ) -> Result<Self, &'static str> {
        match artifact {
            native_artifact::RequestedNativeArtifact::Direct(artifact) => {
                Self::from_retained_native_artifact(
                    root_path,
                    source_file_count,
                    artifact,
                    optimization_rollback,
                )
            }
            native_artifact::RequestedNativeArtifact::DynamicElf(artifact) => {
                Self::from_retained_dynamic_elf_artifact(
                    root_path,
                    source_file_count,
                    artifact,
                    optimization_rollback,
                )
            }
        }
    }

    pub fn root_path(&self) -> &std::path::Path {
        &self.root_path
    }

    pub const fn wrote_output(&self) -> bool {
        self.wrote_output
    }

    pub const fn output_kind(&self) -> CompileOutputKind {
        self.product.kind()
    }

    pub const fn retained_native_artifact(&self) -> Option<&RetainedNativeArtifact> {
        match &self.product {
            CompileProduct::RetainedNativeArtifact(product) => Some(&product.artifact),
            _ => None,
        }
    }

    pub fn retained_native_program(&self) -> Option<RetainedNativeProgram<'_>> {
        match &self.product {
            CompileProduct::RetainedNativeArtifact(product) => Some(RetainedNativeProgram {
                artifact: product.artifact.psi_artifact(),
                identity: RetainedNativeProgramIdentity::Direct(product.artifact.identity()),
            }),
            CompileProduct::RetainedDynamicElfArtifact(product) => Some(RetainedNativeProgram {
                artifact: product.artifact.psi_artifact(),
                identity: RetainedNativeProgramIdentity::DynamicElf(product.artifact.identity()),
            }),
            _ => None,
        }
    }

    pub const fn retained_dynamic_elf_artifact(&self) -> Option<&DynamicElfNativeArtifact> {
        match &self.product {
            CompileProduct::RetainedDynamicElfArtifact(product) => Some(&product.artifact),
            _ => None,
        }
    }

    pub const fn optimization_rollback_receipt(&self) -> Option<&OptimizationRollbackReceipt> {
        self.product.optimization_rollback()
    }

    pub fn with_runtime_provisions(
        mut self,
        provisions: Option<crate::TerminalRuntimeProvisions>,
    ) -> Result<Self, &'static str> {
        let destination = match &mut self.product {
            CompileProduct::RetainedNativeArtifact(product) => &mut product.runtime_provisions,
            CompileProduct::RetainedDynamicElfArtifact(product) => &mut product.runtime_provisions,
            _ => return Err("runtime provisions require a retained native product"),
        };
        if destination.is_some() {
            return Err("runtime provisions are already attached to this product");
        }
        *destination = provisions;
        Ok(self)
    }

    pub(crate) fn record_component_publications(
        &mut self,
        receipts: Vec<native_artifact::NativeComponentPublicationReceipt>,
    ) -> Result<(), String> {
        let runtime = match &mut self.product {
            CompileProduct::RetainedDynamicElfArtifact(product) => {
                product.runtime_provisions.as_mut()
            }
            CompileProduct::RetainedNativeArtifact(product)
                if product.artifact.target().object_format == target::ObjectFormat::MachO
                    || product.artifact.target() == target::NativeTarget::windows_x64() =>
            {
                product.runtime_provisions.as_mut()
            }
            _ => return Err(
                "component files require an admitted ELF, Mach-O, or Windows x86-64 process publication contract"
                    .into(),
            ),
        }
        .ok_or("component publication lost its exact runtime product roster")?;
        runtime
            .record_component_publications(receipts)
            .map_err(str::to_owned)
    }

    pub fn validate_component_product_inputs(&self) -> Result<(), String> {
        let object = self
            .retained_native_artifact()
            .map(|artifact| artifact.object())
            .or_else(|| {
                self.retained_dynamic_elf_artifact()
                    .map(|artifact| artifact.object())
            });
        match object {
            Some(object) => crate::TerminalRuntimeProvisions::validate_root_object(
                object,
                self.runtime_provisions(),
            )
            .map_err(str::to_owned),
            None => self.runtime_provisions().map_or(Ok(()), |runtime| {
                runtime
                    .validate_component_product_roster()
                    .map_err(str::to_owned)
            }),
        }
    }

    pub fn validate_component_process_inputs(
        &self,
        executable: &std::path::Path,
    ) -> Result<(), String> {
        self.validate_component_product_inputs()?;
        if self
            .runtime_provisions()
            .is_none_or(|runtime| runtime.component_products().is_empty())
        {
            return Ok(());
        }
        if self.checked_native_executable_path() != Some(executable) {
            return Err(
                "component process root differs from its exact native publication receipt".into(),
            );
        }
        self.runtime_provisions()
            .ok_or("component process lost runtime provisions")?
            .validate_component_process_inputs(executable)
    }

    pub const fn runtime_provisions(&self) -> Option<&crate::TerminalRuntimeProvisions> {
        match &self.product {
            CompileProduct::RetainedNativeArtifact(product) => product.runtime_provisions.as_ref(),
            CompileProduct::RetainedDynamicElfArtifact(product) => {
                product.runtime_provisions.as_ref()
            }
            CompileProduct::NativeExecutable(product) => product.runtime_provisions.as_ref(),
            CompileProduct::DynamicElfExecutable(product) => product.runtime_provisions.as_ref(),
            CompileProduct::TerminalArtifact(product) => product.runtime_provisions.as_ref(),
            _ => None,
        }
    }

    pub fn with_timings(mut self, timings: Vec<PhaseTiming>) -> Self {
        self.timings = timings;
        self
    }

    pub fn timings(&self) -> &[PhaseTiming] {
        &self.timings
    }

    pub(crate) fn with_application_metadata(
        mut self,
        application: NativeApplicationMetadata,
    ) -> Result<Self, &'static str> {
        match &mut self.product {
            CompileProduct::RetainedNativeArtifact(product) => product.application = application,
            CompileProduct::RetainedDynamicElfArtifact(product) => {
                product.application = application;
            }
            _ => {
                return Err("application publication metadata requires a retained native artifact");
            }
        }
        Ok(self)
    }

    pub fn application_metadata(&self) -> Option<&NativeApplicationMetadata> {
        self.product.application()
    }

    pub fn application_name(&self) -> Option<&str> {
        self.product
            .application()
            .and_then(|application| application.name.as_deref())
    }

    pub fn application_intent(&self) -> Option<target::HostedApplicationIntent> {
        self.product
            .application()
            .and_then(|application| application.intent)
    }

    pub fn application_identifier(&self) -> Option<&target::ApplicationIdentifier> {
        self.product
            .application()
            .and_then(|application| application.identifier.as_ref())
    }

    pub(crate) fn from_retained_terminal_artifact(
        root_path: PathBuf,
        source_file_count: usize,
        artifact: RetainedTerminalArtifact,
    ) -> Result<Self, &'static str> {
        if artifact.native_realization_proposal().is_none() {
            return Err(
                "compiler report requires the Terminal product's native realization proposal",
            );
        }
        Ok(Self::shell(
            root_path,
            source_file_count,
            CompileProduct::TerminalArtifact(TerminalProduct {
                artifact,
                optimization_rollback: None,
                runtime_provisions: None,
            }),
        ))
    }

    pub fn artifact(&self) -> Option<&terminal_codec::CanonicalTerminalArtifact> {
        match &self.product {
            CompileProduct::TerminalArtifact(product) => Some(product.artifact.artifact()),
            _ => None,
        }
    }

    pub(crate) fn with_terminal_runtime_provisions(
        mut self,
        provisions: Option<crate::TerminalRuntimeProvisions>,
    ) -> Result<Self, &'static str> {
        let CompileProduct::TerminalArtifact(product) = &mut self.product else {
            return Err("Terminal runtime provisions require a Terminal product");
        };
        product.runtime_provisions = provisions;
        Ok(self)
    }

    pub(crate) fn with_terminal_optimization_rollback(
        mut self,
        rollback: Option<OptimizationRollbackReceipt>,
    ) -> Result<Self, &'static str> {
        let CompileProduct::TerminalArtifact(product) = &mut self.product else {
            return Err("Terminal rollback requires a Terminal product");
        };
        product.optimization_rollback = rollback;
        Ok(self)
    }

    pub fn into_retained_terminal_artifact(self) -> Option<RetainedTerminalArtifact> {
        match self.product {
            CompileProduct::TerminalArtifact(product) => Some(product.artifact),
            _ => None,
        }
    }

    pub fn checked_native_executable_path(&self) -> Option<&std::path::Path> {
        match &self.product {
            CompileProduct::NativeExecutable(product) => Some(product.executable.output_path()),
            CompileProduct::DynamicElfExecutable(product) => Some(product.executable.output_path()),
            _ => None,
        }
    }

    pub fn checked_native_package_path(&self) -> Option<&std::path::Path> {
        let CompileProduct::NativeExecutable(product) = &self.product else {
            return None;
        };
        let package = product.package.as_ref()?;
        let retained_identity_matches = product.application.name.as_deref()
            == Some(package.application_name())
            && product.application.identifier.as_ref() == Some(package.application_identifier());
        (retained_identity_matches && package.binds_executable(&product.executable))
            .then(|| package.package_root())
    }

    pub fn package_publication(&self) -> Option<&NativePackagePublicationReceipt> {
        match &self.product {
            CompileProduct::NativeExecutable(product) => product.package.as_ref(),
            _ => None,
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "compiled {} source file(s) from {}; wrote_output={}",
            self.source_file_count,
            self.root_path.display(),
            self.wrote_output
        )
    }
}

#[cfg(test)]
mod package_path_tests;
