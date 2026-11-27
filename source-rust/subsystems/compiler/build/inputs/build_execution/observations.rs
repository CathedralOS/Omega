// Durable build observation records (`BuildObservationSummary`,
// `BuildActivation`, `BuildFilesystemOperationAttempt`,
// `BuildCapturedSourceInventory`,
// `BuildRequiredOutputSettlement`) produced when
// a build evaluation settles its filesystem and output obligations.

use build_filesystem::trees::BuildStagedOutputTree;
pub use build_filesystem::trees::{
    BuildCanonicalSourceMetadataIdentity, BuildCapturedSourceInventory,
};

use build_filesystem::authority::{
    FilesystemGrantAccess, FilesystemGrantRefusal, FilesystemLogicalHandleIdentity,
    FilesystemLogicalHandleInput, FilesystemLogicalHandleOutput, FilesystemObservationProvider,
    FilesystemOperationResult,
};
pub use build_model::contracts::BuildEvaluationUsage;

pub const BUILD_OBSERVATION_SCHEMA_VERSION: u32 = 86;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFilesystemAuthorizedPath {
    pub(crate) operand_ordinal: u8,
    pub(crate) access: FilesystemGrantAccess,
    pub(crate) root: BuildFilesystemRoot,
    pub(crate) relative_path: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFilesystemRoot {
    Source,
    Output,
    /// Relative to the guest root among this activation's live directory grants.
    MappedDirectories,
}

impl BuildFilesystemAuthorizedPath {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub const fn access(&self) -> FilesystemGrantAccess {
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
    pub(crate) provider: FilesystemObservationProvider,
    pub(crate) result: FilesystemOperationResult,
    pub(crate) post_error: i32,
    pub(crate) authorized_paths: Vec<BuildFilesystemAuthorizedPath>,
    pub(crate) logical_handle_inputs: Vec<FilesystemLogicalHandleInput>,
    pub(crate) logical_handle_output: Option<FilesystemLogicalHandleOutput>,
    pub(crate) retired_logical_handles: Vec<FilesystemLogicalHandleIdentity>,
    pub(crate) grant_refusals: Vec<FilesystemGrantRefusal>,
}

impl BuildFilesystemOperationAttempt {
    pub const fn operation_tag(&self) -> u16 {
        self.operation_tag
    }

    pub const fn provider(&self) -> FilesystemObservationProvider {
        self.provider
    }

    pub const fn result(&self) -> FilesystemOperationResult {
        self.result
    }

    pub const fn post_error(&self) -> i32 {
        self.post_error
    }

    pub fn authorized_paths(&self) -> &[BuildFilesystemAuthorizedPath] {
        &self.authorized_paths
    }

    pub fn logical_handle_inputs(&self) -> &[FilesystemLogicalHandleInput] {
        &self.logical_handle_inputs
    }

    pub const fn logical_handle_output(&self) -> Option<FilesystemLogicalHandleOutput> {
        self.logical_handle_output
    }

    pub fn retired_logical_handles(&self) -> &[FilesystemLogicalHandleIdentity] {
        &self.retired_logical_handles
    }

    pub fn grant_refusals(&self) -> &[FilesystemGrantRefusal] {
        &self.grant_refusals
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildObservationSummary {
    pub(crate) filesystem_host_observed: bool,
    pub(crate) filesystem_operation_schema_version: u32,
    pub(crate) filesystem_operation_attempts: Vec<BuildFilesystemOperationAttempt>,
    pub(crate) canonical_source_metadata_identity: Option<BuildCanonicalSourceMetadataIdentity>,
    pub(crate) activation: BuildActivation,
    pub(crate) captured_source_inventory: Option<BuildCapturedSourceInventory>,
    pub(super) product_source_tree: Option<BuildStagedOutputTree>,
    pub(super) generated_sources: Vec<build_filesystem::trees::PackageGeneratedSource>,
    pub(crate) required_output_settlements: Vec<BuildRequiredOutputSettlement>, // Completed `BuildOutput::require` obligations in issue order.
    pub(crate) staged_output_tree: Option<BuildStagedOutputTree>,
    pub(crate) build_log: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRequiredOutputSettlement {
    pub(crate) relative_path: Vec<u8>,
    pub(super) staged_file: Vec<u8>,
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

impl BuildObservationSummary {
    pub const fn schema_version(&self) -> u32 {
        BUILD_OBSERVATION_SCHEMA_VERSION
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

    pub const fn staged_output_tree(&self) -> Option<&BuildStagedOutputTree> {
        self.staged_output_tree.as_ref()
    }

    pub fn build_log(&self) -> &[u8] {
        &self.build_log
    }

    pub fn product_source_tree(&self) -> Option<&BuildStagedOutputTree> {
        self.product_source_tree.as_ref()
    }

    pub fn generated_sources(&self) -> &[build_filesystem::trees::PackageGeneratedSource] {
        &self.generated_sources
    }

    pub fn required_output_settlements(&self) -> &[BuildRequiredOutputSettlement] {
        &self.required_output_settlements
    }

    pub fn filesystem_operation_attempts(&self) -> &[BuildFilesystemOperationAttempt] {
        &self.filesystem_operation_attempts
    }
}
