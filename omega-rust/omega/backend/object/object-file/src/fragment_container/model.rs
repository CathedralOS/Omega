//! Source admission, shared current object data, and publication custody.
use super::*;
use machine_emission::RelocationFreeTextSectionPlacementError;
use std::sync::Arc;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionFragmentObjectContainerManifest {
    pub(super) record: Arc<FunctionFragmentObjectContainerManifest>,
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
    pub(super) source: StagedOptimizedObjectTextSectionSource,
    pub(super) object: Arc<RelocationFreeObjectPlan>,
    pub(super) container: Arc<RelocationFreeObjectContainer>,
    pub(super) manifest: ValidatedFunctionFragmentObjectContainerManifest,
    pub(super) custody: StagedRelocationFreeObjectContainerCustodyReceipt,
}

impl StagedOptimizedRelocationFreeObjectContainer {
    pub const fn source(&self) -> &StagedOptimizedObjectTextSectionSource {
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
        self.source.source().verified_input()
    }

    pub fn provider_installation(
        &self,
    ) -> Option<&terminal_psi_to_abstract_operations::AdmittedProviderInstallation> {
        self.source.source().provider_installation()
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
    pub(super) source_text_section_manifest: FunctionFragmentTextSectionManifestIdentity,
    pub(super) text_section: TerminalRelocationFreeTextSectionIdentity,
    pub(super) object: RelocationFreeObjectPlanIdentity,
    pub(super) object_container: RelocationFreeObjectContainerIdentity,
    pub(super) manifest: FunctionFragmentObjectContainerManifestIdentity,
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
