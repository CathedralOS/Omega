use super::codec::{
    Cursor, decode_psi, decode_symbol_id, decode_target, encode_manifest_content,
    encode_plan_content,
};
use super::error::{
    OptimizedProgramStorageSemanticWrapperObjectDecodeError,
    OptimizedProgramStorageSemanticWrapperObjectError,
};
use super::object::valid_manifest_shape;
use super::shared::*;

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

impl OptimizedProgramStorageSemanticWrapperObjectPlan {
    pub fn recomputed_identity(
        &self,
    ) -> Result<
        OptimizedProgramStorageSemanticWrapperObjectIdentity,
        OptimizedProgramStorageSemanticWrapperObjectError,
    > {
        let mut canonical = PLAN_SCHEMA.to_vec();
        canonical.extend_from_slice(&encode_plan_content(self)?);
        Ok(OptimizedProgramStorageSemanticWrapperObjectIdentity::from_canonical_bytes(&canonical))
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
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MANIFEST_MAGIC);
        bytes.extend_from_slice(&CODEC_VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.identity.bytes());
        encode_manifest_content(&mut bytes, self);
        bytes
    }

    pub fn decode(
        bytes: &[u8],
    ) -> Result<Self, OptimizedProgramStorageSemanticWrapperObjectDecodeError> {
        let mut cursor = Cursor::new(bytes);
        if cursor.take(8)? != MANIFEST_MAGIC {
            return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != CODEC_VERSION {
            return Err(
                OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnsupportedVersion(
                    version,
                ),
            );
        }
        let identity = OptimizedProgramStorageSemanticWrapperObjectManifestIdentity::from_bytes(
            cursor.array()?,
        );
        if cursor.byte()? != 1 {
            return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag);
        }
        let object =
            OptimizedProgramStorageSemanticWrapperObjectIdentity::from_bytes(cursor.array()?);
        let container = OptimizedProgramStorageSemanticWrapperObjectContainerIdentity::from_bytes(
            cursor.array()?,
        );
        let source_artifact = OptimizedObjectArtifactIdentity::from_bytes(cursor.array()?);
        let source_artifact_manifest =
            OptimizedObjectArtifactManifestIdentity::from_bytes(cursor.array()?);
        let source_object = RelocationFreeObjectPlanIdentity::from_bytes(cursor.array()?);
        let source_object_container =
            RelocationFreeObjectContainerIdentity::from_bytes(cursor.array()?);
        let source_signature = cursor.array()?;
        let psi = decode_psi(&mut cursor)?;
        let target = decode_target(&mut cursor)?;
        let wrapper_symbol = decode_symbol_id(&mut cursor)?;
        let continuation_symbol = decode_symbol_id(&mut cursor)?;
        let text_byte_count = u64::from_le_bytes(cursor.array()?);
        let symbol_count = u64::from_le_bytes(cursor.array()?);
        let relocation_record_count = u64::from_le_bytes(cursor.array()?);
        for _ in 0..4 {
            if cursor.byte()? != 1 {
                return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::UnknownTag);
            }
        }
        if cursor.remaining() != 0 {
            return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::TrailingBytes);
        }
        let unavailable = OptimizedProgramStorageSemanticWrapperObjectUnavailableData::Unavailable;
        let manifest = Self {
            identity,
            stage: OptimizedProgramStorageSemanticWrapperObjectStage::ValidatedResolvedCompositeObjectV1,
            object,
            container,
            source_artifact,
            source_artifact_manifest,
            source_object,
            source_object_container,
            source_signature,
            psi,
            target,
            wrapper_symbol,
            continuation_symbol,
            text_byte_count,
            symbol_count,
            relocation_record_count,
            physical_entry_bridge: unavailable,
            executable_image: unavailable,
            installation: unavailable,
            publication: unavailable,
        };
        if !valid_manifest_shape(&manifest) || manifest.recomputed_identity() != identity {
            return Err(OptimizedProgramStorageSemanticWrapperObjectDecodeError::IdentityMismatch);
        }
        Ok(manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest {
    pub(crate) record: OptimizedProgramStorageSemanticWrapperObjectManifest,
}

impl ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest {
    pub const fn record(&self) -> &OptimizedProgramStorageSemanticWrapperObjectManifest {
        &self.record
    }

    #[cfg(test)]
    pub(crate) fn record_mut(
        &mut self,
    ) -> &mut OptimizedProgramStorageSemanticWrapperObjectManifest {
        &mut self.record
    }
}

#[derive(Debug)]
#[must_use = "semantic-wrapper object custody retains settlement, encoding, and Omega object sources"]
pub struct StagedValidatedOptimizedProgramStorageSemanticWrapperObject {
    pub(crate) settlement: ValidatedNativeProgramEntrySettlement,
    pub(crate) source: StagedValidatedOptimizedObjectArtifact,
    pub(crate) encoding: StagedOptimizedProgramStorageSemanticWrapperEncoding,
    pub(crate) object: OptimizedProgramStorageSemanticWrapperObjectPlan,
    pub(crate) container: OptimizedProgramStorageSemanticWrapperObjectContainer,
    pub(crate) manifest: ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest,
    pub(crate) custody: OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt,
}

impl StagedValidatedOptimizedProgramStorageSemanticWrapperObject {
    pub const fn settlement(&self) -> &ValidatedNativeProgramEntrySettlement {
        &self.settlement
    }

    pub const fn source(&self) -> &StagedValidatedOptimizedObjectArtifact {
        &self.source
    }

    pub const fn encoding(&self) -> &StagedOptimizedProgramStorageSemanticWrapperEncoding {
        &self.encoding
    }

    pub const fn object(&self) -> &OptimizedProgramStorageSemanticWrapperObjectPlan {
        &self.object
    }

    pub const fn container(&self) -> &OptimizedProgramStorageSemanticWrapperObjectContainer {
        &self.container
    }

    pub const fn manifest(&self) -> &ValidatedOptimizedProgramStorageSemanticWrapperObjectManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt {
        self.custody
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt {
    pub(crate) source_artifact: OptimizedObjectArtifactIdentity,
    pub(crate) source_signature: [u8; 32],
    pub(crate) object: OptimizedProgramStorageSemanticWrapperObjectIdentity,
    pub(crate) container: OptimizedProgramStorageSemanticWrapperObjectContainerIdentity,
    pub(crate) manifest: OptimizedProgramStorageSemanticWrapperObjectManifestIdentity,
}

impl OptimizedProgramStorageSemanticWrapperObjectCustodyReceipt {
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
