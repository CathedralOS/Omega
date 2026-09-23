//! Optimizer module role: representation entrance. Durable records of the optimized ProgramStorage semantic-wrapper object.
//!
//! The wrapper object is one compiler-owned composite: a resolved semantic
//! wrapper prefixed to a validated relocation-free Terminal child. This file
//! declares the plan, container, manifest, and custody receipt that outlive
//! the native-realization stage which joins them to settlement and encoding
//! custody. `composition` builds the sealed plan from the child object and the
//! validated wrapper template, `validation` checks the plan's shape and its
//! agreement with that template, `manifest` derives and replays the manifest,
//! and `codec` owns the canonical wire forms.

use isa_x86_64::{
    ValidatedX86_64SemanticUnitWrapperTemplate, X86_64SemanticUnitWrapperResolutionError,
};
use object_file::ObjectLocalSymbolId;
use optimization_core::{
    OptimizedObjectArtifactIdentity, OptimizedObjectArtifactManifestIdentity,
    OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    OptimizedProgramStorageSemanticWrapperObjectIdentity,
    OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
};
use semantic_vocabulary::MachineId;
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

mod codec;
mod composition;
mod manifest;
mod validation;

pub use codec::{
    decode_optimized_program_storage_semantic_wrapper_object,
    encode_optimized_program_storage_semantic_wrapper_object,
    encode_optimized_program_storage_semantic_wrapper_object_preserving_seal,
};
pub use composition::compose_optimized_program_storage_semantic_wrapper_object;

use codec::{decode_manifest, encode_manifest, encode_manifest_content, encode_plan_content};
use manifest::{construct_manifest, validate_manifest};
use validation::validate_object_preserving_seal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperObjectSymbolRole {
    SemanticWrapperV1,
    PrivateTerminalContinuationV1,
    PrivateTerminalFunctionV1,
}

/// One symbol in the composite object. The wrapper intentionally has no
/// `MachineId`; copied Terminal symbols retain their exact Machine identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperObjectSymbol {
    pub symbol: ObjectLocalSymbolId,
    pub source_function_index: Option<u64>,
    pub machine: Option<MachineId>,
    pub name: String,
    pub section_offset: u64,
    pub byte_count: u64,
    pub role: OptimizedProgramStorageSemanticWrapperObjectSymbolRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperCallResolutionState {
    ResolvedInCompositeTextSectionV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperCallResolution {
    pub state: OptimizedProgramStorageSemanticWrapperCallResolutionState,
    pub wrapper_section_offset: u64,
    pub continuation_section_offset: u64,
    pub next_instruction_section_offset: u64,
    pub displacement: i32,
}

/// A compiler-owned composite object. It retains the child object's identity,
/// but it is not a `RelocationFreeObjectPlan`: its first symbol has no
/// semantic Machine owner and its text has a distinct source lineage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperObjectPlan {
    pub identity: OptimizedProgramStorageSemanticWrapperObjectIdentity,
    pub source_artifact: OptimizedObjectArtifactIdentity,
    pub source_artifact_manifest: OptimizedObjectArtifactManifestIdentity,
    pub source_object: RelocationFreeObjectPlanIdentity,
    pub source_object_container: RelocationFreeObjectContainerIdentity,
    pub source_signature: [u8; 32],
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub text_section_name: String,
    pub text_section_alignment: u64,
    pub text_bytes: Vec<u8>,
    pub symbols: Vec<OptimizedProgramStorageSemanticWrapperObjectSymbol>,
    pub wrapper_symbol: ObjectLocalSymbolId,
    pub continuation_symbol: ObjectLocalSymbolId,
    pub wrapper_byte_count: u64,
    pub call_resolution: OptimizedProgramStorageSemanticWrapperCallResolution,
    pub relocation_record_count: u64,
}

// Test-only count of plan-identity serializations on the calling thread:
// the staging-route test asserts each produced plan value is sealed once.
// Thread-local so parallel tests in one process cannot interleave counts.
#[cfg(test)]
thread_local! {
    static TEST_PLAN_IDENTITY_RECOMPUTATIONS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

impl OptimizedProgramStorageSemanticWrapperObjectPlan {
    pub fn recomputed_identity(
        &self,
    ) -> Result<
        OptimizedProgramStorageSemanticWrapperObjectIdentity,
        OptimizedProgramStorageSemanticWrapperObjectRecordError,
    > {
        #[cfg(test)]
        TEST_PLAN_IDENTITY_RECOMPUTATIONS.with(|count| count.set(count.get() + 1));
        let mut canonical = PLAN_SCHEMA.to_vec();
        canonical.extend_from_slice(&encode_plan_content(self)?);
        Ok(OptimizedProgramStorageSemanticWrapperObjectIdentity::from_canonical_bytes(&canonical))
    }

    /// The object check for a plan whose `identity` the caller assigned from
    /// `recomputed_identity()` on this unchanged value (the stage's retained,
    /// just-composed plan): the digest conjunct
    /// cannot differ and is not reserialized; every shape and template check
    /// still runs.
    pub fn validate_preserving_seal(
        &self,
        template: &ValidatedX86_64SemanticUnitWrapperTemplate,
    ) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectRecordError> {
        validate_object_preserving_seal(self, template)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperObjectContainer {
    pub identity: OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    pub object: OptimizedProgramStorageSemanticWrapperObjectIdentity,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperObjectStage {
    ValidatedResolvedCompositeObjectV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperObjectUnavailableData {
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperObjectManifest {
    pub identity: OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
    pub stage: OptimizedProgramStorageSemanticWrapperObjectStage,
    pub object: OptimizedProgramStorageSemanticWrapperObjectIdentity,
    pub container: OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    pub source_artifact: OptimizedObjectArtifactIdentity,
    pub source_artifact_manifest: OptimizedObjectArtifactManifestIdentity,
    pub source_object: RelocationFreeObjectPlanIdentity,
    pub source_object_container: RelocationFreeObjectContainerIdentity,
    pub source_signature: [u8; 32],
    pub psi: TerminalPsiIdentity,
    pub target: NativeTarget,
    pub wrapper_symbol: ObjectLocalSymbolId,
    pub continuation_symbol: ObjectLocalSymbolId,
    pub text_byte_count: u64,
    pub symbol_count: u64,
    pub relocation_record_count: u64,
    pub physical_entry_bridge: OptimizedProgramStorageSemanticWrapperObjectUnavailableData,
    pub executable_image: OptimizedProgramStorageSemanticWrapperObjectUnavailableData,
    pub installation: OptimizedProgramStorageSemanticWrapperObjectUnavailableData,
    pub publication: OptimizedProgramStorageSemanticWrapperObjectUnavailableData,
}

impl OptimizedProgramStorageSemanticWrapperObjectManifest {
    pub fn recomputed_identity(
        &self,
    ) -> OptimizedProgramStorageSemanticWrapperObjectManifestIdentity {
        let mut canonical =
            b"omega.optimized-program-storage-semantic-wrapper-object-manifest.v1\0".to_vec();
        encode_manifest_content(&mut canonical, self);
        OptimizedProgramStorageSemanticWrapperObjectManifestIdentity::from_canonical_bytes(
            &canonical,
        )
    }

    pub fn encode(&self) -> Vec<u8> {
        encode_manifest(self)
    }

    pub fn decode(
        bytes: &[u8],
    ) -> Result<Self, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        decode_manifest(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest {
    record: OptimizedProgramStorageSemanticWrapperObjectManifest,
}

impl ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest {
    /// Derives the manifest of one plan/container pair and checks its closed
    /// shape; the only route to a validated manifest.
    pub fn construct(
        object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
        container: &OptimizedProgramStorageSemanticWrapperObjectContainer,
    ) -> Result<Self, OptimizedProgramStorageSemanticWrapperObjectRecordError> {
        Ok(Self {
            record: construct_manifest(object, container)?,
        })
    }

    /// Replays the retained manifest against an independently recomputed
    /// plan/container pair through its wire round trip.
    pub fn replay(
        &self,
        object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
        container: &OptimizedProgramStorageSemanticWrapperObjectContainer,
    ) -> Result<(), OptimizedProgramStorageSemanticWrapperObjectRecordError> {
        validate_manifest(object, container, &self.record)
    }

    pub const fn record(&self) -> &OptimizedProgramStorageSemanticWrapperObjectManifest {
        &self.record
    }

    #[cfg(test)]
    fn record_mut(&mut self) -> &mut OptimizedProgramStorageSemanticWrapperObjectManifest {
        &mut self.record
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt {
    pub(super) source_artifact: OptimizedObjectArtifactIdentity,
    pub(super) source_signature: [u8; 32],
    pub(super) object: OptimizedProgramStorageSemanticWrapperObjectIdentity,
    pub(super) container: OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    pub(super) manifest: OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
}

impl OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt {
    pub const fn from_records(
        object: &OptimizedProgramStorageSemanticWrapperObjectPlan,
        container: &OptimizedProgramStorageSemanticWrapperObjectContainer,
        manifest: &OptimizedProgramStorageSemanticWrapperObjectManifest,
    ) -> Self {
        Self {
            source_artifact: object.source_artifact,
            source_signature: object.source_signature,
            object: object.identity,
            container: container.identity,
            manifest: manifest.identity,
        }
    }

    pub const fn source_artifact(self) -> OptimizedObjectArtifactIdentity {
        self.source_artifact
    }

    pub const fn source_signature(self) -> [u8; 32] {
        self.source_signature
    }

    pub const fn object(self) -> OptimizedProgramStorageSemanticWrapperObjectIdentity {
        self.object
    }

    pub const fn container(self) -> OptimizedProgramStorageSemanticWrapperObjectContainerIdentity {
        self.container
    }

    pub const fn manifest(self) -> OptimizedProgramStorageSemanticWrapperObjectManifestIdentity {
        self.manifest
    }
}

// Wire identity of the wrapper object, its container, and its manifest.
const PLAN_SCHEMA: &[u8] = b"omega.optimized-program-storage-semantic-wrapper-object.v1\0";
const CONTAINER_MAGIC: &[u8; 8] = b"OMGPSO\0\0";
const MANIFEST_MAGIC: &[u8; 8] = b"OMGPSM\0\0";
const CODEC_VERSION: u32 = 1;
const WRAPPER_SYMBOL_NAME: &str = "__omega_program_entry_plan_semantic_wrapper_v1";

/// Failures raised by the wrapper object's own record operations. The owning
/// native-realization stage maps each onto its same-named stage variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperObjectRecordError {
    LengthOverflow,
    InvalidObject,
    ManifestMismatch,
    SourceObjectMismatch,
    WrapperResolution(X86_64SemanticUnitWrapperResolutionError),
}

impl std::fmt::Display for OptimizedProgramStorageSemanticWrapperObjectRecordError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized ProgramStorage semantic wrapper object record failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedProgramStorageSemanticWrapperObjectRecordError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizedProgramStorageSemanticWrapperObjectDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidUtf8,
    InvalidLength,
    InvalidSymbol,
    InvalidMachine,
    InvalidVocabulary,
    InvalidTarget,
    UnknownTag,
    IdentityMismatch,
    InvalidObject,
    TrailingBytes,
}

impl std::fmt::Display for OptimizedProgramStorageSemanticWrapperObjectDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid optimized ProgramStorage wrapper object encoding: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedProgramStorageSemanticWrapperObjectDecodeError {}

#[cfg(test)]
mod tests;
