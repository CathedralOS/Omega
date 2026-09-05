use omega_machine_code::RelocationFreeTextSectionPlacement;
use omega_optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity, TerminalRelocationFreeTextSectionIdentity,
};
use std::sync::Arc;

use crate::{StagedFunctionFragmentFrameApplication, StagedOptimizedFunctionFragmentEmission};
use omega_machine_code::FunctionFragmentFrameApplicationIdentity;

use omega_machine_code::FunctionFragmentTextSectionManifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionFragmentTextSectionManifest {
    pub(super) record: Arc<FunctionFragmentTextSectionManifest>,
}

impl ValidatedFunctionFragmentTextSectionManifest {
    pub fn record(&self) -> &FunctionFragmentTextSectionManifest {
        &self.record
    }

    /// Retain the exact current claim without its admission capsule.
    pub fn shared_record(&self) -> Arc<FunctionFragmentTextSectionManifest> {
        Arc::clone(&self.record)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn record_mut(&mut self) -> &mut FunctionFragmentTextSectionManifest {
        Arc::make_mut(&mut self.record)
    }
}

#[derive(Debug)]
#[must_use = "a staged text section owns its complete fragment-emission custody"]
pub struct StagedOptimizedRelocationFreeTextSection {
    pub(super) source: StagedOptimizedFunctionFragmentEmission,
    pub(super) text_section: Arc<RelocationFreeTextSectionPlacement>,
    pub(super) manifest: ValidatedFunctionFragmentTextSectionManifest,
    pub(super) custody: StagedRelocationFreeTextSectionCustodyReceipt,
}

impl StagedOptimizedRelocationFreeTextSection {
    pub const fn source(&self) -> &StagedOptimizedFunctionFragmentEmission {
        &self.source
    }

    pub fn text_section(&self) -> &RelocationFreeTextSectionPlacement {
        self.text_section.as_ref()
    }

    /// Retain current placed data without retaining the producer or granting admission.
    pub fn shared_text_section(&self) -> Arc<RelocationFreeTextSectionPlacement> {
        Arc::clone(&self.text_section)
    }

    pub const fn manifest(&self) -> &ValidatedFunctionFragmentTextSectionManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> StagedRelocationFreeTextSectionCustodyReceipt {
        self.custody
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn text_section_mut(&mut self) -> &mut RelocationFreeTextSectionPlacement {
        Arc::make_mut(&mut self.text_section)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn manifest_mut(&mut self) -> &mut ValidatedFunctionFragmentTextSectionManifest {
        &mut self.manifest
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_source_fragment_manifest_for_test(&mut self) {
        self.custody.source_fragment_manifest =
            FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"corrupt");
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_fragments_for_test(&mut self) {
        self.custody.fragments = FunctionFragmentEmissionIdentity::from_canonical_bytes(b"corrupt");
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_text_section_for_test(&mut self) {
        self.custody.text_section =
            TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"corrupt");
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_manifest_for_test(&mut self) {
        self.custody.manifest =
            FunctionFragmentTextSectionManifestIdentity::from_canonical_bytes(b"corrupt");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedRelocationFreeTextSectionCustodyReceipt {
    pub(super) source_fragment_manifest: FunctionFragmentEmissionManifestIdentity,
    pub(super) fragments: FunctionFragmentEmissionIdentity,
    pub(super) text_section: TerminalRelocationFreeTextSectionIdentity,
    pub(super) manifest: FunctionFragmentTextSectionManifestIdentity,
}

#[derive(Debug)]
#[must_use = "a staged fixed-frame text section owns its exact frame-application custody"]
pub struct StagedOptimizedFixedFrameTextSection {
    pub(super) source: StagedFunctionFragmentFrameApplication,
    pub(super) text_section: Arc<RelocationFreeTextSectionPlacement>,
    pub(super) manifest: ValidatedFunctionFragmentTextSectionManifest,
    pub(super) custody: StagedFixedFrameTextSectionCustodyReceipt,
}

impl StagedOptimizedFixedFrameTextSection {
    pub const fn source(&self) -> &StagedFunctionFragmentFrameApplication {
        &self.source
    }

    pub fn text_section(&self) -> &RelocationFreeTextSectionPlacement {
        self.text_section.as_ref()
    }

    /// Retain current placed data without retaining the producer or granting admission.
    pub fn shared_text_section(&self) -> Arc<RelocationFreeTextSectionPlacement> {
        Arc::clone(&self.text_section)
    }

    pub const fn manifest(&self) -> &ValidatedFunctionFragmentTextSectionManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> StagedFixedFrameTextSectionCustodyReceipt {
        self.custody
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn text_section_mut(&mut self) -> &mut RelocationFreeTextSectionPlacement {
        Arc::make_mut(&mut self.text_section)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn manifest_mut(&mut self) -> &mut ValidatedFunctionFragmentTextSectionManifest {
        &mut self.manifest
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn corrupt_custody_frame_application_for_test(&mut self) {
        self.custody.frame_application =
            FunctionFragmentFrameApplicationIdentity::from_bytes([0xa5; 32]);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedFixedFrameTextSectionCustodyReceipt {
    pub(super) frame_application: FunctionFragmentFrameApplicationIdentity,
    pub(super) fragments: FunctionFragmentEmissionIdentity,
    pub(super) text_section: TerminalRelocationFreeTextSectionIdentity,
    pub(super) manifest: FunctionFragmentTextSectionManifestIdentity,
}

impl StagedFixedFrameTextSectionCustodyReceipt {
    pub const fn frame_application(self) -> FunctionFragmentFrameApplicationIdentity {
        self.frame_application
    }

    pub const fn fragments(self) -> FunctionFragmentEmissionIdentity {
        self.fragments
    }

    pub const fn text_section(self) -> TerminalRelocationFreeTextSectionIdentity {
        self.text_section
    }

    pub const fn manifest(self) -> FunctionFragmentTextSectionManifestIdentity {
        self.manifest
    }
}

impl StagedRelocationFreeTextSectionCustodyReceipt {
    pub const fn source_fragment_manifest(self) -> FunctionFragmentEmissionManifestIdentity {
        self.source_fragment_manifest
    }

    pub const fn fragments(self) -> FunctionFragmentEmissionIdentity {
        self.fragments
    }

    pub const fn text_section(self) -> TerminalRelocationFreeTextSectionIdentity {
        self.text_section
    }

    pub const fn manifest(self) -> FunctionFragmentTextSectionManifestIdentity {
        self.manifest
    }
}
