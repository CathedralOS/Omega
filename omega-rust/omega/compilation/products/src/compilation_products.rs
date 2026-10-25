//! The compile report: which product a compilation produced,
//! the payload each product kind owns, and checked publication receipts.
//! Filesystem orchestration belongs to `publication`, not this model.
//!
//! `CompileReport` is a common shell — root, source count, build-output
//! companions and timings — around
//! one `CompileProduct`. Payload-owning variants carry the evidence only that
//! product can hold, so custody checks are joins over present fields rather
//! than negative-space assertions that unrelated `Option`s stayed `None`.

use target::NativeApplicationMetadata;

use crate::{OptimizationRollbackReceipt, RetainedNativeArtifact, RetainedTerminalArtifact};
use artifacts::compile_timings::PhaseTiming;
use native_artifact::{
    DynamicElfNativeArtifact, DynamicElfPublicationReceipt, ExecutablePublicationReceipt,
    NativePackagePublicationReceipt,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileOutputKind {
    CheckOnly,
    TerminalArtifact,
    RetainedNativeArtifact,
    NativeExecutable,
    ObjectContainer,
    BuildArtifacts,
}

impl CompileOutputKind {}

/// The retained Terminal product and the subtractive build overlay whose
/// effective selection the artifact and its native proposal must carry.
#[derive(Debug)]
struct TerminalProduct {
    /// Canonical source-free Psi artifact retained at the exact Psi/Omega
    /// ownership seam. It carries no target or deployment authority.
    artifact: RetainedTerminalArtifact,
    /// Exact subtractive release overlay applied after build selection.
    optimization_rollback: Option<OptimizationRollbackReceipt>,
}

/// A validated native payload retained before any output or runtime
/// installation, with the authored application metadata its publication may
/// consume.
#[derive(Debug)]
struct NativeProduct {
    artifact: RetainedNativeArtifact,
    runtime_provisions: Option<crate::TerminalRuntimeProvisions>,
    /// Exact subtractive release overlay applied after build selection and
    /// before native realization. Ordinary requests retain `None`.
    optimization_rollback: Option<OptimizationRollbackReceipt>,
    application: NativeApplicationMetadata,
}

/// The installed-executable receipts remaining after publication consumes
/// the retained native artifact. The retained application metadata carries
/// over so the package/executable cross-binding can be replayed.
#[derive(Debug)]
struct PublishedNativeProduct {
    runtime_provisions: Option<crate::TerminalRuntimeProvisions>,
    /// Exact checked publication receipt for a native executable image.
    executable: ExecutablePublicationReceipt,
    /// Exact checked receipt for a macOS application package publication;
    /// flat publications retain `None`.
    package: Option<NativePackagePublicationReceipt>,
    /// Exact subtractive release overlay applied after build selection and
    /// before native realization. Ordinary requests retain `None`.
    optimization_rollback: Option<OptimizationRollbackReceipt>,
    application: NativeApplicationMetadata,
}

/// A validated dynamically linked ELF payload retained before output: an
/// import-bearing Linux object emitted under the build's effective loader.
/// It is the authority-distinct dynamic product, never direct-image,
/// component or installation custody
/// (wiki/spec/build/component_publication.md#products-and-authority).
#[derive(Debug)]
struct DynamicElfProduct {
    artifact: DynamicElfNativeArtifact,
    runtime_provisions: Option<crate::TerminalRuntimeProvisions>,
    /// Exact subtractive release overlay applied after build selection and
    /// before native realization. Ordinary requests retain `None`.
    optimization_rollback: Option<OptimizationRollbackReceipt>,
    application: NativeApplicationMetadata,
}

/// The flat-file receipt remaining after publication consumes a retained
/// dynamic ELF payload. It grants no loader or execution authority.
#[derive(Debug)]
struct PublishedDynamicElfProduct {
    runtime_provisions: Option<crate::TerminalRuntimeProvisions>,
    executable: DynamicElfPublicationReceipt,
    optimization_rollback: Option<OptimizationRollbackReceipt>,
    application: NativeApplicationMetadata,
}

/// The product a compilation produced. Payload kinds carry their evidence
/// directly; `NativeExecutable` is reachable only through
/// `record_native_publication` consuming a `RetainedNativeArtifact`.
///
/// A dynamically linked ELF output shares the retained/published native
/// kinds with direct images, because the product role is the same, while its
/// payload and receipt stay separate variants so no direct-image custody
/// check can accept dynamic bytes.
#[derive(Debug)]
enum CompileProduct {
    /// A completed check; no product files exist.
    CheckOnly,
    /// The build's completed files are the product, retained in the
    /// report's `build_outputs` companion slot.
    BuildArtifacts,
    /// The retained canonical Terminal artifact, published or not.
    TerminalArtifact(TerminalProduct),
    /// A validated native payload retained pre-publication.
    RetainedNativeArtifact(NativeProduct),
    /// A native executable installed against an exact checked receipt.
    NativeExecutable(PublishedNativeProduct),
    /// A validated dynamically linked ELF payload retained pre-publication.
    RetainedDynamicElfArtifact(DynamicElfProduct),
    /// A dynamically linked ELF file installed against its exact receipt.
    DynamicElfExecutable(PublishedDynamicElfProduct),
    /// A non-executable object container written to output. Kept for the
    /// `checked` constructor's kind contract; no current route builds one.
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

impl TerminalProduct {}

impl NativeProduct {}

impl DynamicElfProduct {}

impl PublishedDynamicElfProduct {}

impl PublishedNativeProduct {}

#[derive(Debug)]
pub struct CompileReport {
    source_completion_accounting: build_time_evaluation::SourceCompletionAccounting,
    /// What the compilation produced and the evidence that kind owns.
    product: CompileProduct,
    /// Completed files only, bound to the same build observation as the
    /// primary product. Scratch and generated-source files are not products.
    build_outputs: Option<crate::RetainedBuildOutputs>,
    /// Expected build identity retained independently of the file attachment,
    /// including standalone compilations without a package manifest.
    build_observation: Option<package_compilation::BuildObservationIdentity>,
    root_path: PathBuf,
    pub source_file_count: usize,
    wrote_output: bool,
    /// Canonical package-source/build/target/artifact authority for a
    /// package-aware production. Standalone probes carry no such manifest.
    /// Per-stage measurements the checked record accumulated during this
    /// compilation; empty unless the request asked for timing collection.
    timings: Vec<PhaseTiming>,
}

impl CompileReport {
    /// Shared shell fields every product kind starts from.
    fn shell(root_path: PathBuf, source_file_count: usize, product: CompileProduct) -> Self {
        Self {
            source_completion_accounting: Default::default(),
            product,
            build_outputs: None,
            build_observation: None,
            root_path,
            source_file_count,
            wrote_output: false,
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

    /// Actual authored-completion logical charges on cold or reused source.
    /// These are distinct from fresh Build activation and publication custody.
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

    /// Attach companions only after the requested primary product checked.
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

    /// Record companions after their package owner has published them.
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

    /// Advance custody only after native installation supplies its exact receipts.
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

    /// Advance dynamic ELF custody only after its flat file installs against
    /// the exact receipt.
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

    /// Terminal publication retains the portable payload.
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

    /// A completed check with no retained artifact or executable publication.
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
            // Payload kinds cannot be constructed without their retained
            // product; the custody replay refused them as `None` fields too.
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

    /// Retain one dynamically linked ELF payload. Like
    /// [`Self::from_retained_native_artifact`], publication later installs it
    /// as a flat file.
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

    /// Retain whichever image class native realization produced. The direct
    /// and dynamic ELF payloads keep their separate report variants.
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

    /// The retained dynamically linked ELF payload, before publication.
    pub const fn retained_dynamic_elf_artifact(&self) -> Option<&DynamicElfNativeArtifact> {
        match &self.product {
            CompileProduct::RetainedDynamicElfArtifact(product) => Some(&product.artifact),
            _ => None,
        }
    }

    pub const fn optimization_rollback_receipt(&self) -> Option<&OptimizationRollbackReceipt> {
        self.product.optimization_rollback()
    }

    /// Attach the provisions captured by this native production before image
    /// emission consumed the entry and provider records. The runtime consumer
    /// joins their identity to its admitted program before creating an instance.
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

    /// Required typed component imports cannot publish or execute without
    /// their complete image/resource roster, including alternate product paths.
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
            _ => None,
        }
    }

    pub fn with_timings(mut self, timings: Vec<PhaseTiming>) -> Self {
        self.timings = timings;
        self
    }

    /// The stage ladder this compilation recorded; empty when the request
    /// did not collect timings.
    pub fn timings(&self) -> &[PhaseTiming] {
        &self.timings
    }

    /// Retain the authored application name, hosted intent, and identifier
    /// with a retained native artifact. Publication uses the name for the
    /// `.app` basename and inner executable leaf, the intent to select
    /// whole-package versus flat output, and the identifier for
    /// `CFBundleIdentifier` — checked against the CodeDirectory identity
    /// already bound into the signed bytes. Only a retained-native report
    /// can carry them.
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

    /// Complete publication metadata of the retained or published native product.
    pub fn application_metadata(&self) -> Option<&NativeApplicationMetadata> {
        self.product.application()
    }

    /// The retained authored `Build::application` name, when the build
    /// declared an application role.
    pub fn application_name(&self) -> Option<&str> {
        self.product
            .application()
            .and_then(|application| application.name.as_deref())
    }

    /// The retained authored hosted intent.
    pub fn application_intent(&self) -> Option<target::HostedApplicationIntent> {
        self.product
            .application()
            .and_then(|application| application.intent)
    }

    /// The retained authored application identifier.
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
            }),
        ))
    }

    pub fn artifact(&self) -> Option<&terminal_codec::CanonicalTerminalArtifact> {
        match &self.product {
            CompileProduct::TerminalArtifact(product) => Some(product.artifact.artifact()),
            _ => None,
        }
    }

    /// Attach the subtractive build overlay only when the published Psi and
    /// pending native proposal carry its exact effective selection.
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

    /// Transfer the complete Terminal product without dropping its callback
    /// sidecar. There is deliberately no consuming artifact-only projection.
    pub fn into_retained_terminal_artifact(self) -> Option<RetainedTerminalArtifact> {
        match self.product {
            CompileProduct::TerminalArtifact(product) => Some(product.artifact),
            _ => None,
        }
    }

    /// Returns the exact installed flat executable only after independently
    /// replaying the complete report custody checks. For a published macOS
    /// package this is the inner `Contents/MacOS/<name>` path — the checked
    /// package root is [`Self::checked_native_package_path`]. Object/check-only
    /// reports and any internally drifted receipt graph fail closed.
    pub fn checked_native_executable_path(&self) -> Option<&std::path::Path> {
        match &self.product {
            CompileProduct::NativeExecutable(product) => Some(product.executable.output_path()),
            CompileProduct::DynamicElfExecutable(product) => Some(product.executable.output_path()),
            _ => None,
        }
    }

    /// Returns the installed `.app` package root only after independently
    /// replaying the complete report custody checks, including the
    /// package/executable cross-binding. Flat publications, non-executable
    /// reports, and any drifted receipt graph expose no path at all.
    pub fn checked_native_package_path(&self) -> Option<&std::path::Path> {
        if !matches!(self.product, CompileProduct::NativeExecutable(_)) {
            return None;
        }
        self.package_publication()
            .map(NativePackagePublicationReceipt::package_root)
    }

    /// The checked package receipt for a published `.app`, when this report
    /// installed one.
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
