//! Durable build observation records and evaluator evidence projections.

use build_output::BuildStagedOutputTree;

// Filesystem facts retain the evaluator's vocabulary and logical identities.
// Only root meaning and successful-run custody are build-specific below.
pub use checked_interpreter::BuildEvaluationUsage;
pub use filesystem_authority::{
    FilesystemGrantAccess as BuildFilesystemGrantAccess,
    FilesystemGrantRefusal as BuildFilesystemGrantRefusal,
    FilesystemGrantRefusalReason as BuildFilesystemGrantRefusalReason,
    FilesystemLogicalHandleIdentity as BuildFilesystemLogicalHandleIdentity,
    FilesystemLogicalHandleInput as BuildFilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution as BuildFilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind as BuildFilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutput as BuildFilesystemLogicalHandleOutput,
    FilesystemLogicalHandleOutputSource as BuildFilesystemLogicalHandleOutputSource,
    FilesystemObservationProvider as BuildFilesystemProvider,
    FilesystemOperationResult as BuildFilesystemOperationResult,
};

pub const BUILD_OBSERVATION_SCHEMA_VERSION: u32 = 82;

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
    pub(crate) root_role: Option<crate::BuildDeclarationKind>,
    pub(crate) build_execution_profile: Option<target::TargetProfile>,
}

impl BuildActivation {
    /// Root package occurrence the activation was admitted under.
    pub const fn root_package_identity(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        self.root_package_identity
    }

    /// Authored declaration role (`package` or `application`) of the root
    /// occurrence.
    pub const fn root_role(&self) -> Option<crate::BuildDeclarationKind> {
        self.root_role
    }

    /// The admitted build execution profile the activation's build-scope
    /// sources were checked for and its build machine executed under. This is
    /// the request's admitted profile, not a value inferred from the compiler
    /// process, and it is distinct from the selected product target.
    pub const fn build_execution_profile(&self) -> Option<target::TargetProfile> {
        self.build_execution_profile
    }
}

/// Stable build-evaluation identity for a package filesystem root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemRoot {
    Source,
    Output,
    /// Compiler-issued root joined to the observation's named inventory.
    NamedInput(u32),
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

/// A named immutable input bound to the exact read root used by this build.
/// Names and unread inventory members participate in observation identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildNamedInputInventory {
    pub(crate) name: Vec<u8>,
    pub(crate) root_identity: u32,
    pub(crate) inventory: BuildCapturedSourceInventory,
}

impl BuildNamedInputInventory {
    pub fn name(&self) -> &[u8] {
        &self.name
    }
    pub const fn root_identity(&self) -> u32 {
        self.root_identity
    }
    pub const fn inventory(&self) -> BuildCapturedSourceInventory {
        self.inventory
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
    pub(crate) named_input_inventories: Vec<BuildNamedInputInventory>,
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

    /// Canonical slot order binds names, roots, complete metadata, and bytes.
    pub fn named_input_inventories(&self) -> &[BuildNamedInputInventory] {
        &self.named_input_inventories
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
