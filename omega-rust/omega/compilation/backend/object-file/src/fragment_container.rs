/*
We stage and validate the optimizer's relocation-free object container
(`StagedOptimizedRelocationFreeObjectContainer`), binding the placed fixed-frame
text section, the constructed `RelocationFreeObjectPlan`, the encoded `OMGTRO`
wire container, the `OMGTOM` publication manifest, and the custody receipt into
one verified package. During normal compilation we trust the freshly constructed
text section and encoded container without re-decoding; independent validation
(`validate_optimized_relocation_free_object_container`) replays the full source,
object, wire-decode, manifest, and receipt correspondence for retained artifacts
and tests.
*/

use crate::{
    FunctionFragmentObjectContainerManifest, FunctionFragmentObjectContainerStage,
    FunctionFragmentObjectContainerUnavailableData, RelocationFreeObjectContainer,
    RelocationFreeObjectDecodeError, RelocationFreeObjectError, RelocationFreeObjectFromTextError,
    RelocationFreeObjectPlan, construct_relocation_free_object_from_text,
    decode_relocation_free_object, encode_relocation_free_object,
    validate_relocation_free_object_from_text,
};
use machine_emission::{
    RelocationFreeTextSectionPlacementError, StagedOptimizedFixedFrameTextSection,
    validate_optimized_fixed_frame_text_section,
};
use optimization_core::{
    FunctionFragmentObjectContainerManifestIdentity, FunctionFragmentTextSectionManifestIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
    TerminalRelocationFreeTextSectionIdentity,
};
use std::sync::Arc;

pub fn stage_optimized_relocation_free_object_container(
    source: StagedOptimizedFixedFrameTextSection,
) -> Result<StagedOptimizedRelocationFreeObjectContainer, RelocationFreeObjectContainerError> {
    let object = construct_relocation_free_object_from_text(
        source.text_section(),
        source.manifest().record().selections,
    )
    .map_err(object_error)?;
    let container = encode_relocation_free_object(&object)
        .map_err(RelocationFreeObjectContainerError::InvalidObject)?;
    let manifest = construct_manifest(&source, &object, &container)?;
    let custody = receipt(&manifest, &object, &container);
    Ok(StagedOptimizedRelocationFreeObjectContainer {
        source,
        object: Arc::new(object),
        container: Arc::new(container),
        manifest,
        custody,
    })
}

pub fn validate_optimized_relocation_free_object_container(
    staged: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<StagedRelocationFreeObjectContainerCustodyReceipt, RelocationFreeObjectContainerError> {
    validate_optimized_fixed_frame_text_section(&staged.source)
        .map_err(RelocationFreeObjectContainerError::Source)?;
    validate_relocation_free_object_from_text(
        staged.source.text_section(),
        staged.source.manifest().record().selections,
        &staged.object,
    )
    .map_err(object_error)?;
    if staged.container.object != staged.object.identity
        || staged.container.identity
            != RelocationFreeObjectContainerIdentity::from_canonical_bytes(&staged.container.bytes)
    {
        return Err(RelocationFreeObjectContainerError::ContainerMismatch);
    }
    let decoded = decode_relocation_free_object(&staged.container.bytes)
        .map_err(RelocationFreeObjectContainerError::InvalidContainer)?;
    if decoded != *staged.object {
        return Err(RelocationFreeObjectContainerError::ContainerMismatch);
    }
    validate_manifest(staged)?;
    let expected = receipt(&staged.manifest, &staged.object, &staged.container);
    if staged.custody != expected {
        return Err(RelocationFreeObjectContainerError::ReceiptMismatch);
    }
    Ok(expected)
}

fn validate_manifest(
    staged: &StagedOptimizedRelocationFreeObjectContainer,
) -> Result<(), RelocationFreeObjectContainerError> {
    let expected = construct_manifest(&staged.source, staged.object(), staged.container())?;
    if staged.manifest.record() == expected.record() {
        Ok(())
    } else {
        Err(RelocationFreeObjectContainerError::ManifestMismatch)
    }
}

fn construct_manifest(
    source: &StagedOptimizedFixedFrameTextSection,
    object: &RelocationFreeObjectPlan,
    container: &RelocationFreeObjectContainer,
) -> Result<ValidatedFunctionFragmentObjectContainerManifest, RelocationFreeObjectContainerError> {
    let unavailable = FunctionFragmentObjectContainerUnavailableData::Unavailable;
    let statistics = crate::relocation_free_object_statistics(object, container)
        .map_err(|_| RelocationFreeObjectContainerError::LengthOverflow)?;
    let mut record = FunctionFragmentObjectContainerManifest {
        identity: FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(b"pending"),
        stage: FunctionFragmentObjectContainerStage::ValidatedRelocationFreeObjectContainerV1,
        source_text_section_manifest: source.manifest().record().identity,
        text_section: source.text_section().identity,
        psi: object.psi,
        work_schedule: object.work_schedule,
        selections: object.selections,
        selected: object.selected,
        target: object.target,
        semantic_entry: object.semantic_entry,
        semantic_entry_symbol: object.semantic_entry_symbol,
        symbol_policy: object.symbol_policy,
        object: object.identity,
        object_container: container.identity,
        relocation_requirements: object.relocation_requirements,
        statistics,
        external_entry_bridge: unavailable,
        executable_image: unavailable,
        installation: unavailable,
        publication: unavailable,
    };
    record.identity = record.recomputed_identity();
    Ok(ValidatedFunctionFragmentObjectContainerManifest {
        record: Arc::new(record),
    })
}

fn receipt(
    manifest: &ValidatedFunctionFragmentObjectContainerManifest,
    object: &RelocationFreeObjectPlan,
    container: &RelocationFreeObjectContainer,
) -> StagedRelocationFreeObjectContainerCustodyReceipt {
    StagedRelocationFreeObjectContainerCustodyReceipt {
        source_text_section_manifest: manifest.record.source_text_section_manifest,
        text_section: object.source_text_section,
        object: object.identity,
        object_container: container.identity,
        manifest: manifest.record.identity,
    }
}

fn object_error(error: RelocationFreeObjectFromTextError) -> RelocationFreeObjectContainerError {
    match error {
        RelocationFreeObjectFromTextError::InvalidObject(error) => {
            RelocationFreeObjectContainerError::InvalidObject(error)
        }
        RelocationFreeObjectFromTextError::LengthOverflow => {
            RelocationFreeObjectContainerError::LengthOverflow
        }
        RelocationFreeObjectFromTextError::MissingSemanticEntry => {
            RelocationFreeObjectContainerError::MissingSemanticEntry
        }
        RelocationFreeObjectFromTextError::SourceMismatch => {
            RelocationFreeObjectContainerError::ArtifactMismatch
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionFragmentObjectContainerManifest {
    record: Arc<FunctionFragmentObjectContainerManifest>,
}

impl ValidatedFunctionFragmentObjectContainerManifest {
    pub fn record(&self) -> &FunctionFragmentObjectContainerManifest {
        &self.record
    }
}

#[derive(Debug)]
#[must_use = "a staged object container owns its complete text-section custody"]
pub struct StagedOptimizedRelocationFreeObjectContainer {
    source: StagedOptimizedFixedFrameTextSection,
    object: Arc<RelocationFreeObjectPlan>,
    container: Arc<RelocationFreeObjectContainer>,
    manifest: ValidatedFunctionFragmentObjectContainerManifest,
    custody: StagedRelocationFreeObjectContainerCustodyReceipt,
}

impl StagedOptimizedRelocationFreeObjectContainer {
    pub const fn source(&self) -> &StagedOptimizedFixedFrameTextSection {
        &self.source
    }

    pub fn object(&self) -> &RelocationFreeObjectPlan {
        &self.object
    }

    pub fn container(&self) -> &RelocationFreeObjectContainer {
        &self.container
    }

    pub fn shared_object(&self) -> Arc<RelocationFreeObjectPlan> {
        Arc::clone(&self.object)
    }

    pub const fn manifest(&self) -> &ValidatedFunctionFragmentObjectContainerManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> StagedRelocationFreeObjectContainerCustodyReceipt {
        self.custody
    }

    pub fn lowering_facts(&self) -> &verified_optimization_input::AdmittedLoweringFacts {
        self.source.source().source().lowering_facts()
    }

    pub fn provider_installation(
        &self,
    ) -> Option<&provider_admission::AdmittedProviderInstallation> {
        self.source.source().source().provider_installation()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedRelocationFreeObjectContainerCustodyReceipt {
    source_text_section_manifest: FunctionFragmentTextSectionManifestIdentity,
    text_section: TerminalRelocationFreeTextSectionIdentity,
    object: RelocationFreeObjectPlanIdentity,
    object_container: RelocationFreeObjectContainerIdentity,
    manifest: FunctionFragmentObjectContainerManifestIdentity,
}

impl StagedRelocationFreeObjectContainerCustodyReceipt {
    pub const fn source_text_section_manifest(self) -> FunctionFragmentTextSectionManifestIdentity {
        self.source_text_section_manifest
    }

    pub const fn text_section(self) -> TerminalRelocationFreeTextSectionIdentity {
        self.text_section
    }

    pub const fn object(self) -> RelocationFreeObjectPlanIdentity {
        self.object
    }

    pub const fn object_container(self) -> RelocationFreeObjectContainerIdentity {
        self.object_container
    }

    pub const fn manifest(self) -> FunctionFragmentObjectContainerManifestIdentity {
        self.manifest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationFreeObjectContainerError {
    Source(RelocationFreeTextSectionPlacementError),
    InvalidObject(RelocationFreeObjectError),
    InvalidContainer(RelocationFreeObjectDecodeError),
    LengthOverflow,
    MissingSemanticEntry,
    ArtifactMismatch,
    ContainerMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for RelocationFreeObjectContainerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "relocation-free optimizer object custody failed: {self:?}"
        )
    }
}

impl std::error::Error for RelocationFreeObjectContainerError {}
