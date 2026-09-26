use machine_code::RelocationFreeTextSectionPlacement;
use optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentTextSectionManifestIdentity,
    TerminalRelocationFreeTextSectionIdentity,
};
use std::sync::Arc;

use crate::frame_application::StagedFunctionFragmentFrameApplication;
use machine_code::FunctionFragmentFrameApplicationIdentity;

use machine_code::FunctionFragmentTextSectionManifest;

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
