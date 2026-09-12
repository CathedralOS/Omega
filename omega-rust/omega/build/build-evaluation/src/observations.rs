//! Durable build observation records and evaluator evidence projections.

use build_output::BuildStagedOutputTree;

/// Accounting-only projection of the transitional typed-tree build evaluator.
/// This is not terminal-Psi fuel and does not participate in `BuildConfig` or
/// program identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildEvaluationUsage {
    pub usage_schema_version: u32,
    pub step_schedule_marker: u32,
    /// Deterministic per-invocation ceiling applied independently to the
    /// initial evaluation and an exact replay. This is not a CPU limit.
    pub invocation_fuel_ceiling: u64,
    /// Shared sponsor-limit schema for package review. Standalone compilation
    /// has no aggregate sponsor.
    pub sponsor_schema_version: Option<u32>,
    /// Aggregate deterministic fuel available to the complete sponsored
    /// review session. Dependencies cannot alter this value.
    pub session_fuel_ceiling: Option<u64>,
    /// Aggregate compiler-owned BuildLog bytes available to the complete
    /// sponsored review session.
    pub session_build_log_byte_ceiling: Option<u64>,
    /// Aggregate canonical filesystem operation attempts available to the
    /// complete sponsored review session.
    pub session_filesystem_attempt_ceiling: Option<u64>,
    /// Compiler-owned filesystem resources that may be reserved concurrently
    /// across the sponsored review session.
    pub session_live_filesystem_handle_ceiling: Option<u64>,
    /// Semantic interpreter cells that may be live concurrently across the
    /// sponsored review session. This is not a memory-byte ceiling.
    pub session_live_cell_ceiling: Option<u64>,
    /// Logical bytes in concurrently live interpreter Text backing buffers.
    /// This does not represent Vec capacity or process memory.
    pub session_live_text_byte_ceiling: Option<u64>,
    /// Aggregate recursive result cells admitted across the sponsored session.
    pub session_result_cell_ceiling: Option<u64>,
    /// Aggregate Text payload bytes admitted across the sponsored session.
    pub session_result_text_byte_ceiling: Option<u64>,
    /// Highest concurrent reservation count observed in the shared session at
    /// the point this build result was issued.
    pub session_peak_live_filesystem_handles: u64,
    /// Highest concurrent semantic-cell reservation count observed in the
    /// shared session when this result was issued.
    pub session_peak_live_cells: u64,
    /// Highest live interpreter Text payload-byte count observed in the shared
    /// sponsored session when this result was issued.
    pub session_peak_live_text_bytes: u64,
    /// Fuel consumed by the initial build-machine evaluation.
    pub fuel_units: u64,
    /// Fuel consumed by exact provider-free replay, or zero when no replay ran.
    pub replay_fuel_units: u64,
    /// BuildLog bytes emitted by initial evaluation.
    pub build_log_bytes: u64,
    /// BuildLog bytes emitted by exact replay, or zero when no replay ran.
    pub replay_build_log_bytes: u64,
    /// Filesystem operation attempts retained by initial evaluation.
    pub filesystem_operation_attempts: u64,
    /// Filesystem operation attempts retained by replay, or zero when absent.
    pub replay_filesystem_operation_attempts: u64,
    /// Maximum semantic interpreter cells live concurrently in the initial
    /// evaluation.
    pub peak_live_cells: u64,
    /// Maximum semantic interpreter cells live concurrently in replay, or zero
    /// when no replay ran.
    pub replay_peak_live_cells: u64,
    /// Maximum logical bytes in live Text backing buffers during initial
    /// evaluation and exact replay.
    pub peak_live_text_bytes: u64,
    pub replay_peak_live_text_bytes: u64,
    pub result_cells: u64,
    pub replay_result_cells: u64,
    pub result_text_bytes: u64,
    pub replay_result_text_bytes: u64,
}

impl BuildEvaluationUsage {
    /// Compare the deterministic accounting owned by one build-machine
    /// invocation and its exact replay, excluding aggregate sponsor/session
    /// context.
    ///
    /// A package review may run inside a shared sponsored session while the
    /// later production compile runs independently. Session ceilings and
    /// session-wide peaks therefore identify the review orchestration, not the
    /// individual build whose source and observation are being rejoined.
    pub const fn has_same_invocation_usage(self, other: Self) -> bool {
        self.usage_schema_version == other.usage_schema_version
            && self.step_schedule_marker == other.step_schedule_marker
            && self.invocation_fuel_ceiling == other.invocation_fuel_ceiling
            && self.fuel_units == other.fuel_units
            && self.replay_fuel_units == other.replay_fuel_units
            && self.build_log_bytes == other.build_log_bytes
            && self.replay_build_log_bytes == other.replay_build_log_bytes
            && self.filesystem_operation_attempts == other.filesystem_operation_attempts
            && self.replay_filesystem_operation_attempts
                == other.replay_filesystem_operation_attempts
            && self.peak_live_cells == other.peak_live_cells
            && self.replay_peak_live_cells == other.replay_peak_live_cells
            && self.peak_live_text_bytes == other.peak_live_text_bytes
            && self.replay_peak_live_text_bytes == other.replay_peak_live_text_bytes
            && self.result_cells == other.result_cells
            && self.replay_result_cells == other.replay_result_cells
            && self.result_text_bytes == other.result_text_bytes
            && self.replay_result_text_bytes == other.replay_result_text_bytes
    }
}

pub const BUILD_OBSERVATION_SCHEMA_VERSION: u32 = 75;
pub const BUILD_FILESYSTEM_REPLAY_VERDICT_SCHEMA_VERSION: u32 = 1;

/// Normalized build-host observation class for one selected build machine.
///
/// The static ceiling remains conservative. A realized run becomes receipted
/// only when a bounded no-host replay reproduces its complete admitted Output
/// mutation and that tree matches independent sponsored custody.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BuildObservationClass {
    Hermetic,
    Receipted,
    Volatile,
}

/// Exact replay disposition for one exercised filesystem operation.
///
/// An exercised operation cannot be `Hermetic`: it was either reproduced by
/// the compiler-owned replay provider or it remains a volatile host
/// observation. This row does not claim containment of the host provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemOperationObservationClass {
    Receipted,
    Volatile,
}

/// Compiler-issued observation facts for one completed build-machine run.
///
/// This is execution evidence, not capability/API comparison identity. A
/// volatile row carries no replay receipt and makes no rebuildability claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemProvider {
    /// Deterministic in-memory provider.
    Virtual,
    /// Real process filesystem without path grants; never selected by admitted
    /// build execution.
    RealUnscoped,
    /// Real filesystem constrained by compiler-supplied path grants.
    RealScoped,
}

/// Exact immutable-source coordinate governing canonical build-visible
/// metadata. This is compiler sponsorship, not package admission evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildCanonicalSourceMetadataIdentity {
    pub(super) policy_version: u32,
    pub(super) source_content_commitment: [u8; 32],
}

impl BuildCanonicalSourceMetadataIdentity {
    #[doc(hidden)]
    pub const fn new(policy_version: u32, source_content_commitment: [u8; 32]) -> Self {
        Self {
            policy_version,
            source_content_commitment,
        }
    }

    pub const fn policy_version(self) -> u32 {
        self.policy_version
    }

    pub const fn source_content_commitment(self) -> [u8; 32] {
        self.source_content_commitment
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemGrantAccess {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemGrantRefusalReason {
    Unresolvable,
    OutsideGrantedRoots,
    UnrepresentableRootedPath,
    ObservationEvidenceLimitExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildFilesystemGrantRefusal {
    pub(super) operand_ordinal: u8,
    pub(super) access: BuildFilesystemGrantAccess,
    pub(super) reason: BuildFilesystemGrantRefusalReason,
}

impl BuildFilesystemGrantRefusal {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn access(self) -> BuildFilesystemGrantAccess {
        self.access
    }

    pub const fn reason(self) -> BuildFilesystemGrantRefusalReason {
        self.reason
    }
}

/// Stable build-evaluation identity for a package filesystem root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemRoot {
    Source,
    Output,
}

/// One path operand or descriptor-derived path that passed the scoped grant
/// gate. The path is canonical and relative to `root`; it contains no host
/// absolute prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFilesystemAuthorizedPath {
    pub(super) operand_ordinal: u8,
    pub(super) access: BuildFilesystemGrantAccess,
    pub(super) root: BuildFilesystemRoot,
    pub(super) relative_path: Vec<u8>,
}

impl BuildFilesystemAuthorizedPath {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub const fn access(&self) -> BuildFilesystemGrantAccess {
        self.access
    }

    pub const fn root(&self) -> BuildFilesystemRoot {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemScalarOperandValue {
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildFilesystemScalarOperand {
    pub(super) operand_ordinal: u8,
    pub(super) value: BuildFilesystemScalarOperandValue,
}

impl BuildFilesystemScalarOperand {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn value(self) -> BuildFilesystemScalarOperandValue {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFilesystemByteOperand {
    pub(super) operand_ordinal: u8,
    pub(super) bytes: Vec<u8>,
}

impl BuildFilesystemByteOperand {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Exact path-like bytes consumed by an operation but not interpreted as one
/// rooted grant path. This includes directory-entry names, search patterns,
/// and symlink target spellings; keeping it distinct from payload bytes and
/// authorized paths preserves the operation's argument semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFilesystemPathLikeOperand {
    pub(super) operand_ordinal: u8,
    pub(super) bytes: Vec<u8>,
}

impl BuildFilesystemPathLikeOperand {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Portable compiler-rooted path retained when its authored operand resolves,
/// before lowering to provider-specific path bytes. This does not claim that
/// the later grant check authorized the same rooted location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFilesystemRootedPathOperandResolution {
    pub(super) operand_ordinal: u8,
    pub(super) root: BuildFilesystemRoot,
    pub(super) relative_path: Vec<u8>,
}

impl BuildFilesystemRootedPathOperandResolution {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub const fn root(&self) -> BuildFilesystemRoot {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemReturnedPathKind {
    ReadLinkPayload,
    CanonicalPath,
    FinalPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemReturnedPathCompleteness {
    Complete,
    LimitReached,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFilesystemReturnedPath {
    pub(super) operand_ordinal: u8,
    pub(super) kind: BuildFilesystemReturnedPathKind,
    pub(super) completeness: BuildFilesystemReturnedPathCompleteness,
    pub(super) bytes: Vec<u8>,
}

impl BuildFilesystemReturnedPath {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub const fn kind(&self) -> BuildFilesystemReturnedPathKind {
        self.kind
    }

    pub const fn completeness(&self) -> BuildFilesystemReturnedPathCompleteness {
        self.completeness
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemObservedByteRegionKind {
    SequentialFileRead,
    PositionedFileRead,
    DirectoryRecords,
    FindEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildFilesystemObservedByteRegion {
    pub(super) output_operand_ordinal: u8,
    pub(super) kind: BuildFilesystemObservedByteRegionKind,
    pub(super) offset: u64,
    pub(super) length: u64,
}

impl BuildFilesystemObservedByteRegion {
    pub const fn output_operand_ordinal(self) -> u8 {
        self.output_operand_ordinal
    }

    pub const fn kind(self) -> BuildFilesystemObservedByteRegionKind {
        self.kind
    }

    pub const fn offset(self) -> u64 {
        self.offset
    }

    pub const fn length(self) -> u64 {
        self.length
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemMetadataObservationKind {
    FollowedPath,
    OpenDescriptor,
    UnfollowedFinalPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildFilesystemMetadataObservation {
    pub(super) output_operand_ordinal: u8,
    pub(super) kind: BuildFilesystemMetadataObservationKind,
    pub(super) device: u64,
    pub(super) mode: u32,
    pub(super) link_count: u64,
    pub(super) inode: u64,
    pub(super) user: u32,
    pub(super) group: u32,
    pub(super) referenced_device: u64,
    pub(super) access_time: i64,
    pub(super) modification_time: i64,
    pub(super) change_time: i64,
    pub(super) birth_time: i64,
    pub(super) size: i64,
    pub(super) blocks_512: u64,
    pub(super) preferred_block_size: u64,
}

impl BuildFilesystemMetadataObservation {
    pub const fn output_operand_ordinal(self) -> u8 {
        self.output_operand_ordinal
    }
    pub const fn kind(self) -> BuildFilesystemMetadataObservationKind {
        self.kind
    }
    pub const fn device(self) -> u64 {
        self.device
    }
    pub const fn mode(self) -> u32 {
        self.mode
    }
    pub const fn link_count(self) -> u64 {
        self.link_count
    }
    pub const fn inode(self) -> u64 {
        self.inode
    }
    pub const fn user(self) -> u32 {
        self.user
    }
    pub const fn group(self) -> u32 {
        self.group
    }
    pub const fn referenced_device(self) -> u64 {
        self.referenced_device
    }
    pub const fn access_time(self) -> i64 {
        self.access_time
    }
    pub const fn modification_time(self) -> i64 {
        self.modification_time
    }
    pub const fn change_time(self) -> i64 {
        self.change_time
    }
    pub const fn birth_time(self) -> i64 {
        self.birth_time
    }
    pub const fn size(self) -> i64 {
        self.size
    }
    pub const fn blocks_512(self) -> u64 {
        self.blocks_512
    }
    pub const fn preferred_block_size(self) -> u64 {
        self.preferred_block_size
    }
}

/// Complete mutable-byte carrier contents at the moment the authored operand
/// resolves. This is distinct from provider-visible pre/post state because
/// evaluation of a later argument may legally alias and mutate the carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFilesystemMutableByteOperandResolution {
    pub(super) operand_ordinal: u8,
    pub(super) bytes: Vec<u8>,
}

impl BuildFilesystemMutableByteOperandResolution {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildFilesystemMutableI64OperandResolution {
    pub(super) operand_ordinal: u8,
    pub(super) value: i64,
}

impl BuildFilesystemMutableI64OperandResolution {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn value(self) -> i64 {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFilesystemMutableByteOperand {
    pub(super) operand_ordinal: u8,
    pub(super) pre_bytes: Vec<u8>,
    pub(super) post_bytes: Vec<u8>,
}

impl BuildFilesystemMutableByteOperand {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub fn pre_bytes(&self) -> &[u8] {
        &self.pre_bytes
    }

    pub fn post_bytes(&self) -> &[u8] {
        &self.post_bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildFilesystemMutableI64Operand {
    pub(super) operand_ordinal: u8,
    pub(super) pre_value: i64,
    pub(super) post_value: i64,
}

impl BuildFilesystemMutableI64Operand {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn pre_value(self) -> i64 {
        self.pre_value
    }

    pub const fn post_value(self) -> i64 {
        self.post_value
    }
}

pub(super) const fn project_scalar_operand_value(
    value: checked_interpreter::FilesystemScalarOperandValue,
) -> BuildFilesystemScalarOperandValue {
    match value {
        checked_interpreter::FilesystemScalarOperandValue::I32(value) => {
            BuildFilesystemScalarOperandValue::I32(value)
        }
        checked_interpreter::FilesystemScalarOperandValue::U32(value) => {
            BuildFilesystemScalarOperandValue::U32(value)
        }
        checked_interpreter::FilesystemScalarOperandValue::I64(value) => {
            BuildFilesystemScalarOperandValue::I64(value)
        }
        checked_interpreter::FilesystemScalarOperandValue::U64(value) => {
            BuildFilesystemScalarOperandValue::U64(value)
        }
    }
}

/// Stable build-evaluation identity for one descriptor/handle lifetime in a
/// package build evaluation. Provider token values never define this identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuildFilesystemLogicalHandleIdentity(u64);

impl BuildFilesystemLogicalHandleIdentity {
    pub(super) const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemLogicalHandleKind {
    Descriptor,
    Native,
    Find,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemLogicalHandleInputResolution {
    Resolved(BuildFilesystemLogicalHandleIdentity),
    Null,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildFilesystemLogicalHandleInput {
    pub(super) operand_ordinal: u8,
    pub(super) kind: BuildFilesystemLogicalHandleKind,
    pub(super) resolution: BuildFilesystemLogicalHandleInputResolution,
}

impl BuildFilesystemLogicalHandleInput {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn kind(self) -> BuildFilesystemLogicalHandleKind {
        self.kind
    }

    pub const fn resolution(self) -> BuildFilesystemLogicalHandleInputResolution {
        self.resolution
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemLogicalHandleOutputSource {
    Created,
    Duplicated(BuildFilesystemLogicalHandleIdentity),
    Borrowed(BuildFilesystemLogicalHandleIdentity),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildFilesystemLogicalHandleOutput {
    pub(super) kind: BuildFilesystemLogicalHandleKind,
    pub(super) identity: BuildFilesystemLogicalHandleIdentity,
    pub(super) source: BuildFilesystemLogicalHandleOutputSource,
}

impl BuildFilesystemLogicalHandleOutput {
    pub const fn kind(self) -> BuildFilesystemLogicalHandleKind {
        self.kind
    }

    pub const fn identity(self) -> BuildFilesystemLogicalHandleIdentity {
        self.identity
    }

    pub const fn source(self) -> BuildFilesystemLogicalHandleOutputSource {
        self.source
    }
}

pub(super) const fn project_logical_handle_identity(
    identity: checked_interpreter::FilesystemLogicalHandleIdentity,
) -> BuildFilesystemLogicalHandleIdentity {
    match BuildFilesystemLogicalHandleIdentity::new(identity.get()) {
        Some(identity) => identity,
        None => panic!("checked-interpreter logical handle identity must be nonzero"),
    }
}

pub(super) const fn project_logical_handle_kind(
    kind: checked_interpreter::FilesystemLogicalHandleKind,
) -> BuildFilesystemLogicalHandleKind {
    match kind {
        checked_interpreter::FilesystemLogicalHandleKind::Descriptor => {
            BuildFilesystemLogicalHandleKind::Descriptor
        }
        checked_interpreter::FilesystemLogicalHandleKind::Native => {
            BuildFilesystemLogicalHandleKind::Native
        }
        checked_interpreter::FilesystemLogicalHandleKind::Find => {
            BuildFilesystemLogicalHandleKind::Find
        }
    }
}

pub(super) const fn project_logical_handle_input_resolution(
    resolution: checked_interpreter::FilesystemLogicalHandleInputResolution,
) -> BuildFilesystemLogicalHandleInputResolution {
    match resolution {
        checked_interpreter::FilesystemLogicalHandleInputResolution::Resolved(identity) => {
            BuildFilesystemLogicalHandleInputResolution::Resolved(project_logical_handle_identity(
                identity,
            ))
        }
        checked_interpreter::FilesystemLogicalHandleInputResolution::Null => {
            BuildFilesystemLogicalHandleInputResolution::Null
        }
        checked_interpreter::FilesystemLogicalHandleInputResolution::Unknown => {
            BuildFilesystemLogicalHandleInputResolution::Unknown
        }
    }
}

pub(super) const fn project_logical_handle_output_source(
    source: checked_interpreter::FilesystemLogicalHandleOutputSource,
) -> BuildFilesystemLogicalHandleOutputSource {
    match source {
        checked_interpreter::FilesystemLogicalHandleOutputSource::Created => {
            BuildFilesystemLogicalHandleOutputSource::Created
        }
        checked_interpreter::FilesystemLogicalHandleOutputSource::Duplicated(identity) => {
            BuildFilesystemLogicalHandleOutputSource::Duplicated(project_logical_handle_identity(
                identity,
            ))
        }
        checked_interpreter::FilesystemLogicalHandleOutputSource::Borrowed(identity) => {
            BuildFilesystemLogicalHandleOutputSource::Borrowed(project_logical_handle_identity(
                identity,
            ))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemOperationResult {
    Scalar(i64),
    LogicalHandle(BuildFilesystemLogicalHandleIdentity),
}

pub(super) const fn project_operation_result(
    result: checked_interpreter::FilesystemOperationResult,
) -> BuildFilesystemOperationResult {
    match result {
        checked_interpreter::FilesystemOperationResult::Scalar(value) => {
            BuildFilesystemOperationResult::Scalar(value)
        }
        checked_interpreter::FilesystemOperationResult::LogicalHandle(identity) => {
            BuildFilesystemOperationResult::LogicalHandle(project_logical_handle_identity(identity))
        }
    }
}

/// One completed call from a successful build evaluation. This partial row is
/// execution evidence, not a replay event or receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFilesystemOperationAttempt {
    pub(super) operation_tag: u16,
    pub(super) provider: BuildFilesystemProvider,
    pub(super) observation_class: BuildFilesystemOperationObservationClass,
    pub(super) result: BuildFilesystemOperationResult,
    pub(super) post_error: i32,
    pub(super) scalar_operands: Vec<BuildFilesystemScalarOperand>,
    pub(super) byte_operands: Vec<BuildFilesystemByteOperand>,
    pub(super) path_like_operands: Vec<BuildFilesystemPathLikeOperand>,
    pub(super) rooted_path_operand_resolutions: Vec<BuildFilesystemRootedPathOperandResolution>,
    pub(super) returned_paths: Vec<BuildFilesystemReturnedPath>,
    pub(super) observed_byte_regions: Vec<BuildFilesystemObservedByteRegion>,
    pub(super) metadata_observations: Vec<BuildFilesystemMetadataObservation>,
    pub(super) mutable_byte_operand_resolutions: Vec<BuildFilesystemMutableByteOperandResolution>,
    pub(super) mutable_i64_operand_resolutions: Vec<BuildFilesystemMutableI64OperandResolution>,
    pub(super) mutable_byte_operands: Vec<BuildFilesystemMutableByteOperand>,
    pub(super) mutable_i64_operands: Vec<BuildFilesystemMutableI64Operand>,
    pub(super) authorized_paths: Vec<BuildFilesystemAuthorizedPath>,
    pub(super) logical_handle_inputs: Vec<BuildFilesystemLogicalHandleInput>,
    pub(super) logical_handle_output: Option<BuildFilesystemLogicalHandleOutput>,
    pub(super) retired_logical_handles: Vec<BuildFilesystemLogicalHandleIdentity>,
    pub(super) grant_refusals: Vec<BuildFilesystemGrantRefusal>,
}

impl BuildFilesystemOperationAttempt {
    pub const fn operation_tag(&self) -> u16 {
        self.operation_tag
    }

    pub const fn provider(&self) -> BuildFilesystemProvider {
        self.provider
    }

    pub const fn observation_class(&self) -> BuildFilesystemOperationObservationClass {
        self.observation_class
    }

    pub const fn result(&self) -> BuildFilesystemOperationResult {
        self.result
    }

    pub const fn post_error(&self) -> i32 {
        self.post_error
    }

    pub fn scalar_operands(&self) -> &[BuildFilesystemScalarOperand] {
        &self.scalar_operands
    }

    pub fn byte_operands(&self) -> &[BuildFilesystemByteOperand] {
        &self.byte_operands
    }

    pub fn path_like_operands(&self) -> &[BuildFilesystemPathLikeOperand] {
        &self.path_like_operands
    }

    pub fn rooted_path_operand_resolutions(&self) -> &[BuildFilesystemRootedPathOperandResolution] {
        &self.rooted_path_operand_resolutions
    }

    pub fn returned_paths(&self) -> &[BuildFilesystemReturnedPath] {
        &self.returned_paths
    }

    pub fn observed_byte_regions(&self) -> &[BuildFilesystemObservedByteRegion] {
        &self.observed_byte_regions
    }

    pub fn metadata_observations(&self) -> &[BuildFilesystemMetadataObservation] {
        &self.metadata_observations
    }

    pub fn observed_bytes(&self, region: &BuildFilesystemObservedByteRegion) -> Option<&[u8]> {
        let output = self
            .mutable_byte_operands
            .iter()
            .find(|output| output.operand_ordinal == region.output_operand_ordinal)?;
        let offset = usize::try_from(region.offset).ok()?;
        let length = usize::try_from(region.length).ok()?;
        let end = offset.checked_add(length)?;
        output.post_bytes.get(offset..end)
    }

    pub fn mutable_byte_operand_resolutions(
        &self,
    ) -> &[BuildFilesystemMutableByteOperandResolution] {
        &self.mutable_byte_operand_resolutions
    }

    pub fn mutable_i64_operand_resolutions(&self) -> &[BuildFilesystemMutableI64OperandResolution] {
        &self.mutable_i64_operand_resolutions
    }

    pub fn mutable_byte_operands(&self) -> &[BuildFilesystemMutableByteOperand] {
        &self.mutable_byte_operands
    }

    pub fn mutable_i64_operands(&self) -> &[BuildFilesystemMutableI64Operand] {
        &self.mutable_i64_operands
    }

    pub fn authorized_paths(&self) -> &[BuildFilesystemAuthorizedPath] {
        &self.authorized_paths
    }

    pub fn logical_handle_inputs(&self) -> &[BuildFilesystemLogicalHandleInput] {
        &self.logical_handle_inputs
    }

    pub const fn logical_handle_output(&self) -> Option<BuildFilesystemLogicalHandleOutput> {
        self.logical_handle_output
    }

    pub fn retired_logical_handles(&self) -> &[BuildFilesystemLogicalHandleIdentity] {
        &self.retired_logical_handles
    }

    pub fn grant_refusals(&self) -> &[BuildFilesystemGrantRefusal] {
        &self.grant_refusals
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemReplayDisposition {
    NotReplayed,
    SourceInputsOnly,
    Complete,
}

/// Closed compiler verdict for one build's filesystem replay.
///
/// `Complete` is meaningful only as part of its owning
/// [`BuildObservationSummary`]: the summary retains the exact operation
/// sequence, staged Output commitment, and generated-source handoffs that the
/// compiler compared while issuing the verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildFilesystemReplayVerdict {
    pub(super) schema_version: u32,
    pub(super) disposition: BuildFilesystemReplayDisposition,
}

impl BuildFilesystemReplayVerdict {
    pub(super) const fn new(disposition: BuildFilesystemReplayDisposition) -> Self {
        Self {
            schema_version: BUILD_FILESYSTEM_REPLAY_VERDICT_SCHEMA_VERSION,
            disposition,
        }
    }

    pub const fn schema_version(self) -> u32 {
        self.schema_version
    }

    pub const fn disposition(self) -> BuildFilesystemReplayDisposition {
        self.disposition
    }

    pub const fn replays_source_inputs(self) -> bool {
        matches!(
            self.disposition,
            BuildFilesystemReplayDisposition::SourceInputsOnly
                | BuildFilesystemReplayDisposition::Complete
        )
    }

    pub const fn is_complete(self) -> bool {
        matches!(self.disposition, BuildFilesystemReplayDisposition::Complete)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildObservationSummary {
    pub(super) schema_version: u32,
    pub(super) ceiling: BuildObservationClass,
    pub(super) realized: BuildObservationClass,
    pub(super) filesystem_operation_schema_version: u32,
    pub(super) filesystem_operation_attempts: Vec<BuildFilesystemOperationAttempt>,
    pub(super) canonical_source_metadata_identity: Option<BuildCanonicalSourceMetadataIdentity>,
    pub(super) filesystem_replay_verdict: BuildFilesystemReplayVerdict,
    pub(super) included_source_handoffs: Vec<BuildIncludedSourceHandoff>,
    pub(super) staged_output_tree: Option<BuildStagedOutputTree>,
    pub(super) build_log: Vec<u8>,
}

/// Exact explicit publication of one retained Output file as generated Omega
/// source. Ordering is authored call order; the ordinal is the number of
/// completed filesystem attempts when the handoff occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildIncludedSourceHandoff {
    pub(super) relative_path: Vec<u8>,
    pub(super) filesystem_attempt_ordinal: u64,
}

impl BuildIncludedSourceHandoff {
    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }

    pub const fn filesystem_attempt_ordinal(&self) -> u64 {
        self.filesystem_attempt_ordinal
    }
}

impl BuildObservationSummary {
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub const fn ceiling(&self) -> BuildObservationClass {
        self.ceiling
    }

    pub const fn realized(&self) -> BuildObservationClass {
        self.realized
    }

    pub const fn filesystem_operation_schema_version(&self) -> u32 {
        self.filesystem_operation_schema_version
    }

    pub const fn canonical_source_metadata_identity(
        &self,
    ) -> Option<BuildCanonicalSourceMetadataIdentity> {
        self.canonical_source_metadata_identity
    }

    pub const fn staged_output_tree(&self) -> Option<&BuildStagedOutputTree> {
        self.staged_output_tree.as_ref()
    }

    /// Exact bytes emitted through the compiler-owned `Build.log` facet.
    pub fn build_log(&self) -> &[u8] {
        &self.build_log
    }

    /// Exact ordered Output-relative generated-source handoffs. Ordinary
    /// retained output files are absent.
    pub fn included_source_handoffs(&self) -> &[BuildIncludedSourceHandoff] {
        &self.included_source_handoffs
    }

    /// One versioned disposition over the compiler's replay of this summary.
    /// `SourceInputsOnly` proves only provider-free consumption of the exact
    /// Source prefix. `Complete` additionally proves exact build-result and
    /// attempted-operation equality, generated-source handoffs, replay
    /// teardown, reconstructed Output namespace, and staged-output custody.
    pub const fn filesystem_replay_verdict(&self) -> BuildFilesystemReplayVerdict {
        self.filesystem_replay_verdict
    }

    /// Ordered operation/result/error evidence from the successful evaluator
    /// run. Direct scoped path authorizations are compiler-rooted, but this is
    /// intentionally broader than the currently admitted replay grammar. Exact
    /// path results, file and directory regions, and canonical metadata
    /// observations are retained even when no complete replay claim is made.
    pub fn filesystem_operation_attempts(&self) -> &[BuildFilesystemOperationAttempt] {
        &self.filesystem_operation_attempts
    }
}
