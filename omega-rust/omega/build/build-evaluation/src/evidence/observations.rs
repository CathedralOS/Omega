//! Durable build observation records and evaluator evidence projections.

use build_output::BuildStagedOutputTree;

/// Accounting-only projection of the transitional typed-tree build evaluator.
/// This is not terminal-Psi fuel and does not participate in `BuildConfig` or
/// program identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildEvaluationUsage {
    pub usage_schema_version: u32,
    pub step_schedule_marker: u32,
    /// Deterministic ceiling applied to the single evaluation, not a CPU limit.
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
    /// Fuel consumed by this build-machine evaluation.
    pub fuel_units: u64,
    /// BuildLog bytes emitted by this evaluation.
    pub build_log_bytes: u64,
    /// Filesystem operation attempts retained by this evaluation.
    pub filesystem_operation_attempts: u64,
    /// Maximum semantic interpreter cells live concurrently in this
    /// evaluation.
    pub peak_live_cells: u64,
    /// Maximum logical bytes in live Text backing buffers during this
    /// evaluation.
    pub peak_live_text_bytes: u64,
    pub result_cells: u64,
    pub result_text_bytes: u64,
}

impl BuildEvaluationUsage {
    /// Compare the deterministic accounting owned by one build-machine
    /// invocation, excluding aggregate sponsor/session
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
            && self.build_log_bytes == other.build_log_bytes
            && self.filesystem_operation_attempts == other.filesystem_operation_attempts
            && self.peak_live_cells == other.peak_live_cells
            && self.peak_live_text_bytes == other.peak_live_text_bytes
            && self.result_cells == other.result_cells
            && self.result_text_bytes == other.result_text_bytes
    }
}

pub const BUILD_OBSERVATION_SCHEMA_VERSION: u32 = 80;

/// Provider used for an observed filesystem operation.
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
    pub(crate) policy_version: u32,
    pub(crate) source_content_commitment: [u8; 32],
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

/// The exact admitted package occurrence, declaration role, and profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BuildActivation {
    pub(crate) root_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub(crate) root_role: Option<package_compilation::BuildDeclarationKind>,
    pub(crate) selected_target_profile: Option<target::TargetProfile>,
    pub(crate) build_execution_profile: Option<target::TargetProfile>,
}

impl BuildActivation {
    /// Root package occurrence the activation was admitted under.
    pub const fn root_package_identity(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        self.root_package_identity
    }

    /// Authored declaration role (`package` or `application`) of the root
    /// occurrence.
    pub const fn root_role(&self) -> Option<package_compilation::BuildDeclarationKind> {
        self.root_role
    }

    /// Requested target the activation's build machine could observe.
    pub const fn selected_target_profile(&self) -> Option<target::TargetProfile> {
        self.selected_target_profile
    }

    /// The admitted build execution profile the activation's build-scope
    /// sources were checked for and its build machine executed under. This is
    /// the request's admitted profile, not a value inferred from the compiler
    /// process, and it is distinct from the selected product target.
    pub const fn build_execution_profile(&self) -> Option<target::TargetProfile> {
        self.build_execution_profile
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
    pub(crate) operand_ordinal: u8,
    pub(crate) access: BuildFilesystemGrantAccess,
    pub(crate) reason: BuildFilesystemGrantRefusalReason,
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
    pub(crate) operand_ordinal: u8,
    pub(crate) access: BuildFilesystemGrantAccess,
    pub(crate) root: BuildFilesystemRoot,
    pub(crate) relative_path: Vec<u8>,
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

/// Stable build-evaluation identity for one descriptor/handle lifetime in a
/// package build evaluation. Provider token values never define this identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuildFilesystemLogicalHandleIdentity(u64);

impl BuildFilesystemLogicalHandleIdentity {
    pub(crate) const fn new(value: u64) -> Option<Self> {
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
    pub(crate) operand_ordinal: u8,
    pub(crate) kind: BuildFilesystemLogicalHandleKind,
    pub(crate) resolution: BuildFilesystemLogicalHandleInputResolution,
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
    pub(crate) kind: BuildFilesystemLogicalHandleKind,
    pub(crate) identity: BuildFilesystemLogicalHandleIdentity,
    pub(crate) source: BuildFilesystemLogicalHandleOutputSource,
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

pub(crate) const fn project_logical_handle_identity(
    identity: checked_interpreter::FilesystemLogicalHandleIdentity,
) -> BuildFilesystemLogicalHandleIdentity {
    match BuildFilesystemLogicalHandleIdentity::new(identity.get()) {
        Some(identity) => identity,
        None => panic!("checked-interpreter logical handle identity must be nonzero"),
    }
}

pub(crate) const fn project_logical_handle_kind(
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

pub(crate) const fn project_logical_handle_input_resolution(
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

pub(crate) const fn project_logical_handle_output_source(
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

pub(crate) const fn project_operation_result(
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
/// execution evidence, not authorization for another execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFilesystemOperationAttempt {
    pub(crate) operation_tag: u16,
    pub(crate) provider: BuildFilesystemProvider,
    pub(crate) result: BuildFilesystemOperationResult,
    pub(crate) post_error: i32,
    pub(crate) authorized_paths: Vec<BuildFilesystemAuthorizedPath>,
    pub(crate) logical_handle_inputs: Vec<BuildFilesystemLogicalHandleInput>,
    pub(crate) logical_handle_output: Option<BuildFilesystemLogicalHandleOutput>,
    pub(crate) retired_logical_handles: Vec<BuildFilesystemLogicalHandleIdentity>,
    pub(crate) grant_refusals: Vec<BuildFilesystemGrantRefusal>,
}

impl BuildFilesystemOperationAttempt {
    pub const fn operation_tag(&self) -> u16 {
        self.operation_tag
    }

    pub const fn provider(&self) -> BuildFilesystemProvider {
        self.provider
    }

    pub const fn result(&self) -> BuildFilesystemOperationResult {
        self.result
    }

    pub const fn post_error(&self) -> i32 {
        self.post_error
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

/// Extent evidence of the captured immutable input inventory a build
/// occurrence executed against. Its commitment is distinct from the full
/// package's source provenance: equally sized selections from one package
/// must remain different inputs even when neither build reads a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildCapturedSourceInventory {
    pub(crate) entry_count: u64,
    pub(crate) file_bytes: u64,
    pub(crate) source_metadata_identity: BuildCanonicalSourceMetadataIdentity,
}

impl BuildCapturedSourceInventory {
    /// Exact membership, metadata policy, and byte commitment of the selected view.
    pub const fn source_metadata_identity(self) -> BuildCanonicalSourceMetadataIdentity {
        self.source_metadata_identity
    }

    /// Complete inventory size in canonical rows, including the source root.
    pub const fn entry_count(self) -> u64 {
        self.entry_count
    }

    /// Total retained regular-file bytes across the inventory.
    pub const fn file_bytes(self) -> u64 {
        self.file_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildObservationSummary {
    pub(crate) schema_version: u32,
    pub(crate) filesystem_host_observed: bool,
    pub(crate) filesystem_operation_schema_version: u32,
    pub(crate) filesystem_operation_attempts: Vec<BuildFilesystemOperationAttempt>,
    pub(crate) canonical_source_metadata_identity: Option<BuildCanonicalSourceMetadataIdentity>,
    pub(crate) activation: BuildActivation,
    pub(crate) captured_source_inventory: Option<BuildCapturedSourceInventory>,
    pub(crate) included_source_handoffs: Vec<BuildIncludedSourceHandoff>,
    /// Terminal settlement rows for the compiler-owned required-output
    /// obligations issued through `BuildOutput::require`, in issue order.
    /// Only `Completed` obligations survive settlement, so every row pairs
    /// a declared canonical name with the sealed-attempt ordinal its
    /// completion receipt recorded.
    pub(crate) required_output_settlements: Vec<BuildRequiredOutputSettlement>,
    pub(crate) staged_output_tree: Option<BuildStagedOutputTree>,
    pub(crate) build_log: Vec<u8>,
}

/// One required-output obligation settled during build evaluation. The
/// declared canonical name is paired with the filesystem-attempt ordinal
/// that sealed its file; the sealed bytes themselves remain inside the
/// staged-output tree commitment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRequiredOutputSettlement {
    pub(crate) relative_path: Vec<u8>,
    pub(crate) sealed_attempt_ordinal: u64,
}

impl BuildRequiredOutputSettlement {
    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }

    pub const fn sealed_attempt_ordinal(&self) -> u64 {
        self.sealed_attempt_ordinal
    }
}

/// Exact explicit publication of one retained Output file as generated Omega
/// source. Ordering is authored call order; the ordinal is the number of
/// completed filesystem attempts when the handoff occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildIncludedSourceHandoff {
    pub(crate) relative_path: Vec<u8>,
    pub(crate) filesystem_attempt_ordinal: u64,
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

    pub const fn filesystem_host_observed(&self) -> bool {
        self.filesystem_host_observed
    }

    pub const fn filesystem_operation_schema_version(&self) -> u32 {
        self.filesystem_operation_schema_version
    }

    pub const fn canonical_source_metadata_identity(
        &self,
    ) -> Option<BuildCanonicalSourceMetadataIdentity> {
        self.canonical_source_metadata_identity
    }

    /// The exact activation this summary's evidence was produced under.
    /// Retained observations bind to this tuple; a
    /// different root package, declaration role, or selected target is a
    /// different activation, not a refresh of the same one.
    pub const fn activation(&self) -> BuildActivation {
        self.activation
    }

    /// Identity and extent of this occurrence's selected immutable inventory.
    /// Execution reads its private materialization.
    pub const fn captured_source_inventory(&self) -> Option<BuildCapturedSourceInventory> {
        self.captured_source_inventory
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

    /// Ordered required-output settlement rows in `require` issue order.
    /// Names here completed their linear obligation; files present only in
    /// the staged tree are ordinary outputs, not completed obligations.
    pub fn required_output_settlements(&self) -> &[BuildRequiredOutputSettlement] {
        &self.required_output_settlements
    }

    /// Ordered operation/result/error evidence from the successful evaluator
    /// run. Scoped path authorizations are compiler-rooted.
    pub fn filesystem_operation_attempts(&self) -> &[BuildFilesystemOperationAttempt] {
        &self.filesystem_operation_attempts
    }
}
