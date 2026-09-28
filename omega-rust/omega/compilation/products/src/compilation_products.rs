//! The compile report: which product a compilation produced,
//! the payload each product kind owns, and checked publication receipts.
//! Filesystem orchestration belongs to `publication`, not this model.
//!
//! `CompileReport` is a common shell — root, source count, build-output
//! companions, trust settlement, proof-product requests and timings — around
//! one `CompileProduct`. Payload-owning variants carry the evidence only that
//! product can hold, so custody checks are joins over present fields rather
//! than negative-space assertions that unrelated `Option`s stayed `None`.

use target::NativeApplicationMetadata;

use crate::{
    FinalRealizationEvidenceError, OptimizationRollbackReceipt, PccPublicationReceipt,
    ProductionArtifactIdentity, ProductionCompilationManifest, ProductionCompilationSubject,
    RetainedNativeArtifact, RetainedTerminalArtifact, TerminalNativeRealizationProposal,
};
use artifacts::compile_timings::PhaseTiming;
use native_artifact::{ExecutablePublicationReceipt, NativePackagePublicationReceipt};
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

impl CompileOutputKind {
    /// Eligibility is shared by pre-production admission and direct report attachment.
    pub(crate) fn admit_proof_requests(
        self,
        requests: package_compilation::build_execution::build_value::PccRequests,
    ) -> Result<(), &'static str> {
        match self {
            Self::CheckOnly if requests.any() => {
                Err("a check-only stop cannot satisfy an optional proof-product request")
            }
            Self::BuildArtifacts if requests.any() => {
                Err("artifact-only builds cannot publish Psi or native proof products")
            }
            Self::TerminalArtifact if requests.native => {
                Err("a Terminal stop cannot satisfy a native proof-product request")
            }
            Self::ObjectContainer if requests.any() => {
                Err("an object container cannot satisfy an optional proof-product request")
            }
            _ => Ok(()),
        }
    }
}

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

/// The product a compilation produced. Payload kinds carry their evidence
/// directly; `NativeExecutable` is reachable only through
/// `record_native_publication` consuming a `RetainedNativeArtifact`.
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
            Self::RetainedNativeArtifact(_) => CompileOutputKind::RetainedNativeArtifact,
            Self::NativeExecutable(_) => CompileOutputKind::NativeExecutable,
            Self::ObjectContainer => CompileOutputKind::ObjectContainer,
        }
    }

    /// Replays this product's own custody joins. Fields another kind would
    /// have had to prove `None` no longer exist to be checked.
    fn has_consistent_custody(
        &self,
        wrote_output: bool,
        build_outputs: Option<&crate::RetainedBuildOutputs>,
        manifest: Option<&ProductionCompilationManifest>,
    ) -> bool {
        match self {
            Self::CheckOnly => !wrote_output && build_outputs.is_none(),
            Self::BuildArtifacts => build_outputs.is_some_and(|outputs| {
                wrote_output == outputs.published_directory().is_some()
                    && manifest.is_none_or(|manifest| {
                        manifest.artifact()
                            == ProductionArtifactIdentity::BuildOutputs(*outputs.identity())
                    })
            }),
            Self::TerminalArtifact(product) => product.has_consistent_custody(manifest),
            Self::RetainedNativeArtifact(product) => {
                !wrote_output && product.has_consistent_custody(manifest)
            }
            Self::NativeExecutable(product) => {
                wrote_output && product.has_consistent_custody(manifest)
            }
            Self::ObjectContainer => wrote_output,
        }
    }

    fn application(&self) -> Option<&NativeApplicationMetadata> {
        match self {
            Self::RetainedNativeArtifact(product) => Some(&product.application),
            Self::NativeExecutable(product) => Some(&product.application),
            _ => None,
        }
    }
}

impl TerminalProduct {
    /// The Terminal kind's joins: the artifact validates, any rollback
    /// receipt is Psi-phase and matches both the artifact's optimization
    /// selection and the native proposal's post-Terminal selection, and an
    /// attached manifest names this artifact and the proposal's target.
    fn has_consistent_custody(&self, manifest: Option<&ProductionCompilationManifest>) -> bool {
        self.artifact.validate().is_ok()
            && self.optimization_rollback.as_ref().is_none_or(|receipt| {
                receipt.is_consistent()
                    && receipt.requested_disabled().as_slice().iter().all(|rule| {
                        rule.execution_phase() == optimization_core::OptimizationExecutionPhase::Psi
                    })
                    && self.artifact.artifact().optimization().selection()
                        == receipt.effective().project_psi().selections().identity()
                    && self
                        .artifact
                        .native_realization_proposal()
                        .is_some_and(|proposal| {
                            proposal.post_terminal_optimizations()
                                == &receipt.effective().project_post_terminal()
                        })
            })
            && manifest.is_none_or(|manifest| {
                manifest.matches_terminal_artifact(self.artifact.artifact())
                    && self
                        .artifact
                        .native_realization_proposal()
                        .is_some_and(|proposal| {
                            proposal.target_profile() == manifest.subject().target_profile()
                                && proposal.native_target() == manifest.subject().native_target()
                        })
            })
    }
}

impl NativeProduct {
    fn has_consistent_custody(&self, manifest: Option<&ProductionCompilationManifest>) -> bool {
        self.artifact.validate().is_ok()
            && self
                .optimization_rollback
                .as_ref()
                .is_none_or(OptimizationRollbackReceipt::is_consistent)
            && manifest.is_none_or(|manifest| manifest.matches_native_artifact(&self.artifact))
    }
}

impl PublishedNativeProduct {
    /// The published kind's joins: the executable receipt replays, a package
    /// receipt agrees with it on inner path, digests and the retained
    /// application identity, and an attached manifest names the receipt's
    /// native artifact identity.
    fn has_consistent_custody(&self, manifest: Option<&ProductionCompilationManifest>) -> bool {
        self.executable.has_consistent_installation_identity()
            && self.package.as_ref().is_none_or(|package| {
                package.has_consistent_package_identity()
                    && package.inner_executable_path().as_deref()
                        == Some(self.executable.output_path())
                    && package.executable_container_digest() == self.executable.container_digest()
                    && package.executable_byte_count() == self.executable.container_byte_count()
                    && self.application.name.as_deref() == Some(package.application_name())
                    && self.application.identifier.as_ref()
                        == Some(package.application_identifier())
            })
            && manifest.is_none_or(|manifest| {
                matches!(manifest.artifact(), ProductionArtifactIdentity::Native(identity)
                    if self.executable.native_artifact_identity() == identity.as_bytes())
            })
    }
}

#[derive(Debug)]
pub struct CompileReport {
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
    production_manifest: Option<ProductionCompilationManifest>,
    /// Filesystem-free comparison between the request's explicit admissions
    /// and every trust obligation reconstructed by compilation.
    trust_admission_settlement: trust_model::TrustAdmissionSettlement,
    /// The normalized Build's two independent optional proof-product
    /// requests. Both are off by default; neither grants receiving authority.
    pcc_requests: package_compilation::build_execution::build_value::PccRequests,
    /// The admission profile under which the retained artifact's evidence was
    /// produced. Publication names it as the sidecar's checker profile and
    /// uses it for the producer-side pair validation.
    terminal_admission_profile: proof_admission::AdmissionProfile,
    /// Every artifact/`.proof` companion pair this report published, each
    /// carrying separate artifact and sidecar byte sizes.
    pcc_publications: Vec<PccPublicationReceipt>,
    /// Per-stage measurements the checked record accumulated during this
    /// compilation; empty unless the request asked for timing collection.
    timings: Vec<PhaseTiming>,
}

impl CompileReport {
    /// Shared shell fields every product kind starts from.
    fn shell(root_path: PathBuf, source_file_count: usize, product: CompileProduct) -> Self {
        Self {
            product,
            build_outputs: None,
            build_observation: None,
            root_path,
            source_file_count,
            wrote_output: false,
            production_manifest: None,
            trust_admission_settlement: Default::default(),
            pcc_requests: package_compilation::build_execution::build_value::PccRequests::default(),
            terminal_admission_profile: proof_admission::AdmissionProfile::default(),
            pcc_publications: Vec::new(),
            timings: Vec::new(),
        }
    }

    pub fn from_build_outputs(
        root_path: PathBuf,
        source_file_count: usize,
        outputs: crate::RetainedBuildOutputs,
        production_subject: Option<ProductionCompilationSubject>,
    ) -> Result<Self, &'static str> {
        let mut report = Self::check_only(root_path, source_file_count)?;
        report.production_manifest = production_subject
            .map(|subject| ProductionCompilationManifest::for_build_outputs(subject, &outputs))
            .transpose()?;
        report.product = CompileProduct::BuildArtifacts;
        report.build_observation = Some(outputs.observation_identity());
        report.build_outputs = Some(outputs);
        if !report.has_consistent_executable_publication_custody() {
            return Err("compiler report retained inconsistent completed-output custody");
        }
        Ok(report)
    }

    /// Attach companions only after the requested primary product checked.
    pub fn with_build_outputs(
        mut self,
        outputs: Option<crate::RetainedBuildOutputs>,
        expected_observation: Option<package_compilation::BuildObservationIdentity>,
    ) -> Result<Self, &'static str> {
        if matches!(self.product, CompileProduct::CheckOnly) || self.build_outputs.is_some() {
            return Err("completed outputs require a product with no previous output set");
        }
        self.build_outputs = outputs;
        self.build_observation = expected_observation;
        if !self.has_consistent_executable_publication_custody() {
            return Err("completed build outputs disagree with the product's build observation");
        }
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

    /// Transfer companion custody when consuming a retained primary artifact
    /// into another product. Artifact-only reports cannot lose their payload.
    pub fn take_build_outputs(&mut self) -> Option<crate::RetainedBuildOutputs> {
        if matches!(
            self.product,
            CompileProduct::TerminalArtifact(_) | CompileProduct::RetainedNativeArtifact(_)
        ) && !self.wrote_output
        {
            self.build_outputs.take()
        } else {
            None
        }
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
        if !self.has_consistent_executable_publication_custody() {
            return Err("completed-output publication retained inconsistent custody".into());
        }
        Ok(self)
    }

    /// Advance custody only after native installation supplies its exact receipts.
    pub(crate) fn record_native_publication(
        mut self,
        receipt: ExecutablePublicationReceipt,
        proofs: Vec<PccPublicationReceipt>,
        package: Option<NativePackagePublicationReceipt>,
    ) -> Result<Self, String> {
        let CompileProduct::RetainedNativeArtifact(product) = self.product else {
            return Err("native publication requires exactly one retained native artifact".into());
        };
        if self.wrote_output {
            return Err("native publication requires exactly one retained native artifact".into());
        }
        self.product = CompileProduct::NativeExecutable(PublishedNativeProduct {
            executable: receipt,
            package,
            optimization_rollback: product.optimization_rollback,
            application: product.application,
        });
        self.wrote_output = true;
        self.pcc_publications = proofs;
        if !self.has_consistent_executable_publication_custody() {
            return Err("published native report failed custody replay".into());
        }
        Ok(self)
    }

    /// Terminal publication retains the portable payload and records its proof pairs.
    pub(crate) fn record_terminal_publication(
        mut self,
        proofs: Vec<PccPublicationReceipt>,
    ) -> Result<Self, String> {
        if !matches!(self.product, CompileProduct::TerminalArtifact(_)) || self.wrote_output {
            return Err(
                "terminal publication requires exactly one retained terminal artifact".into(),
            );
        }
        self.wrote_output = true;
        self.pcc_publications = proofs;
        if !self.has_consistent_executable_publication_custody() {
            return Err("published terminal report failed custody replay".into());
        }
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
        if report.has_consistent_executable_publication_custody() {
            Ok(report)
        } else {
            Err("compiler report retained inconsistent executable publication receipts")
        }
    }

    /// Compare retained native-product metadata with the source compilation
    /// that requested it. These expectations do not grant receiving authority
    /// or prove source/IR equivalence; artifact and manifest custody still replay
    /// independently. No source compiler state is retained by this owner.
    pub fn verify_native_source_metadata(
        &self,
        source_file_count: usize,
        target_profile: Option<target::TargetProfile>,
        native_target: Option<target::NativeTarget>,
        application: &NativeApplicationMetadata,
    ) -> Result<(), &'static str> {
        if self.output_kind() != CompileOutputKind::RetainedNativeArtifact
            || self.wrote_output()
            || !self.has_consistent_executable_publication_custody()
        {
            return Err("checked native receipt requires one retained native artifact report");
        }
        if self.source_file_count != source_file_count {
            return Err("checked native receipt source count disagrees with its report");
        }
        let profile =
            target_profile.ok_or("checked native receipt requires one selected target profile")?;
        let native_target =
            native_target.ok_or("checked native receipt requires one selected native target")?;
        if profile.native_target() != native_target {
            return Err("checked native receipt target profile disagrees with its native target");
        }
        let artifact = self
            .retained_native_artifact()
            .ok_or("checked native receipt requires one retained native artifact")?;
        if artifact.target() != native_target {
            return Err("checked native receipt retained artifact target disagrees with checking");
        }
        if let Some(manifest) = self.production_manifest()
            && (manifest.subject().target_profile() != profile
                || manifest.subject().native_target() != native_target
                || !manifest.matches_native_artifact(artifact))
        {
            return Err("checked native receipt production manifest disagrees with checking");
        }
        // The retained publication metadata is exactly the checked build's:
        // a report whose name, intent, identifier, or subsystem drifted from
        // checking cannot pair with it.
        if self.application_metadata() != Some(application) {
            return Err("checked native receipt application metadata disagrees with checking");
        }
        Ok(())
    }

    pub fn from_retained_native_artifact(
        root_path: PathBuf,
        source_file_count: usize,
        artifact: RetainedNativeArtifact,
        optimization_rollback: Option<OptimizationRollbackReceipt>,
        production_subject: Option<ProductionCompilationSubject>,
    ) -> Result<Self, &'static str> {
        artifact
            .validate()
            .map_err(|_| "compiler report received an invalid native artifact")?;
        let production_manifest = production_subject
            .map(|subject| ProductionCompilationManifest::for_native(subject, &artifact))
            .transpose()?;
        let mut report = Self::shell(
            root_path,
            source_file_count,
            CompileProduct::RetainedNativeArtifact(NativeProduct {
                artifact,
                optimization_rollback,
                application: NativeApplicationMetadata::default(),
            }),
        );
        report.production_manifest = production_manifest;
        if !report.has_consistent_executable_publication_custody() {
            return Err("compiler report retained inconsistent native-artifact custody");
        }
        Ok(report)
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

    pub const fn optimization_rollback_receipt(&self) -> Option<&OptimizationRollbackReceipt> {
        match &self.product {
            CompileProduct::TerminalArtifact(product) => product.optimization_rollback.as_ref(),
            CompileProduct::RetainedNativeArtifact(product) => {
                product.optimization_rollback.as_ref()
            }
            CompileProduct::NativeExecutable(product) => product.optimization_rollback.as_ref(),
            _ => None,
        }
    }

    pub const fn production_manifest(&self) -> Option<&ProductionCompilationManifest> {
        self.production_manifest.as_ref()
    }

    /// Require the exact native physical evidence for this package-aware,
    /// retained-native production. Standalone and already-published reports do
    /// not carry the package/artifact join needed to make this claim.
    pub fn require_package_native_physical_evidence(
        &self,
    ) -> Result<&native_artifact::NativePhysicalEvidence, FinalRealizationEvidenceError> {
        if !self.has_consistent_executable_publication_custody() {
            return Err(FinalRealizationEvidenceError::InvalidReportCustody);
        }
        let CompileProduct::RetainedNativeArtifact(product) = &self.product else {
            return Err(FinalRealizationEvidenceError::RetainedNativeArtifactRequired);
        };
        let manifest = self
            .production_manifest
            .as_ref()
            .ok_or(FinalRealizationEvidenceError::PackageProductionManifestRequired)?;
        manifest.require_native_physical_evidence(&product.artifact)
    }

    pub fn with_trust_admission_settlement(
        mut self,
        settlement: trust_model::TrustAdmissionSettlement,
    ) -> Self {
        self.trust_admission_settlement = settlement;
        self
    }

    /// Retain the per-stage measurements the checked record accumulated
    /// during this compilation.
    pub fn with_timings(mut self, timings: Vec<PhaseTiming>) -> Self {
        self.timings = timings;
        self
    }

    /// The stage ladder this compilation recorded; empty when the request
    /// did not collect timings.
    pub fn timings(&self) -> &[PhaseTiming] {
        &self.timings
    }

    /// Retain the normalized Build's optional proof-product requests and the
    /// admission profile that produced the retained evidence. Publication
    /// uses these to emit and self-validate every requested pair; they confer
    /// no receiving authority. Incompatible products reject before attachment;
    /// publication freezes this context so the recorded pairs cannot be relabeled.
    pub fn with_pcc_context(
        mut self,
        pcc_requests: package_compilation::build_execution::build_value::PccRequests,
        terminal_admission_profile: proof_admission::AdmissionProfile,
    ) -> Result<Self, &'static str> {
        self.output_kind().admit_proof_requests(pcc_requests)?;
        if self.wrote_output
            && (self.pcc_requests != pcc_requests
                || self.terminal_admission_profile != terminal_admission_profile)
        {
            return Err("published proof-product context cannot be replaced");
        }
        self.pcc_requests = pcc_requests;
        self.terminal_admission_profile = terminal_admission_profile;
        Ok(self)
    }

    /// Retain the authored application name, hosted intent, and identifier
    /// with a retained native artifact. Publication uses the name for the
    /// `.app` basename and inner executable leaf, the intent to select
    /// whole-package versus flat output, and the identifier for
    /// `CFBundleIdentifier` — checked against the CodeDirectory identity
    /// already bound into the signed bytes. Only a retained-native report
    /// can carry them.
    pub fn with_application_metadata(
        mut self,
        application: NativeApplicationMetadata,
    ) -> Result<Self, &'static str> {
        let CompileProduct::RetainedNativeArtifact(product) = &mut self.product else {
            return Err("application publication metadata requires a retained native artifact");
        };
        product.application = application;
        self.has_consistent_executable_publication_custody()
            .then_some(self)
            .ok_or("application publication metadata left inconsistent report custody")
    }

    /// Complete publication metadata of the retained or published native product.
    pub fn application_metadata(&self) -> Option<&NativeApplicationMetadata> {
        self.product.application()
    }

    /// The retained authored `builder.application` name, when the build
    /// declared an application role.
    pub fn application_name(&self) -> Option<&str> {
        self.product
            .application()
            .and_then(|application| application.name.as_deref())
    }

    /// The retained authored hosted intent.
    pub const fn application_intent(&self) -> Option<target::HostedApplicationIntent> {
        match &self.product {
            CompileProduct::RetainedNativeArtifact(product) => product.application.intent,
            CompileProduct::NativeExecutable(product) => product.application.intent,
            _ => None,
        }
    }

    /// The retained authored application identifier.
    pub const fn application_identifier(&self) -> Option<&target::ApplicationIdentifier> {
        match &self.product {
            CompileProduct::RetainedNativeArtifact(product) => {
                product.application.identifier.as_ref()
            }
            CompileProduct::NativeExecutable(product) => product.application.identifier.as_ref(),
            _ => None,
        }
    }

    /// The normalized Build's two independent optional proof-product
    /// requests retained on this report.
    pub const fn pcc_requests(
        &self,
    ) -> package_compilation::build_execution::build_value::PccRequests {
        self.pcc_requests
    }

    /// Every artifact/`.proof` companion pair published by this report, each
    /// reporting artifact and sidecar byte sizes separately.
    pub fn pcc_publications(&self) -> &[PccPublicationReceipt] {
        &self.pcc_publications
    }

    /// The admission profile under which this report's retained evidence was
    /// produced, retained for pair publication by downstream product routes.
    pub const fn terminal_admission_profile(&self) -> &proof_admission::AdmissionProfile {
        &self.terminal_admission_profile
    }

    pub const fn trust_admission_settlement(&self) -> &trust_model::TrustAdmissionSettlement {
        &self.trust_admission_settlement
    }

    pub fn from_retained_terminal_artifact(
        root_path: PathBuf,
        source_file_count: usize,
        artifact: RetainedTerminalArtifact,
        production_subject: Option<ProductionCompilationSubject>,
    ) -> Result<Self, &'static str> {
        if artifact.native_realization_proposal().is_none() {
            return Err(
                "compiler report requires the Terminal product's native realization proposal",
            );
        }
        artifact
            .validate()
            .map_err(|_| "compiler report received an invalid retained Terminal product")?;
        let production_manifest = production_subject
            .map(|subject| {
                ProductionCompilationManifest::for_terminal(subject, artifact.artifact())
            })
            .transpose()?;
        let mut report = Self::shell(
            root_path,
            source_file_count,
            CompileProduct::TerminalArtifact(TerminalProduct {
                artifact,
                optimization_rollback: None,
            }),
        );
        report.production_manifest = production_manifest;
        report
            .has_consistent_executable_publication_custody()
            .then_some(report)
            .ok_or("compiler report retained inconsistent Terminal-artifact custody")
    }

    pub const fn artifact(&self) -> Option<&terminal_codec::CanonicalTerminalArtifact> {
        match &self.product {
            CompileProduct::TerminalArtifact(product) => Some(product.artifact.artifact()),
            _ => None,
        }
    }

    /// Attach the subtractive build overlay only when the published Psi and
    /// pending native proposal carry its exact effective selection.
    pub fn with_terminal_optimization_rollback(
        mut self,
        rollback: Option<OptimizationRollbackReceipt>,
    ) -> Result<Self, &'static str> {
        let CompileProduct::TerminalArtifact(product) = &mut self.product else {
            return Err("Terminal rollback requires a Terminal product");
        };
        product.optimization_rollback = rollback;
        self.has_consistent_executable_publication_custody()
            .then_some(self)
            .ok_or("Terminal rollback does not match the published optimization selection")
    }

    pub fn terminal_callback_placements(
        &self,
    ) -> Option<&[backend_plan::BoundNominalCallbackPlacement]> {
        self.retained_terminal_artifact()
            .map(RetainedTerminalArtifact::callback_placements)
    }

    /// Borrow the target-constrained native proposal without detaching it from
    /// the retained Terminal product or its report custody.
    pub fn terminal_native_realization_proposal(
        &self,
    ) -> Option<&TerminalNativeRealizationProposal> {
        self.retained_terminal_artifact()
            .and_then(RetainedTerminalArtifact::native_realization_proposal)
    }

    /// Transfer the complete Terminal product without dropping its callback
    /// sidecar. There is deliberately no consuming artifact-only projection.
    pub fn into_retained_terminal_artifact(self) -> Option<RetainedTerminalArtifact> {
        match self.product {
            CompileProduct::TerminalArtifact(product) => Some(product.artifact),
            _ => None,
        }
    }

    /// Transfer the complete non-clonable pre-publication native payload out
    /// of this report. Other requested products return `None`.
    pub fn into_retained_native_artifact(self) -> Option<RetainedNativeArtifact> {
        match self.product {
            CompileProduct::RetainedNativeArtifact(product) => Some(product.artifact),
            _ => None,
        }
    }

    pub fn executable_publication(&self) -> Option<&ExecutablePublicationReceipt> {
        match &self.product {
            CompileProduct::NativeExecutable(product) => Some(&product.executable),
            _ => None,
        }
    }

    /// Returns the exact installed flat executable only after independently
    /// replaying the complete report custody checks. For a published macOS
    /// package this is the inner `Contents/MacOS/<name>` path — the checked
    /// package root is [`Self::checked_native_package_path`]. Object/check-only
    /// reports and any internally drifted receipt graph fail closed.
    pub fn checked_native_executable_path(&self) -> Option<&std::path::Path> {
        if !matches!(self.product, CompileProduct::NativeExecutable(_))
            || !self.has_consistent_executable_publication_custody()
        {
            return None;
        }
        self.executable_publication()
            .map(ExecutablePublicationReceipt::output_path)
    }

    /// Returns the installed `.app` package root only after independently
    /// replaying the complete report custody checks, including the
    /// package/executable cross-binding. Flat publications, non-executable
    /// reports, and any drifted receipt graph expose no path at all.
    pub fn checked_native_package_path(&self) -> Option<&std::path::Path> {
        if !matches!(self.product, CompileProduct::NativeExecutable(_))
            || !self.has_consistent_executable_publication_custody()
        {
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

    /// Replays exact output-product custody. The companion joins are common;
    /// each product kind then replays only the evidence it can hold — the
    /// retained/installation identity joins live beside their payloads.
    pub fn has_consistent_executable_publication_custody(&self) -> bool {
        if self
            .output_kind()
            .admit_proof_requests(self.pcc_requests)
            .is_err()
        {
            return false;
        }
        if self.build_outputs.as_ref().is_some_and(|outputs| {
            self.build_observation != Some(outputs.observation_identity())
                || self.production_manifest.as_ref().is_some_and(|manifest| {
                    manifest.subject().build_observation_identity()
                        != outputs.observation_identity()
                })
        }) {
            return false;
        }
        if self
            .production_manifest
            .as_ref()
            .is_some_and(|manifest| !manifest.validate())
        {
            return false;
        }
        self.product.has_consistent_custody(
            self.wrote_output,
            self.build_outputs.as_ref(),
            self.production_manifest.as_ref(),
        )
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
