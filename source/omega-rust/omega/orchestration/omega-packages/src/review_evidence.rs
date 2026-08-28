use crate::{CompilerIssuedPackageReview, ImmutableSourceResolution, PackageKey};
use omega_build_evaluation::{
    BuildFilesystemGrantAccess, BuildFilesystemGrantRefusalReason,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutputSource, BuildFilesystemOperationResult,
    BuildFilesystemProvider, BuildFilesystemRoot, BuildFilesystemScalarOperandValue,
    BuildObservationClass, BuildObservationSummary,
};
use omega_build_provenance::CompilerExecutableCommitment;
use omega_package_compilation::PackageSourceConsumptionCommitment;
use omega_package_review::{
    DecodedPackageReviewCanonicalRow, PackageReviewCanonicalRow, PackageReviewCanonicalRowKind,
    PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
};
use sha2::{Digest, Sha256};

const WHOLE_REVIEW_COMMITMENT_DOMAIN: &[u8] = b"OMEGA-PACKAGE-REVIEW-COMPARISON\0";
const BUILD_OBSERVATION_COMMITMENT_DOMAIN: &[u8] = b"OMEGA-PACKAGE-BUILD-OBSERVATION-COMPARISON\0";

/// Review-only identity of the exact compiler executable bytes observed while
/// evidence was produced. This does not certify the compiler or seal a package
/// instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlyCompilerExecutableCommitment([u8; 32]);

impl ReviewOnlyCompilerExecutableCommitment {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }

    pub(crate) const fn from_recovered_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }
}

impl From<CompilerExecutableCommitment> for ReviewOnlyCompilerExecutableCommitment {
    fn from(commitment: CompilerExecutableCommitment) -> Self {
        Self(commitment.digest())
    }
}

/// Review-only identity of the exact package/toolchain source bytes consumed
/// by one compiler run. It is provenance, not admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlySourceConsumptionCommitment([u8; 32]);

impl ReviewOnlySourceConsumptionCommitment {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }

    pub(crate) const fn from_recovered_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }
}

impl From<PackageSourceConsumptionCommitment> for ReviewOnlySourceConsumptionCommitment {
    fn from(commitment: PackageSourceConsumptionCommitment) -> Self {
        Self(commitment.digest())
    }
}

/// Opaque canonical comparison row used by package review orchestration.
///
/// Live rows are copied from an unforgeable compiler-issued review. Recovered
/// rows are constructed only by the compiler's strict recovery-frame decoder
/// and remain distinctly review-only; this type is never compiler evidence or
/// an admission artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOnlyCanonicalRow {
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    key_bytes: Vec<u8>,
    canonical_bytes: Vec<u8>,
    source: PackageReviewCanonicalRowSource,
    recovery_bytes: Option<Vec<u8>>,
}

impl ReviewOnlyCanonicalRow {
    pub const fn kind(&self) -> PackageReviewCanonicalRowKind {
        self.kind
    }

    pub const fn risk(&self) -> PackageReviewCanonicalRowRisk {
        self.risk
    }

    pub fn key_bytes(&self) -> &[u8] {
        &self.key_bytes
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn source(&self) -> &PackageReviewCanonicalRowSource {
        &self.source
    }

    pub(crate) fn from_compiler_issued(row: &PackageReviewCanonicalRow) -> Self {
        Self {
            kind: row.kind(),
            risk: row.risk(),
            key_bytes: row.key_bytes().to_vec(),
            canonical_bytes: row.canonical_bytes().to_vec(),
            source: row.source().clone(),
            recovery_bytes: None,
        }
    }

    pub(crate) fn from_recovered(
        row: &DecodedPackageReviewCanonicalRow,
        recovery_bytes: Vec<u8>,
    ) -> Self {
        Self {
            kind: row.kind(),
            risk: row.risk(),
            key_bytes: row.key_bytes().to_vec(),
            canonical_bytes: row.canonical_bytes().to_vec(),
            source: row.source().clone(),
            recovery_bytes: Some(recovery_bytes),
        }
    }

    pub(crate) fn recovery_bytes(&self) -> Option<&[u8]> {
        self.recovery_bytes.as_deref()
    }
}

/// The package-manager-facing evidence common to a live compiler review and a
/// restart-stable review-only baseline record.
///
/// This trait is deliberately private. Implementing it does not issue accepted
/// evidence or permit construction of a package instance.
pub(crate) trait PackageReviewEvidence {
    fn key(&self) -> &PackageKey;
    fn resolution(&self) -> &ImmutableSourceResolution;
    fn projection_identity_matches(&self) -> bool;
    fn target_name(&self) -> &str;
    fn compiler_executable_commitment(&self) -> ReviewOnlyCompilerExecutableCommitment;
    fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment;
    fn build_observation_commitment(&self) -> Option<[u8; 32]>;
    fn whole_review_commitment(&self) -> [u8; 32];
    fn canonical_rows(&self) -> &[ReviewOnlyCanonicalRow];
}

impl PackageReviewEvidence for CompilerIssuedPackageReview {
    fn key(&self) -> &PackageKey {
        CompilerIssuedPackageReview::key(self)
    }

    fn resolution(&self) -> &ImmutableSourceResolution {
        CompilerIssuedPackageReview::resolution(self)
    }

    fn projection_identity_matches(&self) -> bool {
        self.projection().package() == self.key().identity()
    }

    fn target_name(&self) -> &str {
        self.projection().target().target_name()
    }

    fn compiler_executable_commitment(&self) -> ReviewOnlyCompilerExecutableCommitment {
        CompilerIssuedPackageReview::compiler_executable_commitment(self).into()
    }

    fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment {
        CompilerIssuedPackageReview::source_consumption_commitment(self).into()
    }

    fn build_observation_commitment(&self) -> Option<[u8; 32]> {
        self.build_observation_summary()
            .map(build_observation_commitment)
    }

    fn whole_review_commitment(&self) -> [u8; 32] {
        whole_review_commitment(self.canonical_review_bytes())
    }

    fn canonical_rows(&self) -> &[ReviewOnlyCanonicalRow] {
        self.comparison_rows()
    }
}

pub(crate) fn whole_review_commitment(canonical_review_bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(WHOLE_REVIEW_COMMITMENT_DOMAIN);
    hash_bytes(&mut digest, canonical_review_bytes);
    digest.finalize().into()
}

pub(crate) fn build_observation_commitment(summary: &BuildObservationSummary) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(BUILD_OBSERVATION_COMMITMENT_DOMAIN);
    digest.update(summary.schema_version().to_le_bytes());
    digest.update([observation_class_tag(summary.ceiling())]);
    digest.update([observation_class_tag(summary.realized())]);
    digest.update(summary.filesystem_operation_schema_version().to_le_bytes());
    match summary.canonical_source_metadata_identity() {
        None => digest.update([0]),
        Some(identity) => {
            digest.update([1]);
            digest.update(identity.policy_version().to_le_bytes());
            digest.update(identity.source_content_commitment());
        }
    }
    digest.update([u8::from(summary.source_inputs_replay_verified())]);
    digest.update([u8::from(summary.operation_replay_verified())]);
    match summary.staged_output_tree() {
        None => digest.update([0]),
        Some(tree) => {
            digest.update([1]);
            digest.update(tree.digest());
            digest.update(tree.entry_count().to_le_bytes());
            digest.update(tree.file_bytes().to_le_bytes());
        }
    }
    digest.update(
        u64::try_from(summary.filesystem_operation_attempts().len())
            .expect("build observation attempt count fits u64")
            .to_le_bytes(),
    );
    for attempt in summary.filesystem_operation_attempts() {
        digest.update(attempt.operation_tag().to_le_bytes());
        digest.update([filesystem_provider_tag(attempt.provider())]);
        match attempt.result() {
            BuildFilesystemOperationResult::Scalar(value) => {
                digest.update([0]);
                digest.update(value.to_le_bytes());
            }
            BuildFilesystemOperationResult::LogicalHandle(identity) => {
                digest.update([1]);
                digest.update(identity.get().to_le_bytes());
            }
        }
        digest.update(attempt.post_error().to_le_bytes());
        digest.update(
            u64::try_from(attempt.scalar_operands().len())
                .expect("build observation scalar-operand count fits u64")
                .to_le_bytes(),
        );
        for operand in attempt.scalar_operands() {
            digest.update([operand.operand_ordinal()]);
            match operand.value() {
                BuildFilesystemScalarOperandValue::I32(value) => {
                    digest.update([0]);
                    digest.update(value.to_le_bytes());
                }
                BuildFilesystemScalarOperandValue::U32(value) => {
                    digest.update([1]);
                    digest.update(value.to_le_bytes());
                }
                BuildFilesystemScalarOperandValue::I64(value) => {
                    digest.update([2]);
                    digest.update(value.to_le_bytes());
                }
                BuildFilesystemScalarOperandValue::U64(value) => {
                    digest.update([3]);
                    digest.update(value.to_le_bytes());
                }
            }
        }
        digest.update(
            u64::try_from(attempt.byte_operands().len())
                .expect("build observation byte-operand count fits u64")
                .to_le_bytes(),
        );
        for operand in attempt.byte_operands() {
            digest.update([operand.operand_ordinal()]);
            hash_bytes(&mut digest, operand.bytes());
        }
        digest.update(
            u64::try_from(attempt.path_like_operands().len())
                .expect("build observation path-like-operand count fits u64")
                .to_le_bytes(),
        );
        for operand in attempt.path_like_operands() {
            digest.update([operand.operand_ordinal()]);
            hash_bytes(&mut digest, operand.bytes());
        }
        digest.update(
            u64::try_from(attempt.rooted_path_operand_resolutions().len())
                .expect("build observation rooted-path-resolution count fits u64")
                .to_le_bytes(),
        );
        for operand in attempt.rooted_path_operand_resolutions() {
            digest.update([operand.operand_ordinal()]);
            digest.update([match operand.root() {
                omega_build_evaluation::BuildFilesystemRoot::Source => 0,
                omega_build_evaluation::BuildFilesystemRoot::Output => 1,
            }]);
            hash_bytes(&mut digest, operand.relative_path());
        }
        digest.update(
            u64::try_from(attempt.returned_paths().len())
                .expect("build observation returned-path count fits u64")
                .to_le_bytes(),
        );
        for returned in attempt.returned_paths() {
            digest.update([returned.operand_ordinal()]);
            digest.update([match returned.kind() {
                omega_build_evaluation::BuildFilesystemReturnedPathKind::ReadLinkPayload => 0,
                omega_build_evaluation::BuildFilesystemReturnedPathKind::CanonicalPath => 1,
                omega_build_evaluation::BuildFilesystemReturnedPathKind::FinalPath => 2,
            }]);
            digest.update([match returned.completeness() {
                omega_build_evaluation::BuildFilesystemReturnedPathCompleteness::Complete => 0,
                omega_build_evaluation::BuildFilesystemReturnedPathCompleteness::LimitReached => 1,
            }]);
            hash_bytes(&mut digest, returned.bytes());
        }
        digest.update(
            u64::try_from(attempt.observed_byte_regions().len())
                .expect("build observation observed-byte-region count fits u64")
                .to_le_bytes(),
        );
        for region in attempt.observed_byte_regions() {
            digest.update([region.output_operand_ordinal()]);
            digest.update([match region.kind() {
                omega_build_evaluation::BuildFilesystemObservedByteRegionKind::SequentialFileRead => 0,
                omega_build_evaluation::BuildFilesystemObservedByteRegionKind::PositionedFileRead => 1,
                omega_build_evaluation::BuildFilesystemObservedByteRegionKind::DirectoryRecords => 2,
                omega_build_evaluation::BuildFilesystemObservedByteRegionKind::FindEntry => 3,
            }]);
            digest.update(region.offset().to_le_bytes());
            digest.update(region.length().to_le_bytes());
        }
        digest.update(
            u64::try_from(attempt.metadata_observations().len())
                .expect("build observation metadata count fits u64")
                .to_le_bytes(),
        );
        for metadata in attempt.metadata_observations() {
            digest.update([metadata.output_operand_ordinal()]);
            digest.update([match metadata.kind() {
                omega_build_evaluation::BuildFilesystemMetadataObservationKind::FollowedPath => 0,
                omega_build_evaluation::BuildFilesystemMetadataObservationKind::OpenDescriptor => 1,
                omega_build_evaluation::BuildFilesystemMetadataObservationKind::UnfollowedFinalPath => 2,
            }]);
            digest.update(metadata.device().to_le_bytes());
            digest.update(metadata.mode().to_le_bytes());
            digest.update(metadata.link_count().to_le_bytes());
            digest.update(metadata.inode().to_le_bytes());
            digest.update(metadata.user().to_le_bytes());
            digest.update(metadata.group().to_le_bytes());
            digest.update(metadata.referenced_device().to_le_bytes());
            digest.update(metadata.access_time().to_le_bytes());
            digest.update(metadata.modification_time().to_le_bytes());
            digest.update(metadata.change_time().to_le_bytes());
            digest.update(metadata.birth_time().to_le_bytes());
            digest.update(metadata.size().to_le_bytes());
            digest.update(metadata.blocks_512().to_le_bytes());
            digest.update(metadata.preferred_block_size().to_le_bytes());
        }
        digest.update(
            u64::try_from(attempt.mutable_byte_operand_resolutions().len())
                .expect("build observation mutable-byte-resolution count fits u64")
                .to_le_bytes(),
        );
        for operand in attempt.mutable_byte_operand_resolutions() {
            digest.update([operand.operand_ordinal()]);
            hash_bytes(&mut digest, operand.bytes());
        }
        digest.update(
            u64::try_from(attempt.mutable_i64_operand_resolutions().len())
                .expect("build observation mutable-i64-resolution count fits u64")
                .to_le_bytes(),
        );
        for operand in attempt.mutable_i64_operand_resolutions() {
            digest.update([operand.operand_ordinal()]);
            digest.update(operand.value().to_le_bytes());
        }
        digest.update(
            u64::try_from(attempt.mutable_byte_operands().len())
                .expect("build observation mutable-byte-operand count fits u64")
                .to_le_bytes(),
        );
        for operand in attempt.mutable_byte_operands() {
            digest.update([operand.operand_ordinal()]);
            hash_bytes(&mut digest, operand.pre_bytes());
            hash_bytes(&mut digest, operand.post_bytes());
        }
        digest.update(
            u64::try_from(attempt.mutable_i64_operands().len())
                .expect("build observation mutable-i64-operand count fits u64")
                .to_le_bytes(),
        );
        for operand in attempt.mutable_i64_operands() {
            digest.update([operand.operand_ordinal()]);
            digest.update(operand.pre_value().to_le_bytes());
            digest.update(operand.post_value().to_le_bytes());
        }
        digest.update(
            u64::try_from(attempt.authorized_paths().len())
                .expect("build observation authorized-path count fits u64")
                .to_le_bytes(),
        );
        for path in attempt.authorized_paths() {
            digest.update([path.operand_ordinal()]);
            digest.update([grant_access_tag(path.access())]);
            digest.update([filesystem_root_tag(path.root())]);
            hash_bytes(&mut digest, path.relative_path());
        }
        digest.update(
            u64::try_from(attempt.logical_handle_inputs().len())
                .expect("build observation logical-handle input count fits u64")
                .to_le_bytes(),
        );
        for input in attempt.logical_handle_inputs() {
            digest.update([input.operand_ordinal()]);
            digest.update([logical_handle_kind_tag(input.kind())]);
            match input.resolution() {
                BuildFilesystemLogicalHandleInputResolution::Resolved(identity) => {
                    digest.update([0]);
                    digest.update(identity.get().to_le_bytes());
                }
                BuildFilesystemLogicalHandleInputResolution::Null => digest.update([1]),
                BuildFilesystemLogicalHandleInputResolution::Unknown => digest.update([2]),
            }
        }
        match attempt.logical_handle_output() {
            None => digest.update([0]),
            Some(output) => {
                digest.update([1]);
                digest.update([logical_handle_kind_tag(output.kind())]);
                digest.update(output.identity().get().to_le_bytes());
                match output.source() {
                    BuildFilesystemLogicalHandleOutputSource::Created => digest.update([0]),
                    BuildFilesystemLogicalHandleOutputSource::Duplicated(identity) => {
                        digest.update([1]);
                        digest.update(identity.get().to_le_bytes());
                    }
                    BuildFilesystemLogicalHandleOutputSource::Borrowed(identity) => {
                        digest.update([2]);
                        digest.update(identity.get().to_le_bytes());
                    }
                }
            }
        }
        digest.update(
            u64::try_from(attempt.retired_logical_handles().len())
                .expect("build observation retired logical-handle count fits u64")
                .to_le_bytes(),
        );
        for identity in attempt.retired_logical_handles() {
            digest.update(identity.get().to_le_bytes());
        }
        digest.update(
            u64::try_from(attempt.grant_refusals().len())
                .expect("build observation refusal count fits u64")
                .to_le_bytes(),
        );
        for refusal in attempt.grant_refusals() {
            digest.update([refusal.operand_ordinal()]);
            digest.update([grant_access_tag(refusal.access())]);
            digest.update([grant_refusal_reason_tag(refusal.reason())]);
        }
    }
    digest.finalize().into()
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("review evidence byte length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
}

const fn observation_class_tag(class: BuildObservationClass) -> u8 {
    match class {
        BuildObservationClass::Hermetic => 0,
        BuildObservationClass::Receipted => 1,
        BuildObservationClass::Volatile => 2,
    }
}

const fn filesystem_provider_tag(provider: BuildFilesystemProvider) -> u8 {
    match provider {
        BuildFilesystemProvider::Virtual => 0,
        BuildFilesystemProvider::RealUnscoped => 1,
        BuildFilesystemProvider::RealScoped => 2,
    }
}

const fn grant_access_tag(access: BuildFilesystemGrantAccess) -> u8 {
    match access {
        BuildFilesystemGrantAccess::Read => 0,
        BuildFilesystemGrantAccess::Write => 1,
    }
}

const fn filesystem_root_tag(root: BuildFilesystemRoot) -> u8 {
    match root {
        BuildFilesystemRoot::Source => 0,
        BuildFilesystemRoot::Output => 1,
    }
}

const fn logical_handle_kind_tag(kind: BuildFilesystemLogicalHandleKind) -> u8 {
    match kind {
        BuildFilesystemLogicalHandleKind::Descriptor => 0,
        BuildFilesystemLogicalHandleKind::Native => 1,
        BuildFilesystemLogicalHandleKind::Find => 2,
    }
}

const fn grant_refusal_reason_tag(reason: BuildFilesystemGrantRefusalReason) -> u8 {
    match reason {
        BuildFilesystemGrantRefusalReason::Unresolvable => 0,
        BuildFilesystemGrantRefusalReason::OutsideGrantedRoots => 1,
        BuildFilesystemGrantRefusalReason::UnrepresentableRootedPath => 2,
        BuildFilesystemGrantRefusalReason::ObservationEvidenceLimitExceeded => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_compiler::compile_to_checked;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_OBSERVATION_FIXTURE: AtomicU64 = AtomicU64::new(1);

    fn compiled_observation(
        relative_output: &str,
        mode: i32,
        payload: &str,
    ) -> BuildObservationSummary {
        let sequence = NEXT_OBSERVATION_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let project = std::env::temp_dir().join(format!(
            "omega-review-observation-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&project);
        let build_dir = project.join("build");
        let output = build_dir.join(relative_output);
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        std::fs::write(
            project.join("build.omg"),
            format!(
                r#"use omega::language::std::filesystem_host;

target windows_x64 {{}}

data RootedWriter {{ filesystem: FilesystemHost; descriptor: i32; written: i64; result: i32; }}

machine RootedWriter::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{{
    let output: &[u8] in Path = builder.output.resolve("{relative_output}");
    self.descriptor = self.filesystem.create(output, {mode});
    self.written = self.filesystem.write(self.descriptor, "{payload}");
    self.result = self.filesystem.close(self.descriptor);
}}
"#,
                relative_output = relative_output,
                mode = mode,
                payload = payload,
            ),
        )
        .unwrap();
        std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n").unwrap();
        let summary = compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
            .unwrap()
            .build_observation_summary()
            .expect("filesystem build publishes observations")
            .clone();
        std::fs::remove_dir_all(project).unwrap();
        summary
    }

    fn compiled_handle_order_observation(reverse_close_order: bool) -> BuildObservationSummary {
        let sequence = NEXT_OBSERVATION_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let project = std::env::temp_dir().join(format!(
            "omega-review-handle-observation-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&project);
        std::fs::create_dir_all(&project).unwrap();
        let input = project.join("input.txt");
        std::fs::write(&input, "input\n").unwrap();
        let close_order = if reverse_close_order {
            "self.result = self.filesystem.close(self.second);\n    self.result = self.filesystem.close(self.first);"
        } else {
            "self.result = self.filesystem.close(self.first);\n    self.result = self.filesystem.close(self.second);"
        };
        std::fs::write(
            project.join("build.omg"),
            format!(
                r#"use omega::language::std::filesystem_host;

target windows_x64 {{}}

data HandleOrder {{ filesystem: FilesystemHost; first: i32; second: i32; result: i32; }}

machine HandleOrder::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{{
    let input: &[u8] in Path = builder.source.resolve("input.txt");
    self.first = self.filesystem.open(input, 0);
    self.second = self.filesystem.open(input, 0);
    {close_order}
}}
"#,
            ),
        )
        .unwrap();
        std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n").unwrap();
        let summary = compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
            .unwrap()
            .build_observation_summary()
            .expect("filesystem build publishes observations")
            .clone();
        std::fs::remove_dir_all(project).unwrap();
        summary
    }

    fn compiled_read_observation(input_bytes: &str) -> BuildObservationSummary {
        let sequence = NEXT_OBSERVATION_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let project = std::env::temp_dir().join(format!(
            "omega-review-read-observation-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&project);
        std::fs::create_dir_all(&project).unwrap();
        let input = project.join("input.txt");
        std::fs::write(&input, input_bytes).unwrap();
        std::fs::write(
            project.join("build.omg"),
            format!(
                r#"use omega::language::std::filesystem_host;

target windows_x64 {{}}

data RootedReader {{ filesystem: FilesystemHost; descriptor: i32; buffer: [u8; 6]; read: i64; result: i32; }}

machine RootedReader::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{{
    let input: &[u8] in Path = builder.source.resolve("input.txt");
    self.descriptor = self.filesystem.open(input, 0);
    self.read = self.filesystem.read(self.descriptor, &mut self.buffer, 6);
    self.result = self.filesystem.close(self.descriptor);
}}
"#,
            ),
        )
        .unwrap();
        std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n").unwrap();
        let summary = compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
            .unwrap()
            .build_observation_summary()
            .expect("filesystem build publishes observations")
            .clone();
        std::fs::remove_dir_all(project).unwrap();
        summary
    }

    fn compiled_path_like_observation(target: &str) -> BuildObservationSummary {
        let sequence = NEXT_OBSERVATION_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let project = std::env::temp_dir().join(format!(
            "omega-review-path-like-observation-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&project);
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(
            project.join("build.omg"),
            format!(
                r#"use omega::language::std::filesystem_host;

target windows_x64 {{}}

data SymlinkProbe {{ filesystem: FilesystemHost; result: i32; }}

machine SymlinkProbe::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{{
    let link: &[u8] in Path = builder.output.resolve("missing-parent/link");
    self.result = self.filesystem.symlink("{target}", link);
}}
"#,
            ),
        )
        .unwrap();
        std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n").unwrap();
        let summary = compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
            .unwrap()
            .build_observation_summary()
            .expect("filesystem build publishes observations")
            .clone();
        std::fs::remove_dir_all(project).unwrap();
        summary
    }

    #[cfg(unix)]
    fn compiled_read_link_observation(target: &str) -> BuildObservationSummary {
        use std::os::unix::fs::symlink;

        let sequence = NEXT_OBSERVATION_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let project = std::env::temp_dir().join(format!(
            "omega-review-read-link-observation-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&project);
        std::fs::create_dir_all(&project).unwrap();
        symlink(target, project.join("fixture-link")).unwrap();
        std::fs::write(
            project.join("build.omg"),
            r#"use omega::language::std::filesystem_host;

target windows_x64 {}

data ReadLinkProbe { filesystem: FilesystemHost; buffer: [u8; 32]; result: i64; }

machine ReadLinkProbe::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{
    let link: &[u8] in Path = builder.source.resolve("fixture-link");
    self.result = self.filesystem.read_link(link, &mut self.buffer, 32);
}
"#,
        )
        .unwrap();
        std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n").unwrap();
        let summary = compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
            .unwrap()
            .build_observation_summary()
            .expect("read_link build publishes observations")
            .clone();
        std::fs::remove_dir_all(project).unwrap();
        summary
    }

    fn compiled_metadata_observation(main_source: &str) -> BuildObservationSummary {
        let sequence = NEXT_OBSERVATION_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let project = std::env::temp_dir().join(format!(
            "omega-review-metadata-observation-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&project);
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(
            project.join("build.omg"),
            r#"use omega::language::std::filesystem_host;

target windows_x64 {}

data MetadataProbe { filesystem: FilesystemHost; buffer: [u8; 144]; result: i32; }

machine MetadataProbe::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{
    let input: &[u8] in Path = builder.source.resolve("main.omg");
    self.result = self.filesystem.read_metadata(input, &mut self.buffer);
}
"#,
        )
        .unwrap();
        std::fs::write(project.join("main.omg"), main_source).unwrap();
        let summary = compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
            .unwrap()
            .build_observation_summary()
            .expect("metadata build publishes observations")
            .clone();
        std::fs::remove_dir_all(project).unwrap();
        summary
    }

    #[test]
    fn rooted_observation_commitment_is_relocation_stable_and_path_sensitive() {
        let first = compiled_observation("stage/artifact.bin", 438, "payload-a");
        let relocated = compiled_observation("stage/artifact.bin", 438, "payload-a");
        assert_eq!(first, relocated);
        assert_eq!(
            build_observation_commitment(&first),
            build_observation_commitment(&relocated)
        );

        let changed = compiled_observation("stage/changed.bin", 438, "payload-a");
        assert_ne!(first, changed);
        assert_ne!(
            build_observation_commitment(&first),
            build_observation_commitment(&changed)
        );
        let scalar_changed = compiled_observation("stage/artifact.bin", 420, "payload-a");
        assert_ne!(
            build_observation_commitment(&first),
            build_observation_commitment(&scalar_changed),
            "one changed scalar operand changes observation identity"
        );
        let bytes_changed = compiled_observation("stage/artifact.bin", 438, "payload-b");
        assert_ne!(
            build_observation_commitment(&first),
            build_observation_commitment(&bytes_changed),
            "one changed immutable byte operand changes observation identity"
        );
        assert_eq!(first.schema_version(), 24);
        assert_eq!(first.filesystem_operation_schema_version(), 18);
        assert!(first.staged_output_tree().is_none());
        assert!(relocated.staged_output_tree().is_none());
        assert!(bytes_changed.staged_output_tree().is_none());
        let [create, write, close] = first.filesystem_operation_attempts() else {
            panic!("fixture performs create, write, and close")
        };
        let [path] = create.authorized_paths() else {
            panic!("create retains one rooted output path")
        };
        let [resolution] = create.rooted_path_operand_resolutions() else {
            panic!("create retains one rooted input resolution")
        };
        assert_eq!(resolution.operand_ordinal(), 0);
        assert_eq!(resolution.root(), BuildFilesystemRoot::Output);
        assert_eq!(resolution.relative_path(), b"stage/artifact.bin");
        assert_eq!(path.root(), BuildFilesystemRoot::Output);
        assert_eq!(path.relative_path(), b"stage/artifact.bin");
        let created_identity = create
            .logical_handle_output()
            .expect("successful create retains its logical descriptor")
            .identity();
        assert_eq!(
            create.result(),
            BuildFilesystemOperationResult::LogicalHandle(created_identity)
        );
        assert_eq!(write.result(), BuildFilesystemOperationResult::Scalar(9));
        assert_eq!(
            create.scalar_operands()[0].value(),
            BuildFilesystemScalarOperandValue::I32(438)
        );
        assert_eq!(write.byte_operands()[0].bytes(), b"payload-a");
        assert!(close.authorized_paths().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn observation_commitment_binds_exact_returned_path_bytes() {
        let alpha = compiled_read_link_observation("alpha");
        let bravo = compiled_read_link_observation("bravo");
        let [alpha_attempt] = alpha.filesystem_operation_attempts() else {
            panic!("fixture performs one read_link")
        };
        let [bravo_attempt] = bravo.filesystem_operation_attempts() else {
            panic!("fixture performs one read_link")
        };
        assert_eq!(alpha_attempt.returned_paths()[0].bytes(), b"alpha");
        assert_eq!(bravo_attempt.returned_paths()[0].bytes(), b"bravo");
        assert_ne!(
            build_observation_commitment(&alpha),
            build_observation_commitment(&bravo),
            "changed returned-path bytes change observation identity"
        );
    }

    #[test]
    fn observation_commitment_binds_logical_handle_lifetimes() {
        let forward = compiled_handle_order_observation(false);
        let reverse = compiled_handle_order_observation(true);
        let without_handles = |summary: &BuildObservationSummary| {
            summary
                .filesystem_operation_attempts()
                .iter()
                .map(|attempt| {
                    (
                        attempt.operation_tag(),
                        attempt.provider(),
                        attempt.result(),
                        attempt.post_error(),
                        attempt.authorized_paths().to_vec(),
                        attempt.grant_refusals().to_vec(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            without_handles(&forward),
            without_handles(&reverse),
            "the fixture pair differs only in logical descriptor use"
        );
        assert_ne!(forward, reverse);
        assert_ne!(
            build_observation_commitment(&forward),
            build_observation_commitment(&reverse),
            "package review must bind which live descriptor each close consumed"
        );

        let [_, _, forward_first_close, forward_second_close] =
            forward.filesystem_operation_attempts()
        else {
            panic!("forward fixture performs two opens and two closes")
        };
        let [first_input] = forward_first_close.logical_handle_inputs() else {
            panic!("first close retains one logical descriptor")
        };
        let [second_input] = forward_second_close.logical_handle_inputs() else {
            panic!("second close retains one logical descriptor")
        };
        assert_eq!(
            first_input.resolution(),
            BuildFilesystemLogicalHandleInputResolution::Resolved(
                forward_first_close.retired_logical_handles()[0]
            )
        );
        assert_eq!(
            second_input.resolution(),
            BuildFilesystemLogicalHandleInputResolution::Resolved(
                forward_second_close.retired_logical_handles()[0]
            )
        );
        assert_ne!(first_input.resolution(), second_input.resolution());
    }

    #[test]
    fn observation_commitment_binds_mutable_pre_and_post_state() {
        let first = compiled_read_observation("alpha\n");
        let changed = compiled_read_observation("bravo\n");
        let first_read = &first.filesystem_operation_attempts()[1];
        let changed_read = &changed.filesystem_operation_attempts()[1];
        assert_eq!(
            first_read.mutable_byte_operand_resolutions()[0].bytes(),
            &[0; 6]
        );
        assert_eq!(
            changed_read.mutable_byte_operand_resolutions()[0].bytes(),
            &[0; 6]
        );
        assert_eq!(first_read.mutable_byte_operands()[0].pre_bytes(), &[0; 6]);
        assert_eq!(changed_read.mutable_byte_operands()[0].pre_bytes(), &[0; 6]);
        assert_eq!(
            first_read.mutable_byte_operands()[0].post_bytes(),
            b"alpha\n"
        );
        assert_eq!(
            changed_read.mutable_byte_operands()[0].post_bytes(),
            b"bravo\n"
        );
        let [first_region] = first_read.observed_byte_regions() else {
            panic!("successful read retains one semantic observed-byte region")
        };
        let [changed_region] = changed_read.observed_byte_regions() else {
            panic!("changed read retains one semantic observed-byte region")
        };
        assert_eq!(first_region, changed_region);
        assert_eq!(
            first_read.observed_bytes(first_region),
            Some(b"alpha\n".as_slice())
        );
        assert_eq!(
            changed_read.observed_bytes(changed_region),
            Some(b"bravo\n".as_slice())
        );
        assert_ne!(
            build_observation_commitment(&first),
            build_observation_commitment(&changed),
            "one changed observed file-content region changes observation identity"
        );
    }

    #[test]
    fn observation_commitment_binds_canonical_metadata() {
        let short = compiled_metadata_observation("const VALUE: u8 = 1;\n");
        let long = compiled_metadata_observation("const VALUE: u64 = 123456789;\n");
        let [short_attempt] = short.filesystem_operation_attempts() else {
            panic!("metadata fixture performs one stat")
        };
        let [long_attempt] = long.filesystem_operation_attempts() else {
            panic!("metadata fixture performs one stat")
        };
        let [short_metadata] = short_attempt.metadata_observations() else {
            panic!("successful short stat retains canonical metadata")
        };
        let [long_metadata] = long_attempt.metadata_observations() else {
            panic!("successful long stat retains canonical metadata")
        };
        assert_ne!(short_metadata.size(), long_metadata.size());
        assert_ne!(
            build_observation_commitment(&short),
            build_observation_commitment(&long),
            "changed canonical metadata changes package observation identity"
        );
    }

    #[test]
    fn observation_commitment_binds_path_like_operands() {
        let first = compiled_path_like_observation("missing-alpha");
        let changed = compiled_path_like_observation("missing-bravo");
        let [first_symlink] = first.filesystem_operation_attempts() else {
            panic!("fixture performs one symlink operation")
        };
        let [changed_symlink] = changed.filesystem_operation_attempts() else {
            panic!("fixture performs one symlink operation")
        };
        assert_eq!(first_symlink.result(), changed_symlink.result());
        assert_eq!(first_symlink.post_error(), changed_symlink.post_error());
        assert_eq!(
            first_symlink.authorized_paths(),
            changed_symlink.authorized_paths()
        );
        assert_eq!(
            first_symlink.grant_refusals(),
            changed_symlink.grant_refusals()
        );
        assert_eq!(
            first_symlink.path_like_operands()[0].bytes(),
            b"missing-alpha"
        );
        assert_eq!(
            changed_symlink.path_like_operands()[0].bytes(),
            b"missing-bravo"
        );
        assert_ne!(
            build_observation_commitment(&first),
            build_observation_commitment(&changed),
            "one changed path-like operand changes observation identity"
        );
    }
}
