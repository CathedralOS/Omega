//! Validated package/source/build/target custody captured before artifact production.
//!
//! The compiler supplies checked observations; this owner validates accounting
//! and defines their canonical field order. Artifact kinds and publication
//! manifests remain consumer-owned and append their own versioned envelope.

use crate::PackageCompilationSubject;
use crate::{BuildEvaluationUsage, BuildObservationIdentity, BuildObservationSummary};

/// Exact artifact-free compilation custody captured before checked state is
/// consumed by Terminal/native production.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionCompilationSubject {
    package: PackageCompilationSubject,
    selected_build_machine_identity: String,
    build_evaluation_usage: BuildEvaluationUsage,
    build_observation_identity: BuildObservationIdentity,
    target_profile: target::TargetProfile,
    native_target: target::NativeTarget,
}

impl ProductionCompilationSubject {
    #[doc(hidden)]
    pub fn from_checked(
        package: PackageCompilationSubject,
        selected_build_machine_identity: String,
        build_evaluation_usage: BuildEvaluationUsage,
        build_observation: &BuildObservationSummary,
        target_profile: target::TargetProfile,
        native_target: target::NativeTarget,
    ) -> Result<Self, &'static str> {
        if selected_build_machine_identity.is_empty() {
            return Err("production compilation subject has an empty build-machine identity");
        }
        if target_profile.native_target() != native_target {
            return Err(
                "production compilation subject target profile disagrees with native target",
            );
        }
        build_evaluation_usage
            .validate()
            .map_err(build_usage_diagnostic)?;
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

    pub const fn target_profile(&self) -> target::TargetProfile {
        self.target_profile
    }

    pub const fn native_target(&self) -> target::NativeTarget {
        self.native_target
    }

    /// Append the canonical subject fields, without a manifest domain or artifact.
    ///
    /// This layout is the subject portion of production-manifest V11. Keep its
    /// field order stable; consumers own versioning the surrounding envelope.
    pub fn append_canonical_bytes(&self, bytes: &mut Vec<u8>) {
        let package = &self.package;
        append_field(bytes, &package.root().digest());
        let closure = package.dependency_closure();
        bytes.push(match closure.root_role() {
            crate::BuildDeclarationKind::Package => 0,
            crate::BuildDeclarationKind::Application => 1,
            crate::BuildDeclarationKind::Workspace => {
                unreachable!("workspace roots cannot enter a package compilation subject")
            }
        });
        append_count(bytes, closure.packages().len());
        for identity in closure.packages() {
            append_field(bytes, &identity.digest());
        }
        append_count(bytes, closure.dependencies().len());
        for dependency in closure.dependencies() {
            append_field(bytes, &dependency.requester().digest());
            append_field(bytes, dependency.alias().as_bytes());
            append_field(bytes, &dependency.target().digest());
        }
        append_field(bytes, &package.source_consumption_commitment().digest());
        package.append_canonical_consumed_units(bytes);
        append_field(bytes, self.selected_build_machine_identity.as_bytes());
        let usage = self.build_evaluation_usage;
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
        bytes.extend_from_slice(&usage.build_log_bytes.to_le_bytes());
        bytes.extend_from_slice(&usage.filesystem_operation_attempts.to_le_bytes());
        bytes.extend_from_slice(&usage.session_peak_live_filesystem_handles.to_le_bytes());
        bytes.extend_from_slice(&usage.session_peak_live_cells.to_le_bytes());
        bytes.extend_from_slice(&usage.peak_live_cells.to_le_bytes());
        bytes.extend_from_slice(&usage.session_peak_live_text_bytes.to_le_bytes());
        bytes.extend_from_slice(&usage.peak_live_text_bytes.to_le_bytes());
        bytes.extend_from_slice(&usage.result_cells.to_le_bytes());
        bytes.extend_from_slice(&usage.result_text_bytes.to_le_bytes());
        bytes.extend_from_slice(self.build_observation_identity.as_bytes());
        bytes.push(target_profile_tag(self.target_profile));
        append_native_target(bytes, self.native_target);
    }
}

// The interpreter owns accounting consistency; this subject keeps its existing
// production-specific diagnostic contract.
fn build_usage_diagnostic(error: checked_interpreter::BuildEvaluationUsageError) -> &'static str {
    use checked_interpreter::BuildEvaluationUsageError;
    match error {
        BuildEvaluationUsageError::ZeroInvocationFuelCeiling => {
            "production compilation subject has a zero invocation fuel ceiling"
        }
        BuildEvaluationUsageError::InvocationFuelExceeded => {
            "production compilation subject exceeded its invocation fuel ceiling"
        }
        BuildEvaluationUsageError::UnsponsoredSessionUsage => {
            "production compilation subject has unsponsored session-live usage"
        }
        BuildEvaluationUsageError::ZeroSessionFuelCeiling => {
            "production compilation subject has a zero session fuel ceiling"
        }
        BuildEvaluationUsageError::ZeroSessionBuildLogCeiling => {
            "production compilation subject has a zero session BuildLog ceiling"
        }
        BuildEvaluationUsageError::ZeroSessionFilesystemAttemptCeiling => {
            "production compilation subject has a zero session filesystem-attempt ceiling"
        }
        BuildEvaluationUsageError::ZeroLiveFilesystemHandleCeiling => {
            "production compilation subject has a zero live-filesystem-handle ceiling"
        }
        BuildEvaluationUsageError::ZeroLiveCellCeiling => {
            "production compilation subject has a zero live-cell ceiling"
        }
        BuildEvaluationUsageError::ZeroLiveTextByteCeiling => {
            "production compilation subject has a zero live-Text-byte ceiling"
        }
        BuildEvaluationUsageError::ZeroResultCellCeiling => {
            "production compilation subject has a zero result-cell ceiling"
        }
        BuildEvaluationUsageError::ZeroResultTextByteCeiling => {
            "production compilation subject has a zero result-Text-byte ceiling"
        }
        BuildEvaluationUsageError::SessionFuelExceeded => {
            "production compilation subject exceeded its session fuel ceiling"
        }
        BuildEvaluationUsageError::SessionBuildLogExceeded => {
            "production compilation subject exceeded its session BuildLog ceiling"
        }
        BuildEvaluationUsageError::SessionFilesystemAttemptsExceeded => {
            "production compilation subject exceeded its session filesystem-attempt ceiling"
        }
        BuildEvaluationUsageError::LiveFilesystemHandlesExceeded => {
            "production compilation subject exceeded its live-filesystem-handle ceiling"
        }
        BuildEvaluationUsageError::LiveCellsExceeded => {
            "production compilation subject exceeded its live-cell ceiling"
        }
        BuildEvaluationUsageError::InvocationCellPeakExceedsSession => {
            "production compilation subject live-cell peak exceeds its session peak"
        }
        BuildEvaluationUsageError::LiveTextBytesExceeded => {
            "production compilation subject exceeded its live-Text-byte ceiling"
        }
        BuildEvaluationUsageError::InvocationTextPeakExceedsSession => {
            "production compilation subject live-Text-byte peak exceeds its session peak"
        }
        BuildEvaluationUsageError::ResultCellsExceeded => {
            "production compilation subject exceeded its result-cell ceiling"
        }
        BuildEvaluationUsageError::ResultTextBytesExceeded => {
            "production compilation subject exceeded its result-Text-byte ceiling"
        }
        BuildEvaluationUsageError::IncompleteSponsorIdentity => {
            "production compilation subject has incomplete evaluation sponsor identity"
        }
    }
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

pub(crate) fn target_profile_tag(profile: target::TargetProfile) -> u8 {
    match profile {
        target::TargetProfile::LinuxArm64 => 1,
        target::TargetProfile::LinuxX64 => 2,
        target::TargetProfile::MacosArm64 => 3,
        target::TargetProfile::WindowsX64 => 4,
        target::TargetProfile::UefiX64 => 5,
        target::TargetProfile::CrossPlatformCli => 6,
        target::TargetProfile::LocalUnchecked => 7,
        target::TargetProfile::AlphaBootstrap => 8,
        target::TargetProfile::MacosX64 => 9,
    }
}

fn append_native_target(bytes: &mut Vec<u8>, target: target::NativeTarget) {
    bytes.push(match target.architecture {
        target::Architecture::Aarch64 => 1,
        target::Architecture::X86_64 => 2,
    });
    bytes.push(match target.object_format {
        target::ObjectFormat::Elf => 1,
        target::ObjectFormat::MachO => 2,
        target::ObjectFormat::Coff => 3,
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

/// Recover a target profile from its canonical evidence tag.
pub(crate) fn recover_target_profile_tag(tag: u8) -> Option<target::TargetProfile> {
    use target::TargetProfile::*;
    match tag {
        1 => Some(LinuxArm64),
        2 => Some(LinuxX64),
        3 => Some(MacosArm64),
        4 => Some(WindowsX64),
        5 => Some(UefiX64),
        6 => Some(CrossPlatformCli),
        7 => Some(LocalUnchecked),
        8 => Some(AlphaBootstrap),
        9 => Some(MacosX64),
        _ => None,
    }
}
