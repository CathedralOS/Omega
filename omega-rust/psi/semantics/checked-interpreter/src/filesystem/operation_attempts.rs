//! Operation attempts, outcomes, halts and service bindings.

use crate::filesystem::{
    FilesystemAuthorizedPath, FilesystemByteOperand, FilesystemGrantRefusal,
    FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput, FilesystemLogicalHandleOutput,
    FilesystemMetadataObservation, FilesystemMutableByteOperand,
    FilesystemMutableByteOperandResolution, FilesystemMutableI64Operand,
    FilesystemMutableI64OperandResolution, FilesystemObservationProvider,
    FilesystemObservedByteRegion, FilesystemPathLikeOperand, FilesystemReturnedPath,
    FilesystemRootedPathOperandResolution, FilesystemScalarOperand,
};
use checked_trees::CheckedTrees;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemEvaluationHaltKind {
    Exit,
    Unsupported,
    Trap,
    ResourceExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemOperationResult {
    Scalar(i64),
    LogicalHandle(FilesystemLogicalHandleIdentity),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemOperationAttemptOutcome {
    Returned {
        result: FilesystemOperationResult,
        post_error: i32,
    },
    EvaluationHalted(FilesystemEvaluationHaltKind),
}

/// One completed canonical filesystem operation attempted during build-machine
/// evaluation. Failed evaluations retain their completed prefix as
/// non-admission evidence.
///
/// The operation tag is an append-only compiler-owned identity. No package
/// string enters this row. Successful descriptor/handle results and uses are
/// normalized into logical lifetimes; provider token numbers do not survive.
/// Failed handle-result sentinels remain scalar results. Mutable carriers
/// retain both their successfully resolved preparation prefix and complete
/// provider-visible pre/post snapshots. Path results and successful file and
/// directory and metadata observations have semantic rows. Replay execution
/// remains incomplete, so this stays below receipt strength.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemOperationAttempt {
    pub(crate) operation_tag: u16,
    pub(crate) provider: FilesystemObservationProvider,
    pub(crate) outcome: Option<FilesystemOperationAttemptOutcome>,
    pub(crate) scalar_operands: Vec<FilesystemScalarOperand>,
    pub(crate) byte_operands: Vec<FilesystemByteOperand>,
    pub(crate) path_like_operands: Vec<FilesystemPathLikeOperand>,
    pub(crate) rooted_path_operand_resolutions: Vec<FilesystemRootedPathOperandResolution>,
    pub(crate) returned_paths: Vec<FilesystemReturnedPath>,
    pub(crate) observed_byte_regions: Vec<FilesystemObservedByteRegion>,
    pub(crate) metadata_observations: Vec<FilesystemMetadataObservation>,
    pub(crate) mutable_byte_operand_resolutions: Vec<FilesystemMutableByteOperandResolution>,
    pub(crate) mutable_i64_operand_resolutions: Vec<FilesystemMutableI64OperandResolution>,
    pub(crate) mutable_byte_operands: Vec<FilesystemMutableByteOperand>,
    pub(crate) mutable_i64_operands: Vec<FilesystemMutableI64Operand>,
    pub(crate) authorized_paths: Vec<FilesystemAuthorizedPath>,
    pub(crate) logical_handle_inputs: Vec<FilesystemLogicalHandleInput>,
    pub(crate) logical_handle_output: Option<FilesystemLogicalHandleOutput>,
    pub(crate) retired_logical_handles: Vec<FilesystemLogicalHandleIdentity>,
    pub(crate) grant_refusals: Vec<FilesystemGrantRefusal>,
}

impl FilesystemOperationAttempt {
    pub(crate) const fn pending(
        operation_tag: u16,
        provider: FilesystemObservationProvider,
    ) -> Self {
        Self {
            operation_tag,
            provider,
            outcome: None,
            scalar_operands: Vec::new(),
            byte_operands: Vec::new(),
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: Vec::new(),
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: Vec::new(),
            mutable_i64_operand_resolutions: Vec::new(),
            mutable_byte_operands: Vec::new(),
            mutable_i64_operands: Vec::new(),
            authorized_paths: Vec::new(),
            logical_handle_inputs: Vec::new(),
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        }
    }

    pub const fn operation_tag(&self) -> u16 {
        self.operation_tag
    }

    pub const fn provider(&self) -> FilesystemObservationProvider {
        self.provider
    }

    pub const fn outcome(&self) -> Option<FilesystemOperationAttemptOutcome> {
        self.outcome
    }

    pub const fn result(&self) -> Option<FilesystemOperationResult> {
        match self.outcome {
            Some(FilesystemOperationAttemptOutcome::Returned { result, .. }) => Some(result),
            _ => None,
        }
    }

    pub const fn post_error(&self) -> Option<i32> {
        match self.outcome {
            Some(FilesystemOperationAttemptOutcome::Returned { post_error, .. }) => {
                Some(post_error)
            }
            _ => None,
        }
    }

    pub fn scalar_operands(&self) -> &[FilesystemScalarOperand] {
        &self.scalar_operands
    }

    pub fn byte_operands(&self) -> &[FilesystemByteOperand] {
        &self.byte_operands
    }

    pub fn path_like_operands(&self) -> &[FilesystemPathLikeOperand] {
        &self.path_like_operands
    }

    pub fn rooted_path_operand_resolutions(&self) -> &[FilesystemRootedPathOperandResolution] {
        &self.rooted_path_operand_resolutions
    }

    pub fn returned_paths(&self) -> &[FilesystemReturnedPath] {
        &self.returned_paths
    }

    pub fn observed_byte_regions(&self) -> &[FilesystemObservedByteRegion] {
        &self.observed_byte_regions
    }

    pub fn metadata_observations(&self) -> &[FilesystemMetadataObservation] {
        &self.metadata_observations
    }

    pub fn mutable_byte_operand_resolutions(&self) -> &[FilesystemMutableByteOperandResolution] {
        &self.mutable_byte_operand_resolutions
    }

    pub fn mutable_i64_operand_resolutions(&self) -> &[FilesystemMutableI64OperandResolution] {
        &self.mutable_i64_operand_resolutions
    }

    pub fn mutable_byte_operands(&self) -> &[FilesystemMutableByteOperand] {
        &self.mutable_byte_operands
    }

    pub fn mutable_i64_operands(&self) -> &[FilesystemMutableI64Operand] {
        &self.mutable_i64_operands
    }

    pub fn grant_refusals(&self) -> &[FilesystemGrantRefusal] {
        &self.grant_refusals
    }

    pub fn authorized_paths(&self) -> &[FilesystemAuthorizedPath] {
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
}

/// Same-program routing custody for one compiler-resolved filesystem boundary.
///
/// This token grants no filesystem access and proves no package admission. It
/// only prevents the checked interpreter from rediscovering a package-owned
/// service through a readable name or source path after Omega has already
/// resolved an exact accepted semantic binding. The token is valid only for
/// the exact [`CheckedTrees`] instance from which it was constructed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilesystemServiceBinding {
    pub(crate) checked_program_address: usize,
    pub(crate) declaration_symbol: symbols::SymbolHandle,
}

impl FilesystemServiceBinding {
    pub fn from_compiler_resolved_declaration(
        checked: &CheckedTrees,
        declaration_symbol: symbols::SymbolHandle,
    ) -> Result<Self, &'static str> {
        if !declaration_symbol.is_valid()
            || checked
                .typed
                .traits()
                .iter()
                .filter(|definition| {
                    definition.is_boundary && definition.symbol == declaration_symbol
                })
                .count()
                != 1
        {
            return Err("filesystem service binding is not one exact checked boundary declaration");
        }
        Ok(Self {
            checked_program_address: std::ptr::from_ref(checked).addr(),
            declaration_symbol,
        })
    }

    pub(crate) fn declaration_symbol_for(
        self,
        checked: &CheckedTrees,
    ) -> Result<symbols::SymbolHandle, &'static str> {
        if self.checked_program_address != std::ptr::from_ref(checked).addr() {
            return Err("filesystem service binding belongs to a different checked program");
        }
        Ok(self.declaration_symbol)
    }
}
