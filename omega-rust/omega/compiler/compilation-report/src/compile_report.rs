//! The compile report: which of the five products a compilation produced,
//! the payload slots each kind may occupy, and the publication of retained
//! native artifacts into exact executable bytes with their custody receipts.

use crate::executable_publication::{
    ExecutablePublicationReceipt, appended_file_name_path, executable_container_digest,
    executable_installation_evidence_digest, native_publication_certificate_digest,
    native_publication_evidence_digest, publish_exact_executable_bytes, publish_exact_file_bytes,
    remove_stale_companion, validate_native_pair, validate_psi_pair,
};
use crate::package;
use crate::pcc::build_native_proof_sidecar;
use crate::{
    FinalRealizationEvidenceError, OptimizationRollbackReceipt, PccPublicationReceipt,
    ProductionArtifactIdentity, ProductionCompilationManifest, ProductionCompilationSubject,
    RetainedNativeArtifact, RetainedTerminalArtifact, TerminalNativeRealizationProposal,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileOutputKind {
    CheckOnly,
    TerminalArtifact,
    RetainedNativeArtifact,
    NativeExecutable,
    ObjectContainer,
}

#[derive(Debug)]
pub struct CompileReport {
    root_path: PathBuf,
    pub source_file_count: usize,
    wrote_output: bool,
    /// Exact output category selected by orchestration. This distinguishes a
    /// native executable, which requires publication custody, from the
    /// non-executable object-container fallback.
    output_kind: CompileOutputKind,
    /// Complete validated native payload retained before any output or runtime
    /// installation. Exactly the retained-native output kind owns this value.
    retained_native_artifact: Option<RetainedNativeArtifact>,
    /// Canonical source-free Psi artifact retained at the exact Psi/Omega
    /// ownership seam. It carries no target or deployment authority.
    artifact: Option<RetainedTerminalArtifact>,
    /// Exact checked publication receipt for a native executable image.
    /// Object-container fallbacks and check-only compilations retain `None`.
    executable_publication: Option<ExecutablePublicationReceipt>,
    /// Exact subtractive release overlay applied after build selection and
    /// before native realization. Ordinary requests retain `None`.
    optimization_rollback: Option<OptimizationRollbackReceipt>,
    /// Canonical package-source/build/target/artifact authority for a
    /// package-aware production. Standalone probes carry no such manifest.
    production_manifest: Option<ProductionCompilationManifest>,
    /// Filesystem-free comparison between the request's explicit admissions
    /// and every trust obligation reconstructed by compilation.
    trust_admission_settlement: trust_model::TrustAdmissionSettlement,
    /// The normalized Build's two independent optional proof-product
    /// requests. Both are off by default; neither grants receiving authority.
    pcc_requests: build_evaluation::PccRequests,
    /// The admission profile under which the retained artifact's evidence was
    /// produced. Publication names it as the sidecar's checker profile and
    /// uses it for the producer-side pair validation.
    terminal_admission_profile: proof_admission::AdmissionProfile,
    /// Every artifact/`.proof` companion pair this report published, each
    /// carrying separate artifact and sidecar byte sizes.
    pcc_publications: Vec<PccPublicationReceipt>,
    /// The validated authored `builder.application` name retained with a
    /// retained native artifact. Publication uses it for the `.app` basename
    /// and inner executable leaf when the artifact is a selected macOS GUI
    /// product; `None` means the build declared a non-application role.
    application_name: Option<String>,
    /// The authored hosted intent retained with the native artifact. `Gui`
    /// selects whole-package publication for a Mach-O target; it is semantic
    /// data, never a PE loader word.
    application_intent: Option<build_evaluation::HostedApplicationIntent>,
    /// The authored identifier retained with the native artifact. It supplies
    /// `CFBundleIdentifier` and must agree with the identity already bound
    /// into the signed executable bytes.
    application_identifier: Option<build_evaluation::ApplicationIdentifier>,
    /// Exact checked receipt for a macOS application package publication.
    /// Flat publications and unpublished reports retain `None`.
    package_publication: Option<package::NativePackagePublicationReceipt>,
}

impl CompileReport {
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
        let report = Self {
            root_path,
            source_file_count,
            wrote_output,
            output_kind,
            retained_native_artifact: None,
            artifact: None,
            executable_publication,
            optimization_rollback: None,
            production_manifest: None,
            trust_admission_settlement: Default::default(),
            pcc_requests: build_evaluation::PccRequests::default(),
            terminal_admission_profile: proof_admission::AdmissionProfile::default(),
            pcc_publications: Vec::new(),
            application_name: None,
            application_intent: None,
            application_identifier: None,
            package_publication: None,
        };
        if report.has_consistent_executable_publication_custody() {
            Ok(report)
        } else {
            Err("compiler report retained inconsistent executable publication receipts")
        }
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
        let report = Self {
            root_path,
            source_file_count,
            wrote_output: false,
            output_kind: CompileOutputKind::RetainedNativeArtifact,
            retained_native_artifact: Some(artifact),
            artifact: None,
            executable_publication: None,
            optimization_rollback,
            production_manifest,
            trust_admission_settlement: Default::default(),
            pcc_requests: build_evaluation::PccRequests::default(),
            terminal_admission_profile: proof_admission::AdmissionProfile::default(),
            pcc_publications: Vec::new(),
            application_name: None,
            application_intent: None,
            application_identifier: None,
            package_publication: None,
        };
        if !report.has_consistent_executable_publication_custody() {
            return Err("compiler report retained inconsistent native-artifact custody");
        }
        Ok(report)
    }

    /// Publish the retained native product as one flat executable and replace
    /// pre-publication custody with an exact publication receipt.
    ///
    /// Compilation ends before this operation. Path selection and filesystem
    /// mutation are an explicit product operation, never another compiler
    /// request route.
    ///
    /// A selected macOS GUI product instead installs one complete `.app`
    /// package whose staged tree includes the requested pairs beside the
    /// inner executable. On the flat route an unrequested pair cannot be left
    /// behind: a previous publication's `.psi`/`.proof` companions are removed
    /// before new executable bytes install, so no stale sidecar is ever
    /// associated with bytes this report did not write.
    pub fn publish_retained_native_artifact(
        self,
        build_dir: &std::path::Path,
    ) -> Result<Self, String> {
        if self.output_kind != CompileOutputKind::RetainedNativeArtifact
            || self.wrote_output
            || self.artifact.is_some()
            || self.executable_publication.is_some()
        {
            return Err(
                "native publication requires exactly one retained native artifact".to_owned(),
            );
        }
        let artifact = self.retained_native_artifact.as_ref().ok_or_else(|| {
            "native publication requires exactly one retained native artifact".to_owned()
        })?;
        artifact
            .validate()
            .map_err(|error| format!("refusing to publish an invalid native artifact: {error}"))?;
        if self
            .production_manifest
            .as_ref()
            .is_some_and(|manifest| !manifest.matches_native_artifact(artifact))
        {
            return Err("native publication manifest disagrees with retained artifact".to_owned());
        }
        let native_artifact_identity = *artifact.identity().as_bytes();

        let output = artifact.image().output();
        if std::path::Path::new(&output.file_name).components().count() != 1 {
            return Err("native artifact supplied a non-local output filename".to_owned());
        }
        let text_validation_digest = output
            .compiler_text_validation
            .map(|validation| validation.derivation_digest)
            .ok_or_else(|| {
                "native publication requires strong compiler-text validation evidence".to_owned()
            })?;
        let function_validation = output.compiler_function_validation.ok_or_else(|| {
            "native publication requires compiler-function validation evidence".to_owned()
        })?;
        let function_validation_digest = function_validation.evidence_digest();
        let function_validation_report_fingerprint =
            function_validation.evidence_report_fingerprint();
        let boundary_contract_report_fingerprint =
            function_validation.boundary_contract_report_fingerprint;

        // A requested native pair is built and self-checked before any byte
        // is installed: the placed-image evidence must decode canonically and
        // replay against the exact bytes about to be published under a
        // self-consistent policy, so a pair that cannot carry its bounded
        // claim never produces a certified-looking install. The behavioral
        // remainder of a native claim still reports `Incomplete` to receivers;
        // that is the honest ceiling of the shipped evidence, and `Reject`
        // here would be a producer defect rather than a publishable pair.
        let native_sidecar_bytes = if self.pcc_requests.native {
            let sidecar = build_native_proof_sidecar(
                artifact,
                &self.terminal_admission_profile,
                &output.bytes,
            )?;
            validate_native_pair(&output.bytes, &sidecar, &self.terminal_admission_profile)?;
            Some(sidecar.to_bytes())
        } else {
            None
        };

        std::fs::create_dir_all(build_dir).map_err(|error| {
            format!(
                "failed to create output directory {}: {error}",
                build_dir.display()
            )
        })?;

        // Complete selected macOS GUI output is one `.app` package; every
        // other selected product keeps the flat executable path. The
        // retained authored name and identifier — not the emitted leaf or a
        // re-derived folder name — supply the package shape.
        let package = artifact.target().object_format == target::ObjectFormat::MachO
            && matches!(
                self.application_intent,
                Some(build_evaluation::HostedApplicationIntent::Gui)
            );
        let container_digest = executable_container_digest(&output.bytes);
        let mut pcc_publications = Vec::new();
        let mut package_publication = None;
        let output_path = if package {
            let application_name = self.application_name.clone().ok_or_else(|| {
                "macOS GUI package publication requires the retained authored application name"
                    .to_owned()
            })?;
            let application_identifier =
                self.application_identifier.clone().ok_or_else(|| {
                    "macOS GUI package publication requires the retained authored application identifier"
                        .to_owned()
                })?;
            let mut companions = Vec::new();
            // Requested artifact/companion pairs assemble inside the package
            // beside the executable: one staged tree, one rename, so a failed
            // pair can never leave a certified-looking install.
            if self.pcc_requests.psi {
                let psi_bytes = artifact.psi_artifact().to_bytes();
                let psi_sidecar = terminal_codec::build_psi_proof_sidecar(
                    artifact.psi_artifact(),
                    &self.terminal_admission_profile,
                    &psi_bytes,
                )
                .map_err(|error| format!("cannot build the psi proof sidecar: {error}"))?;
                let psi_sidecar_bytes = psi_sidecar.to_bytes();
                validate_psi_pair(&psi_bytes, &psi_sidecar, &self.terminal_admission_profile)?;
                companions.push((".psi".to_owned(), psi_bytes.clone()));
                companions.push((".psi.proof".to_owned(), psi_sidecar_bytes.clone()));
                let package_root = build_dir.join(format!("{application_name}.app"));
                let macos_dir = std::path::Path::new("Contents").join("MacOS");
                pcc_publications.push(PccPublicationReceipt {
                    product: terminal_codec::PccProductKind::Psi,
                    artifact_path: package_root
                        .join(macos_dir.join(format!("{application_name}.psi"))),
                    artifact_byte_len: psi_bytes.len() as u64,
                    sidecar_path: package_root
                        .join(macos_dir.join(format!("{application_name}.psi.proof"))),
                    sidecar_byte_len: psi_sidecar_bytes.len() as u64,
                });
            }
            // The native companion sits beside the inner executable as
            // `Contents/MacOS/<name>.proof`; the staged rename makes its
            // install atomic with the executable it commits to.
            if let Some(native_sidecar_bytes) = &native_sidecar_bytes {
                companions.push((".proof".to_owned(), native_sidecar_bytes.clone()));
                let package_root = build_dir.join(format!("{application_name}.app"));
                let macos_dir = std::path::Path::new("Contents").join("MacOS");
                pcc_publications.push(PccPublicationReceipt {
                    product: terminal_codec::PccProductKind::Native,
                    artifact_path: package_root.join(macos_dir.join(&application_name)),
                    artifact_byte_len: output.bytes.len() as u64,
                    sidecar_path: package_root
                        .join(macos_dir.join(format!("{application_name}.proof"))),
                    sidecar_byte_len: native_sidecar_bytes.len() as u64,
                });
            }
            let receipt = package::publish_macos_application_package(
                build_dir,
                &application_name,
                &application_identifier,
                &output.bytes,
                container_digest,
                &companions,
            )?;
            let output_path = receipt.inner_executable_path().ok_or_else(|| {
                "macOS package publication produced no inner executable component".to_owned()
            })?;
            package_publication = Some(receipt);
            output_path
        } else {
            let output_path = build_dir.join(&output.file_name);

            // Every requested artifact/companion pair is staged, validated and
            // published before the executable itself becomes visible. A failed
            // pair therefore never produces a certified-looking install, and a
            // sidecar can never be left bound to bytes this report did not write.
            if self.pcc_requests.psi {
                let psi_bytes = artifact.psi_artifact().to_bytes();
                let psi_sidecar = terminal_codec::build_psi_proof_sidecar(
                    artifact.psi_artifact(),
                    &self.terminal_admission_profile,
                    &psi_bytes,
                )
                .map_err(|error| format!("cannot build the psi proof sidecar: {error}"))?;
                let psi_sidecar_bytes = psi_sidecar.to_bytes();
                validate_psi_pair(&psi_bytes, &psi_sidecar, &self.terminal_admission_profile)?;
                let psi_path = appended_file_name_path(&output_path, ".psi");
                publish_exact_file_bytes(&psi_path, &psi_bytes)?;
                let psi_sidecar_path = appended_file_name_path(&psi_path, ".proof");
                publish_exact_file_bytes(&psi_sidecar_path, &psi_sidecar_bytes)?;
                pcc_publications.push(PccPublicationReceipt {
                    product: terminal_codec::PccProductKind::Psi,
                    artifact_path: psi_path,
                    artifact_byte_len: psi_bytes.len() as u64,
                    sidecar_path: psi_sidecar_path,
                    sidecar_byte_len: psi_sidecar_bytes.len() as u64,
                });
            } else {
                // An earlier Psi-requested publication's companions must not
                // survive beside an executable they do not commit to; removing
                // them cannot create a certified-looking install, so removal
                // precedes the new bytes. Package publication needs no such
                // pass: the staged tree rename replaces every prior member.
                let psi_path = appended_file_name_path(&output_path, ".psi");
                remove_stale_companion(&appended_file_name_path(&psi_path, ".proof"))?;
                remove_stale_companion(&psi_path)?;
            }
            if let Some(native_sidecar_bytes) = &native_sidecar_bytes {
                let native_sidecar_path = appended_file_name_path(&output_path, ".proof");
                publish_exact_file_bytes(&native_sidecar_path, native_sidecar_bytes)?;
                pcc_publications.push(PccPublicationReceipt {
                    product: terminal_codec::PccProductKind::Native,
                    artifact_path: output_path.clone(),
                    artifact_byte_len: output.bytes.len() as u64,
                    sidecar_path: native_sidecar_path,
                    sidecar_byte_len: native_sidecar_bytes.len() as u64,
                });
            } else {
                // An earlier native-PCC publication's sidecar must not
                // survive beside executable bytes it does not commit to.
                remove_stale_companion(&appended_file_name_path(&output_path, ".proof"))?;
            }
            publish_exact_executable_bytes(&output_path, &output.bytes)?;
            output_path
        };

        let certificate_digest = native_publication_certificate_digest(
            &native_artifact_identity,
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            artifact.target(),
            artifact.image().final_image_symbol_digest(),
            boundary_contract_report_fingerprint,
            text_validation_digest,
            function_validation_digest,
            function_validation_report_fingerprint,
            output.executable_regions.inventory_digest,
            output.executable_regions.inventory_report_fingerprint,
        );
        let publication_evidence_digest = native_publication_evidence_digest(
            &native_artifact_identity,
            certificate_digest,
            output.callback_placement_identity_report_fingerprint,
            output.executable_regions.inventory_digest,
            output.executable_regions.inventory_report_fingerprint,
            text_validation_digest,
            function_validation_digest,
            function_validation_report_fingerprint,
            output.bytes.len(),
            container_digest,
        );
        let installation_evidence_digest = executable_installation_evidence_digest(
            publication_evidence_digest,
            output.callback_placement_identity_report_fingerprint,
            &output_path,
            output.bytes.len(),
            container_digest,
        );
        let receipt = ExecutablePublicationReceipt::new(
            output_path,
            native_artifact_identity,
            certificate_digest,
            output.callback_placement_identity_report_fingerprint,
            boundary_contract_report_fingerprint,
            output.executable_regions.inventory_digest,
            output.executable_regions.inventory_report_fingerprint,
            text_validation_digest,
            function_validation_digest,
            function_validation_report_fingerprint,
            publication_evidence_digest,
            output.bytes.len(),
            container_digest,
            installation_evidence_digest,
        );
        if !receipt.has_consistent_installation_identity() {
            return Err("native publication produced an inconsistent installation receipt".into());
        }

        let report = Self {
            root_path: self.root_path,
            source_file_count: self.source_file_count,
            wrote_output: true,
            output_kind: CompileOutputKind::NativeExecutable,
            retained_native_artifact: None,
            artifact: None,
            executable_publication: Some(receipt),
            optimization_rollback: self.optimization_rollback,
            production_manifest: self.production_manifest,
            trust_admission_settlement: self.trust_admission_settlement,
            pcc_requests: self.pcc_requests,
            terminal_admission_profile: self.terminal_admission_profile,
            pcc_publications,
            application_name: self.application_name,
            application_intent: self.application_intent,
            application_identifier: self.application_identifier,
            package_publication,
        };
        if !report.has_consistent_executable_publication_custody() {
            return Err("published native report failed custody replay".to_owned());
        }
        Ok(report)
    }

    /// Publish the retained Terminal product as one `<root>.psi` artifact,
    /// plus its adjacent `.proof` companion when the normalized Build
    /// requested Psi PCC. The artifact is staged, validated and replayed
    /// before the pair is reported. A native proof request reports
    /// `Incomplete` rather than downgrading to custody-only success, and an
    /// earlier publication's unrequested `.proof` is removed before the new
    /// artifact installs so no stale sidecar binds the new bytes.
    pub fn publish_retained_terminal_artifact(
        self,
        build_dir: &std::path::Path,
    ) -> Result<Self, String> {
        if self.output_kind != CompileOutputKind::TerminalArtifact
            || self.wrote_output
            || self.retained_native_artifact.is_some()
            || self.executable_publication.is_some()
        {
            return Err(
                "terminal publication requires exactly one retained terminal artifact".to_owned(),
            );
        }
        // A Terminal stop cannot satisfy a native proof-product request; the
        // compile route rejects this earlier, and the report fails closed
        // rather than silently downgrading the requested pair.
        if self.pcc_requests.native {
            let outcome = terminal_codec::PccVerificationOutcome::Incomplete(
                terminal_codec::PccIncompleteness::UnsupportedEvidence {
                    product: terminal_codec::PccProductKind::Native,
                },
            );
            return Err(format!(
                "native PCC publication is {outcome:?}: a Terminal stop carries no native product"
            ));
        }
        let artifact = self.artifact.as_ref().ok_or_else(|| {
            "terminal publication requires exactly one retained terminal artifact".to_owned()
        })?;
        artifact.validate().map_err(|error| {
            format!("refusing to publish an invalid terminal artifact: {error}")
        })?;
        let stem = self
            .root_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.is_empty())
            .ok_or_else(|| "terminal publication requires a named root file".to_owned())?;
        std::fs::create_dir_all(build_dir).map_err(|error| {
            format!(
                "failed to create output directory {}: {error}",
                build_dir.display()
            )
        })?;
        let psi_path = build_dir.join(format!("{stem}.psi"));
        let psi_bytes = artifact.artifact().to_bytes();
        let mut pcc_publications = Vec::new();
        // The companion is built and validated before any pair bytes become
        // visible, so a failed pair never leaves a certified-looking artifact.
        let psi_sidecar_bytes = if self.pcc_requests.psi {
            let psi_sidecar = terminal_codec::build_psi_proof_sidecar(
                artifact.artifact(),
                &self.terminal_admission_profile,
                &psi_bytes,
            )
            .map_err(|error| format!("cannot build the psi proof sidecar: {error}"))?;
            let psi_sidecar_bytes = psi_sidecar.to_bytes();
            validate_psi_pair(&psi_bytes, &psi_sidecar, &self.terminal_admission_profile)?;
            Some(psi_sidecar_bytes)
        } else {
            // Without a Psi proof request no `.proof` companion may remain
            // beside the artifact bytes about to be replaced: an earlier
            // publication's sidecar commits to bytes this run did not write.
            remove_stale_companion(&appended_file_name_path(&psi_path, ".proof"))?;
            None
        };
        publish_exact_file_bytes(&psi_path, &psi_bytes)?;
        if let Some(psi_sidecar_bytes) = psi_sidecar_bytes {
            let psi_sidecar_path = appended_file_name_path(&psi_path, ".proof");
            publish_exact_file_bytes(&psi_sidecar_path, &psi_sidecar_bytes)?;
            pcc_publications.push(PccPublicationReceipt {
                product: terminal_codec::PccProductKind::Psi,
                artifact_path: psi_path.clone(),
                artifact_byte_len: psi_bytes.len() as u64,
                sidecar_path: psi_sidecar_path,
                sidecar_byte_len: psi_sidecar_bytes.len() as u64,
            });
        }
        Ok(Self {
            root_path: self.root_path,
            source_file_count: self.source_file_count,
            wrote_output: true,
            output_kind: CompileOutputKind::TerminalArtifact,
            retained_native_artifact: None,
            artifact: self.artifact,
            executable_publication: None,
            optimization_rollback: self.optimization_rollback,
            production_manifest: self.production_manifest,
            trust_admission_settlement: self.trust_admission_settlement,
            pcc_requests: self.pcc_requests,
            terminal_admission_profile: self.terminal_admission_profile,
            pcc_publications,
            application_name: self.application_name,
            application_intent: self.application_intent,
            application_identifier: self.application_identifier,
            package_publication: None,
        })
    }

    pub fn root_path(&self) -> &std::path::Path {
        &self.root_path
    }

    pub const fn wrote_output(&self) -> bool {
        self.wrote_output
    }

    pub const fn output_kind(&self) -> CompileOutputKind {
        self.output_kind
    }

    pub const fn retained_native_artifact(&self) -> Option<&RetainedNativeArtifact> {
        self.retained_native_artifact.as_ref()
    }

    pub const fn optimization_rollback_receipt(&self) -> Option<&OptimizationRollbackReceipt> {
        self.optimization_rollback.as_ref()
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
        if self.output_kind != CompileOutputKind::RetainedNativeArtifact {
            return Err(FinalRealizationEvidenceError::RetainedNativeArtifactRequired);
        }
        let artifact = self
            .retained_native_artifact
            .as_ref()
            .ok_or(FinalRealizationEvidenceError::RetainedNativeArtifactRequired)?;
        let manifest = self
            .production_manifest
            .as_ref()
            .ok_or(FinalRealizationEvidenceError::PackageProductionManifestRequired)?;
        manifest.require_native_physical_evidence(artifact)
    }

    pub fn with_trust_admission_settlement(
        mut self,
        settlement: trust_model::TrustAdmissionSettlement,
    ) -> Self {
        self.trust_admission_settlement = settlement;
        self
    }

    /// Retain the normalized Build's optional proof-product requests and the
    /// admission profile that produced the retained evidence. Publication
    /// uses these to emit and self-validate every requested pair; they confer
    /// no receiving authority.
    pub fn with_pcc_context(
        mut self,
        pcc_requests: build_evaluation::PccRequests,
        terminal_admission_profile: proof_admission::AdmissionProfile,
    ) -> Self {
        self.pcc_requests = pcc_requests;
        self.terminal_admission_profile = terminal_admission_profile;
        self
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
        application_name: Option<String>,
        application_intent: Option<build_evaluation::HostedApplicationIntent>,
        application_identifier: Option<build_evaluation::ApplicationIdentifier>,
    ) -> Result<Self, &'static str> {
        if self.output_kind != CompileOutputKind::RetainedNativeArtifact {
            return Err("application publication metadata requires a retained native artifact");
        }
        self.application_name = application_name;
        self.application_intent = application_intent;
        self.application_identifier = application_identifier;
        self.has_consistent_executable_publication_custody()
            .then_some(self)
            .ok_or("application publication metadata left inconsistent report custody")
    }

    /// The retained authored `builder.application` name, when the build
    /// declared an application role.
    pub fn application_name(&self) -> Option<&str> {
        self.application_name.as_deref()
    }

    /// The retained authored hosted intent.
    pub const fn application_intent(&self) -> Option<build_evaluation::HostedApplicationIntent> {
        self.application_intent
    }

    /// The retained authored application identifier.
    pub const fn application_identifier(&self) -> Option<&build_evaluation::ApplicationIdentifier> {
        self.application_identifier.as_ref()
    }

    /// The normalized Build's two independent optional proof-product
    /// requests retained on this report.
    pub const fn pcc_requests(&self) -> build_evaluation::PccRequests {
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
        let report = Self {
            root_path,
            source_file_count,
            wrote_output: false,
            output_kind: CompileOutputKind::TerminalArtifact,
            retained_native_artifact: None,
            artifact: Some(artifact),
            executable_publication: None,
            optimization_rollback: None,
            production_manifest,
            trust_admission_settlement: Default::default(),
            pcc_requests: build_evaluation::PccRequests::default(),
            terminal_admission_profile: proof_admission::AdmissionProfile::default(),
            pcc_publications: Vec::new(),
            application_name: None,
            application_intent: None,
            application_identifier: None,
            package_publication: None,
        };
        report
            .has_consistent_executable_publication_custody()
            .then_some(report)
            .ok_or("compiler report retained inconsistent Terminal-artifact custody")
    }

    pub const fn artifact(&self) -> Option<&terminal_codec::CanonicalTerminalArtifact> {
        match &self.artifact {
            Some(retained) => Some(retained.artifact()),
            None => None,
        }
    }

    /// Attach the subtractive build overlay only when the published Psi and
    /// pending native proposal carry its exact effective selection.
    pub fn with_terminal_optimization_rollback(
        mut self,
        rollback: Option<OptimizationRollbackReceipt>,
    ) -> Result<Self, &'static str> {
        if self.output_kind != CompileOutputKind::TerminalArtifact {
            return Err("Terminal rollback requires a Terminal product");
        }
        self.optimization_rollback = rollback;
        self.has_consistent_executable_publication_custody()
            .then_some(self)
            .ok_or("Terminal rollback does not match the published optimization selection")
    }

    pub fn terminal_callback_placements(
        &self,
    ) -> Option<&[backend_plan::BoundNominalCallbackPlacement]> {
        self.artifact
            .as_ref()
            .map(RetainedTerminalArtifact::callback_placements)
    }

    /// Borrow the target-constrained native proposal without detaching it from
    /// the retained Terminal product or its report custody.
    pub fn terminal_native_realization_proposal(
        &self,
    ) -> Option<&TerminalNativeRealizationProposal> {
        self.artifact
            .as_ref()
            .and_then(RetainedTerminalArtifact::native_realization_proposal)
    }

    /// Transfer the complete Terminal product without dropping its callback
    /// sidecar. There is deliberately no consuming artifact-only projection.
    pub fn into_retained_terminal_artifact(self) -> Option<RetainedTerminalArtifact> {
        self.artifact
    }

    /// Transfer the complete non-clonable pre-publication native payload out
    /// of this report. Other requested products return `None`.
    pub fn into_retained_native_artifact(self) -> Option<RetainedNativeArtifact> {
        self.retained_native_artifact
    }

    pub fn executable_publication(&self) -> Option<&ExecutablePublicationReceipt> {
        self.executable_publication.as_ref()
    }

    /// Returns the exact installed flat executable only after independently
    /// replaying the complete report custody checks. For a published macOS
    /// package this is the inner `Contents/MacOS/<name>` path — the checked
    /// package root is [`Self::checked_native_package_path`]. Object/check-only
    /// reports and any internally drifted receipt graph fail closed.
    pub fn checked_native_executable_path(&self) -> Option<&std::path::Path> {
        if self.output_kind != CompileOutputKind::NativeExecutable
            || !self.has_consistent_executable_publication_custody()
        {
            return None;
        }
        self.executable_publication
            .as_ref()
            .map(ExecutablePublicationReceipt::output_path)
    }

    /// Returns the installed `.app` package root only after independently
    /// replaying the complete report custody checks, including the
    /// package/executable cross-binding. Flat publications, non-executable
    /// reports, and any drifted receipt graph expose no path at all.
    pub fn checked_native_package_path(&self) -> Option<&std::path::Path> {
        if self.output_kind != CompileOutputKind::NativeExecutable
            || !self.has_consistent_executable_publication_custody()
        {
            return None;
        }
        self.package_publication
            .as_ref()
            .map(package::NativePackagePublicationReceipt::package_root)
    }

    /// The checked package receipt for a published `.app`, when this report
    /// installed one.
    pub fn package_publication(&self) -> Option<&package::NativePackagePublicationReceipt> {
        self.package_publication.as_ref()
    }

    /// Replays exact output-product cardinality. A retained native artifact is
    /// mutually exclusive with publication, native output checks the flat
    /// executable receipt, and terminal output replays
    /// the retained installation/image/file join.
    pub fn has_consistent_executable_publication_custody(&self) -> bool {
        let rollback_matches_kind = match self.output_kind {
            CompileOutputKind::RetainedNativeArtifact | CompileOutputKind::NativeExecutable => self
                .optimization_rollback
                .as_ref()
                .is_none_or(OptimizationRollbackReceipt::is_consistent),
            CompileOutputKind::TerminalArtifact => {
                self.optimization_rollback.as_ref().is_none_or(|receipt| {
                    receipt.is_consistent()
                        && receipt.requested_disabled().as_slice().iter().all(|rule| {
                            rule.execution_phase()
                                == optimization_core::OptimizationExecutionPhase::Psi
                        })
                        && self.artifact().is_some_and(|artifact| {
                            artifact.optimization().selection()
                                == receipt.effective().project_psi().selections().identity()
                        })
                        && self
                            .terminal_native_realization_proposal()
                            .is_some_and(|proposal| {
                                proposal.post_terminal_optimizations()
                                    == &receipt.effective().project_post_terminal()
                            })
                })
            }
            CompileOutputKind::CheckOnly | CompileOutputKind::ObjectContainer => {
                self.optimization_rollback.is_none()
            }
        };
        if !rollback_matches_kind {
            return false;
        }
        if self
            .production_manifest
            .as_ref()
            .is_some_and(|manifest| !manifest.validate())
        {
            return false;
        }
        match self.output_kind {
            CompileOutputKind::CheckOnly => {
                !self.wrote_output
                    && self.artifact.is_none()
                    && self.retained_native_artifact.is_none()
                    && self.executable_publication.is_none()
                    && self.package_publication.is_none()
            }
            CompileOutputKind::TerminalArtifact => {
                !self.wrote_output
                    && self.retained_native_artifact.is_none()
                    && self
                        .artifact
                        .as_ref()
                        .is_some_and(|artifact| artifact.validate().is_ok())
                    && self.executable_publication.is_none()
                    && self.package_publication.is_none()
                    && self.production_manifest.as_ref().is_none_or(|manifest| {
                        self.artifact
                            .as_ref()
                            .is_some_and(|artifact| {
                                manifest.matches_terminal_artifact(artifact.artifact())
                                    && artifact.native_realization_proposal().is_some_and(
                                        |proposal| {
                                            proposal.target_profile()
                                                == manifest.subject().target_profile()
                                                && proposal.native_target()
                                                    == manifest.subject().native_target()
                                        },
                                    )
                            })
                    })
            }
            CompileOutputKind::RetainedNativeArtifact => {
                !self.wrote_output
                    && self.artifact.is_none()
                    && self
                        .retained_native_artifact
                        .as_ref()
                        .is_some_and(|artifact| artifact.validate().is_ok())
                    && self.executable_publication.is_none()
                    && self.package_publication.is_none()
                    && self.production_manifest.as_ref().is_none_or(|manifest| {
                        self.retained_native_artifact
                            .as_ref()
                            .is_some_and(|artifact| manifest.matches_native_artifact(artifact))
                    })
            }
            CompileOutputKind::NativeExecutable => {
                self.wrote_output
                    && self.artifact.is_none()
                    && self.retained_native_artifact.is_none()
                    && self.executable_publication.as_ref().is_some_and(|receipt| {
                        receipt.has_consistent_installation_identity()
                    })
                    && self.package_publication.as_ref().is_none_or(|package| {
                        package.has_consistent_package_identity()
                            && self.executable_publication.as_ref().is_some_and(|receipt| {
                                package.inner_executable_path().as_deref()
                                    == Some(receipt.output_path())
                                    && package.executable_container_digest()
                                        == receipt.container_digest()
                                    && package.executable_byte_count()
                                        == receipt.container_byte_count()
                            })
                            && self.application_name.as_deref()
                                == Some(package.application_name())
                            && self.application_identifier.as_ref()
                                == Some(package.application_identifier())
                    })
                    && self.production_manifest.as_ref().is_none_or(|manifest| {
                        matches!(manifest.artifact(), ProductionArtifactIdentity::Native(identity)
                            if self.executable_publication.as_ref().is_some_and(|receipt| receipt.native_artifact_identity() == identity.as_bytes()))
                    })
            }
            CompileOutputKind::ObjectContainer => {
                self.wrote_output
                    && self.artifact.is_none()
                    && self.retained_native_artifact.is_none()
                    && self.executable_publication.is_none()
                    && self.package_publication.is_none()
            }
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
mod custody_tests;
#[cfg(test)]
mod tests;
