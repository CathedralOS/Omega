//! Optimizer module role: executable entrance. Object publication and source custody.
//! Object-format construction and independent correspondence live in the backend.
use crate::fragment_container::reconstruction::{construct_manifest, receipt};
use crate::{
    FunctionFragmentObjectContainerManifest, FunctionFragmentObjectContainerStage,
    FunctionFragmentObjectContainerUnavailableData, RelocationFreeObjectContainer,
    RelocationFreeObjectDecodeError, RelocationFreeObjectError, RelocationFreeObjectFromTextError,
    RelocationFreeObjectPlan, construct_relocation_free_object_from_text,
    encode_relocation_free_object,
};
use optimization_core::{
    FunctionFragmentObjectContainerManifestIdentity, FunctionFragmentTextSectionManifestIdentity,
    RelocationFreeObjectContainerIdentity, RelocationFreeObjectPlanIdentity,
    TerminalRelocationFreeTextSectionIdentity,
};
mod reconstruction;
mod validation;

use machine_emission::RelocationFreeTextSectionPlacementError;
use machine_emission::{
    StagedOptimizedFixedFrameTextSection, validate_optimized_fixed_frame_text_section,
};
use std::sync::Arc;
pub use validation::validate_optimized_relocation_free_object_container;

pub fn stage_optimized_relocation_free_object_container(
    source: StagedOptimizedFixedFrameTextSection,
) -> Result<StagedOptimizedRelocationFreeObjectContainer, RelocationFreeObjectContainerError> {
    validate_optimized_fixed_frame_text_section(&source)
        .map_err(RelocationFreeObjectContainerError::Source)?;
    let object = construct_relocation_free_object_from_text(
        source.text_section(),
        source.manifest().record().selections,
    )
    .map_err(object_error)?;
    let container = encode_relocation_free_object(&object)
        .map_err(RelocationFreeObjectContainerError::InvalidObject)?;
    let manifest = construct_manifest(&source, &object, &container)?;
    let custody = receipt(&manifest, &object, &container);
    let staged = StagedOptimizedRelocationFreeObjectContainer {
        source,
        object: std::sync::Arc::new(object),
        container: std::sync::Arc::new(container),
        manifest,
        custody,
    };
    validate_optimized_relocation_free_object_container(&staged)?;
    Ok(staged)
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

    pub fn shared_record(&self) -> Arc<FunctionFragmentObjectContainerManifest> {
        Arc::clone(&self.record)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn record_mut(&mut self) -> &mut FunctionFragmentObjectContainerManifest {
        Arc::make_mut(&mut self.record)
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
    pub fn shared_container(&self) -> Arc<RelocationFreeObjectContainer> {
        Arc::clone(&self.container)
    }

    pub const fn manifest(&self) -> &ValidatedFunctionFragmentObjectContainerManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> StagedRelocationFreeObjectContainerCustodyReceipt {
        self.custody
    }

    pub fn verified_input(
        &self,
    ) -> &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput {
        self.source.source().source().verified_input()
    }

    pub fn provider_installation(
        &self,
    ) -> Option<&terminal_psi_to_abstract_operations::AdmittedProviderInstallation> {
        self.source.source().source().provider_installation()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn object_mut(&mut self) -> &mut RelocationFreeObjectPlan {
        Arc::make_mut(&mut self.object)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn container_mut(&mut self) -> &mut RelocationFreeObjectContainer {
        Arc::make_mut(&mut self.container)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn manifest_mut(&mut self) -> &mut ValidatedFunctionFragmentObjectContainerManifest {
        &mut self.manifest
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_source_text_section_manifest_for_test(&mut self) {
        self.custody.source_text_section_manifest =
            FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(b"corrupt");
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_text_section_for_test(&mut self) {
        self.custody.text_section =
            TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"corrupt");
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_object_for_test(&mut self) {
        self.custody.object = RelocationFreeObjectPlanIdentity::from_canonical_bytes(b"corrupt");
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_object_container_for_test(&mut self) {
        self.custody.object_container =
            RelocationFreeObjectContainerIdentity::from_canonical_bytes(b"corrupt");
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_manifest_for_test(&mut self) {
        self.custody.manifest =
            FunctionFragmentObjectContainerManifestIdentity::from_canonical_bytes(b"corrupt");
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
