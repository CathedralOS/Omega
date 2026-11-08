// Durable build observation records (`BuildObservationSummary`,
// `BuildActivation`, `BuildFilesystemOperationAttempt`,
// `BuildCapturedSourceInventory`, `BuildNamedInputInventory`,
// `BuildRequiredOutputSettlement`, `BuildIncludedSourceHandoff`) produced when
// a build evaluation settles its filesystem and output obligations.

use build_output::BuildStagedOutputTree;

pub use evaluation_accounting::BuildEvaluationUsage;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BuildActivation {
    pub(crate) root_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub(crate) root_role: Option<crate::BuildDeclarationKind>,
    pub(crate) build_execution_profile: Option<target::TargetProfile>,
}

impl BuildActivation {
    pub const fn root_package_identity(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        self.root_package_identity
    }

    pub const fn root_role(&self) -> Option<crate::BuildDeclarationKind> {
        self.root_role
    }

    pub const fn build_execution_profile(&self) -> Option<target::TargetProfile> {
        self.build_execution_profile
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemRoot {
    Source,
    Output,
    NamedInput(u32), // Compiler-issued root joined to the observation's named inventory.
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildCapturedSourceInventory {
    pub(crate) entry_count: u64,
    pub(crate) file_bytes: u64,
    pub(crate) source_metadata_identity: BuildCanonicalSourceMetadataIdentity,
}

impl BuildCapturedSourceInventory {
    pub const fn source_metadata_identity(self) -> BuildCanonicalSourceMetadataIdentity {
        self.source_metadata_identity
    }

    pub const fn entry_count(self) -> u64 {
        self.entry_count
    }

    pub const fn file_bytes(self) -> u64 {
        self.file_bytes
    }
}

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
    pub(crate) required_output_settlements: Vec<BuildRequiredOutputSettlement>, // Completed `BuildOutput::require` obligations in issue order.
    pub(crate) staged_output_tree: Option<BuildStagedOutputTree>,
    pub(crate) build_log: Vec<u8>,
}

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

    pub const fn activation(&self) -> BuildActivation {
        self.activation
    }

    pub const fn captured_source_inventory(&self) -> Option<BuildCapturedSourceInventory> {
        self.captured_source_inventory
    }

    pub fn named_input_inventories(&self) -> &[BuildNamedInputInventory] {
        &self.named_input_inventories
    }

    pub const fn staged_output_tree(&self) -> Option<&BuildStagedOutputTree> {
        self.staged_output_tree.as_ref()
    }

    pub fn build_log(&self) -> &[u8] {
        &self.build_log
    }

    pub fn included_source_handoffs(&self) -> &[BuildIncludedSourceHandoff] {
        &self.included_source_handoffs
    }

    pub fn required_output_settlements(&self) -> &[BuildRequiredOutputSettlement] {
        &self.required_output_settlements
    }

    pub fn filesystem_operation_attempts(&self) -> &[BuildFilesystemOperationAttempt] {
        &self.filesystem_operation_attempts
    }
}
