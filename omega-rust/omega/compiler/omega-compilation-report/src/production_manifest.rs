use omega_build_evaluation::{
    BuildEvaluationUsage, BuildObservationIdentity, BuildObservationSummary,
};
use omega_native_artifact::{NativeArtifact, NativeArtifactIdentity, NativePhysicalEvidence};
use omega_package_compilation::PackageCompilationSubject;
use psi_terminal_codec::{CanonicalTerminalArtifact, TerminalArtifactIdentity};
use sha2::{Digest, Sha256};

const MANIFEST_DOMAIN: &[u8] = b"OMEGA-PRODUCTION-COMPILATION-MANIFEST-V9\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProductionCompilationManifestIdentity([u8; 32]);

impl ProductionCompilationManifestIdentity {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionArtifactIdentity {
    Terminal(TerminalArtifactIdentity),
    Native(NativeArtifactIdentity),
}

/// Fail-closed rejection from the narrow package/native physical-evidence
/// join. This does not claim that a package was audited or that the complete
/// executable is correct; it only admits the exact native physical evidence
/// already carried by the matching artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalRealizationEvidenceError {
    InvalidReportCustody,
    RetainedNativeArtifactRequired,
    PackageProductionManifestRequired,
    InvalidProductionManifest,
    InvalidNativeArtifact,
    NativeTargetMismatch,
    NativeArtifactMismatch,
    NativePhysicalEvidenceUnavailable,
}

impl std::fmt::Display for FinalRealizationEvidenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidReportCustody => "compiler report has inconsistent production custody",
            Self::RetainedNativeArtifactRequired => {
                "final-realization evidence requires a retained native artifact"
            }
            Self::PackageProductionManifestRequired => {
                "final-realization evidence requires package-aware production"
            }
            Self::InvalidProductionManifest => "production compilation manifest is invalid",
            Self::InvalidNativeArtifact => "native artifact is invalid",
            Self::NativeTargetMismatch => {
                "production compilation manifest and native artifact target disagree"
            }
            Self::NativeArtifactMismatch => {
                "production compilation manifest and native artifact identity disagree"
            }
            Self::NativePhysicalEvidenceUnavailable => {
                "native artifact carries no physical evidence for this realization"
            }
        })
    }
}

impl std::error::Error for FinalRealizationEvidenceError {}

/// Exact artifact-free compilation custody captured before checked state is
/// consumed by Terminal/native production.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionCompilationSubject {
    package: PackageCompilationSubject,
    selected_build_machine_identity: String,
    build_evaluation_usage: BuildEvaluationUsage,
    build_observation_identity: BuildObservationIdentity,
    target_profile: omega_target::TargetProfile,
    native_target: omega_target::NativeTarget,
}

impl ProductionCompilationSubject {
    #[doc(hidden)]
    pub fn from_checked(
        package: PackageCompilationSubject,
        selected_build_machine_identity: String,
        build_evaluation_usage: BuildEvaluationUsage,
        build_observation: &BuildObservationSummary,
        target_profile: omega_target::TargetProfile,
        native_target: omega_target::NativeTarget,
    ) -> Result<Self, &'static str> {
        if selected_build_machine_identity.is_empty() {
            return Err("production compilation subject has an empty build-machine identity");
        }
        if target_profile.native_target() != native_target {
            return Err(
                "production compilation subject target profile disagrees with native target",
            );
        }
        if build_evaluation_usage.invocation_fuel_ceiling == 0 {
            return Err("production compilation subject has a zero invocation fuel ceiling");
        }
        if build_evaluation_usage.fuel_units > build_evaluation_usage.invocation_fuel_ceiling
            || build_evaluation_usage.replay_fuel_units
                > build_evaluation_usage.invocation_fuel_ceiling
        {
            return Err("production compilation subject exceeded its invocation fuel ceiling");
        }
        match (
            build_evaluation_usage.sponsor_schema_version,
            build_evaluation_usage.session_fuel_ceiling,
            build_evaluation_usage.session_build_log_byte_ceiling,
            build_evaluation_usage.session_filesystem_attempt_ceiling,
            build_evaluation_usage.session_live_filesystem_handle_ceiling,
            build_evaluation_usage.session_live_cell_ceiling,
            build_evaluation_usage.session_live_text_byte_ceiling,
            build_evaluation_usage.session_result_cell_ceiling,
            build_evaluation_usage.session_result_text_byte_ceiling,
        ) {
            (None, None, None, None, None, None, None, None, None) => {
                if build_evaluation_usage.session_peak_live_filesystem_handles != 0
                    || build_evaluation_usage.session_peak_live_cells != 0
                    || build_evaluation_usage.session_peak_live_text_bytes != 0
                {
                    return Err(
                        "production compilation subject has unsponsored session-live usage",
                    );
                }
            }
            (Some(_), Some(0), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_)) => {
                return Err("production compilation subject has a zero session fuel ceiling");
            }
            (Some(_), Some(_), Some(0), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_)) => {
                return Err("production compilation subject has a zero session BuildLog ceiling");
            }
            (Some(_), Some(_), Some(_), Some(0), Some(_), Some(_), Some(_), Some(_), Some(_)) => {
                return Err(
                    "production compilation subject has a zero session filesystem-attempt ceiling",
                );
            }
            (Some(_), Some(_), Some(_), Some(_), Some(0), Some(_), Some(_), Some(_), Some(_)) => {
                return Err(
                    "production compilation subject has a zero live-filesystem-handle ceiling",
                );
            }
            (Some(_), Some(_), Some(_), Some(_), Some(_), Some(0), Some(_), Some(_), Some(_)) => {
                return Err("production compilation subject has a zero live-cell ceiling");
            }
            (Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(0), Some(_), Some(_)) => {
                return Err("production compilation subject has a zero live-Text-byte ceiling");
            }
            (Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(0), Some(_)) => {
                return Err("production compilation subject has a zero result-cell ceiling");
            }
            (Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(0)) => {
                return Err("production compilation subject has a zero result-Text-byte ceiling");
            }
            (
                Some(_),
                Some(session_ceiling),
                Some(build_log_ceiling),
                Some(filesystem_attempt_ceiling),
                Some(live_filesystem_handle_ceiling),
                Some(live_cell_ceiling),
                Some(live_text_byte_ceiling),
                Some(result_cell_ceiling),
                Some(result_text_byte_ceiling),
            ) => {
                let consumed = build_evaluation_usage
                    .fuel_units
                    .checked_add(build_evaluation_usage.replay_fuel_units)
                    .ok_or("production compilation subject build fuel overflowed")?;
                if consumed > session_ceiling {
                    return Err("production compilation subject exceeded its session fuel ceiling");
                }
                let build_log = build_evaluation_usage
                    .build_log_bytes
                    .checked_add(build_evaluation_usage.replay_build_log_bytes)
                    .ok_or("production compilation subject BuildLog accounting overflowed")?;
                if build_log > build_log_ceiling {
                    return Err(
                        "production compilation subject exceeded its session BuildLog ceiling",
                    );
                }
                let filesystem_attempts = build_evaluation_usage
                    .filesystem_operation_attempts
                    .checked_add(build_evaluation_usage.replay_filesystem_operation_attempts)
                    .ok_or(
                        "production compilation subject filesystem-attempt accounting overflowed",
                    )?;
                if filesystem_attempts > filesystem_attempt_ceiling {
                    return Err(
                        "production compilation subject exceeded its session filesystem-attempt ceiling",
                    );
                }
                if build_evaluation_usage.session_peak_live_filesystem_handles
                    > live_filesystem_handle_ceiling
                {
                    return Err(
                        "production compilation subject exceeded its live-filesystem-handle ceiling",
                    );
                }
                if build_evaluation_usage.session_peak_live_cells > live_cell_ceiling
                    || build_evaluation_usage.peak_live_cells > live_cell_ceiling
                    || build_evaluation_usage.replay_peak_live_cells > live_cell_ceiling
                {
                    return Err("production compilation subject exceeded its live-cell ceiling");
                }
                if build_evaluation_usage.peak_live_cells
                    > build_evaluation_usage.session_peak_live_cells
                    || build_evaluation_usage.replay_peak_live_cells
                        > build_evaluation_usage.session_peak_live_cells
                {
                    return Err(
                        "production compilation subject live-cell peak exceeds its session peak",
                    );
                }
                if build_evaluation_usage.session_peak_live_text_bytes > live_text_byte_ceiling
                    || build_evaluation_usage.peak_live_text_bytes > live_text_byte_ceiling
                    || build_evaluation_usage.replay_peak_live_text_bytes > live_text_byte_ceiling
                {
                    return Err(
                        "production compilation subject exceeded its live-Text-byte ceiling",
                    );
                }
                if build_evaluation_usage.peak_live_text_bytes
                    > build_evaluation_usage.session_peak_live_text_bytes
                    || build_evaluation_usage.replay_peak_live_text_bytes
                        > build_evaluation_usage.session_peak_live_text_bytes
                {
                    return Err(
                        "production compilation subject live-Text-byte peak exceeds its session peak",
                    );
                }
                let result_cells = build_evaluation_usage
                    .result_cells
                    .checked_add(build_evaluation_usage.replay_result_cells)
                    .ok_or("production compilation subject result-cell accounting overflowed")?;
                if result_cells > result_cell_ceiling {
                    return Err("production compilation subject exceeded its result-cell ceiling");
                }
                let result_text_bytes = build_evaluation_usage
                    .result_text_bytes
                    .checked_add(build_evaluation_usage.replay_result_text_bytes)
                    .ok_or("production compilation subject result-Text accounting overflowed")?;
                if result_text_bytes > result_text_byte_ceiling {
                    return Err(
                        "production compilation subject exceeded its result-Text-byte ceiling",
                    );
                }
            }
            _ => {
                return Err(
                    "production compilation subject has incomplete evaluation sponsor identity",
                );
            }
        }
        Ok(Self {
            package,
            selected_build_machine_identity,
            build_evaluation_usage,
            build_observation_identity: build_observation.identity(),
            target_profile,
            native_target,
        })
    }

    pub const fn package(&self) -> &PackageCompilationSubject {
        &self.package
    }

    pub fn selected_build_machine_identity(&self) -> &str {
        &self.selected_build_machine_identity
    }

    pub const fn build_evaluation_usage(&self) -> BuildEvaluationUsage {
        self.build_evaluation_usage
    }

    pub const fn build_observation_identity(&self) -> BuildObservationIdentity {
        self.build_observation_identity
    }

    pub const fn target_profile(&self) -> omega_target::TargetProfile {
        self.target_profile
    }

    pub const fn native_target(&self) -> omega_target::NativeTarget {
        self.native_target
    }
}

/// One canonical package-source/build/target/artifact join retained by the
/// real production report. Human views may decode this authority; they cannot
/// define a parallel schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionCompilationManifest {
    subject: ProductionCompilationSubject,
    artifact: ProductionArtifactIdentity,
    canonical_bytes: Vec<u8>,
    identity: ProductionCompilationManifestIdentity,
}

impl ProductionCompilationManifest {
    pub fn for_terminal(
        subject: ProductionCompilationSubject,
        artifact: &CanonicalTerminalArtifact,
    ) -> Result<Self, &'static str> {
        artifact
            .validate()
            .map_err(|_| "production manifest received invalid Terminal artifact")?;
        Ok(Self::new(
            subject,
            ProductionArtifactIdentity::Terminal(artifact.manifest().identity()),
        ))
    }

    pub fn for_native(
        subject: ProductionCompilationSubject,
        artifact: &NativeArtifact,
    ) -> Result<Self, &'static str> {
        artifact
            .validate()
            .map_err(|_| "production manifest received invalid native artifact")?;
        if subject.native_target != artifact.target() {
            return Err("production manifest subject target disagrees with native artifact");
        }
        Ok(Self::new(
            subject,
            ProductionArtifactIdentity::Native(artifact.identity()),
        ))
    }

    fn new(subject: ProductionCompilationSubject, artifact: ProductionArtifactIdentity) -> Self {
        let canonical_bytes = canonical_manifest_bytes(&subject, artifact);
        let mut digest = Sha256::new();
        digest.update(&canonical_bytes);
        Self {
            subject,
            artifact,
            canonical_bytes,
            identity: ProductionCompilationManifestIdentity(digest.finalize().into()),
        }
    }

    pub const fn subject(&self) -> &ProductionCompilationSubject {
        &self.subject
    }

    pub const fn artifact(&self) -> ProductionArtifactIdentity {
        self.artifact
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> ProductionCompilationManifestIdentity {
        self.identity
    }

    pub fn validate(&self) -> bool {
        let canonical = canonical_manifest_bytes(&self.subject, self.artifact);
        if canonical != self.canonical_bytes {
            return false;
        }
        let mut digest = Sha256::new();
        digest.update(&canonical);
        self.identity == ProductionCompilationManifestIdentity(digest.finalize().into())
    }

    pub fn matches_terminal_artifact(&self, artifact: &CanonicalTerminalArtifact) -> bool {
        matches!(
            self.artifact,
            ProductionArtifactIdentity::Terminal(identity)
                if identity == artifact.manifest().identity()
        )
    }

    pub fn matches_native_artifact(&self, artifact: &NativeArtifact) -> bool {
        matches!(
            self.artifact,
            ProductionArtifactIdentity::Native(identity) if identity == artifact.identity()
        ) && self.subject.native_target == artifact.target()
    }

    /// Borrow the exact physical evidence already owned by this manifest's
    /// native artifact. No receipt, digest proxy, or blanket Terminal-complete
    /// bit is minted here.
    pub fn require_native_physical_evidence<'artifact>(
        &self,
        artifact: &'artifact NativeArtifact,
    ) -> Result<&'artifact NativePhysicalEvidence, FinalRealizationEvidenceError> {
        if !self.validate() {
            return Err(FinalRealizationEvidenceError::InvalidProductionManifest);
        }
        artifact
            .validate()
            .map_err(|_| FinalRealizationEvidenceError::InvalidNativeArtifact)?;
        if self.subject.native_target != artifact.target() {
            return Err(FinalRealizationEvidenceError::NativeTargetMismatch);
        }
        if !matches!(
            self.artifact,
            ProductionArtifactIdentity::Native(identity) if identity == artifact.identity()
        ) {
            return Err(FinalRealizationEvidenceError::NativeArtifactMismatch);
        }
        artifact
            .physical_evidence()
            .ok_or(FinalRealizationEvidenceError::NativePhysicalEvidenceUnavailable)
    }
}

fn canonical_manifest_bytes(
    subject: &ProductionCompilationSubject,
    artifact: ProductionArtifactIdentity,
) -> Vec<u8> {
    let mut bytes = MANIFEST_DOMAIN.to_vec();
    let package = &subject.package;
    append_field(&mut bytes, &package.root().digest());
    let closure = package.dependency_closure();
    bytes.push(match closure.root_role() {
        omega_package_compilation::BuildDeclarationKind::Package => 0,
        omega_package_compilation::BuildDeclarationKind::Application => 1,
        omega_package_compilation::BuildDeclarationKind::Workspace => {
            unreachable!("workspace roots cannot enter a package compilation subject")
        }
    });
    append_count(&mut bytes, closure.packages().len());
    for identity in closure.packages() {
        append_field(&mut bytes, &identity.digest());
    }
    append_count(&mut bytes, closure.dependencies().len());
    for dependency in closure.dependencies() {
        append_field(&mut bytes, &dependency.requester().digest());
        append_field(&mut bytes, dependency.alias().as_bytes());
        append_field(&mut bytes, &dependency.target().digest());
    }
    append_field(
        &mut bytes,
        &package.source_consumption_commitment().digest(),
    );
    let source_rows = package.canonical_consumed_unit_bytes();
    append_count(&mut bytes, source_rows.len());
    for row in source_rows {
        append_field(&mut bytes, &row);
    }
    append_field(
        &mut bytes,
        subject.selected_build_machine_identity.as_bytes(),
    );
    let usage = subject.build_evaluation_usage;
    bytes.extend_from_slice(&usage.usage_schema_version.to_le_bytes());
    bytes.extend_from_slice(&usage.step_schedule_marker.to_le_bytes());
    bytes.extend_from_slice(&usage.invocation_fuel_ceiling.to_le_bytes());
    match (
        usage.sponsor_schema_version,
        usage.session_fuel_ceiling,
        usage.session_build_log_byte_ceiling,
        usage.session_filesystem_attempt_ceiling,
        usage.session_live_filesystem_handle_ceiling,
        usage.session_live_cell_ceiling,
        usage.session_live_text_byte_ceiling,
        usage.session_result_cell_ceiling,
        usage.session_result_text_byte_ceiling,
    ) {
        (
            Some(schema),
            Some(fuel_ceiling),
            Some(build_log_ceiling),
            Some(filesystem_attempt_ceiling),
            Some(live_filesystem_handle_ceiling),
            Some(live_cell_ceiling),
            Some(live_text_byte_ceiling),
            Some(result_cell_ceiling),
            Some(result_text_byte_ceiling),
        ) => {
            bytes.push(1);
            bytes.extend_from_slice(&schema.to_le_bytes());
            bytes.extend_from_slice(&fuel_ceiling.to_le_bytes());
            bytes.extend_from_slice(&build_log_ceiling.to_le_bytes());
            bytes.extend_from_slice(&filesystem_attempt_ceiling.to_le_bytes());
            bytes.extend_from_slice(&live_filesystem_handle_ceiling.to_le_bytes());
            bytes.extend_from_slice(&live_cell_ceiling.to_le_bytes());
            bytes.extend_from_slice(&live_text_byte_ceiling.to_le_bytes());
            bytes.extend_from_slice(&result_cell_ceiling.to_le_bytes());
            bytes.extend_from_slice(&result_text_byte_ceiling.to_le_bytes());
        }
        (None, None, None, None, None, None, None, None, None) => bytes.push(0),
        _ => unreachable!("validated production subject has paired sponsor identity"),
    }
    bytes.extend_from_slice(&usage.fuel_units.to_le_bytes());
    bytes.extend_from_slice(&usage.replay_fuel_units.to_le_bytes());
    bytes.extend_from_slice(&usage.build_log_bytes.to_le_bytes());
    bytes.extend_from_slice(&usage.replay_build_log_bytes.to_le_bytes());
    bytes.extend_from_slice(&usage.filesystem_operation_attempts.to_le_bytes());
    bytes.extend_from_slice(&usage.replay_filesystem_operation_attempts.to_le_bytes());
    bytes.extend_from_slice(&usage.session_peak_live_filesystem_handles.to_le_bytes());
    bytes.extend_from_slice(&usage.session_peak_live_cells.to_le_bytes());
    bytes.extend_from_slice(&usage.peak_live_cells.to_le_bytes());
    bytes.extend_from_slice(&usage.replay_peak_live_cells.to_le_bytes());
    bytes.extend_from_slice(&usage.session_peak_live_text_bytes.to_le_bytes());
    bytes.extend_from_slice(&usage.peak_live_text_bytes.to_le_bytes());
    bytes.extend_from_slice(&usage.replay_peak_live_text_bytes.to_le_bytes());
    bytes.extend_from_slice(&usage.result_cells.to_le_bytes());
    bytes.extend_from_slice(&usage.replay_result_cells.to_le_bytes());
    bytes.extend_from_slice(&usage.result_text_bytes.to_le_bytes());
    bytes.extend_from_slice(&usage.replay_result_text_bytes.to_le_bytes());
    bytes.extend_from_slice(subject.build_observation_identity.as_bytes());
    bytes.push(target_profile_tag(subject.target_profile));
    append_native_target(&mut bytes, subject.native_target);
    match artifact {
        ProductionArtifactIdentity::Terminal(identity) => {
            bytes.push(1);
            bytes.extend_from_slice(identity.as_bytes());
        }
        ProductionArtifactIdentity::Native(identity) => {
            bytes.push(2);
            bytes.extend_from_slice(identity.as_bytes());
        }
    }
    bytes
}

fn append_count(bytes: &mut Vec<u8>, count: usize) {
    bytes.extend_from_slice(
        &u64::try_from(count)
            .expect("production manifest collection length fits u64")
            .to_le_bytes(),
    );
}

fn append_field(bytes: &mut Vec<u8>, field: &[u8]) {
    append_count(bytes, field.len());
    bytes.extend_from_slice(field);
}

fn target_profile_tag(profile: omega_target::TargetProfile) -> u8 {
    match profile {
        omega_target::TargetProfile::LinuxArm64 => 1,
        omega_target::TargetProfile::LinuxX64 => 2,
        omega_target::TargetProfile::MacosArm64 => 3,
        omega_target::TargetProfile::WindowsX64 => 4,
        omega_target::TargetProfile::UefiX64 => 5,
        omega_target::TargetProfile::CrossPlatformCli => 6,
        omega_target::TargetProfile::LocalUnchecked => 7,
    }
}

fn append_native_target(bytes: &mut Vec<u8>, target: omega_target::NativeTarget) {
    bytes.push(match target.architecture {
        omega_target::Architecture::Aarch64 => 1,
        omega_target::Architecture::X86_64 => 2,
    });
    bytes.push(match target.object_format {
        omega_target::ObjectFormat::Elf => 1,
        omega_target::ObjectFormat::MachO => 2,
        omega_target::ObjectFormat::Coff => 3,
    });
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_size)
            .expect("native pointer size fits u64")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(
        &u64::try_from(target.pointer_alignment)
            .expect("native pointer alignment fits u64")
            .to_le_bytes(),
    );
}
