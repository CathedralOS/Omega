use omega_object_file::RelocationFreeTextSectionPlacement;
use omega_optimization_core::{
    FunctionFragmentEmissionIdentity, FunctionFragmentEmissionManifestIdentity,
    FunctionFragmentTextSectionManifestIdentity, TerminalRelocationFreeTextSectionIdentity,
};

use crate::StagedOptimizedFunctionFragmentEmission;

use super::model::FunctionFragmentTextSectionManifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFunctionFragmentTextSectionManifest {
    pub(super) record: FunctionFragmentTextSectionManifest,
}

impl ValidatedFunctionFragmentTextSectionManifest {
    pub const fn record(&self) -> &FunctionFragmentTextSectionManifest {
        &self.record
    }

    #[cfg(test)]
    pub(crate) fn record_mut(&mut self) -> &mut FunctionFragmentTextSectionManifest {
        &mut self.record
    }
}

#[derive(Debug)]
#[must_use = "a staged text section owns its complete fragment-emission custody"]
pub struct StagedOptimizedRelocationFreeTextSection {
    pub(super) source: StagedOptimizedFunctionFragmentEmission,
    pub(super) text_section: Box<RelocationFreeTextSectionPlacement>,
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

    pub const fn manifest(&self) -> &ValidatedFunctionFragmentTextSectionManifest {
        &self.manifest
    }

    pub const fn custody(&self) -> StagedRelocationFreeTextSectionCustodyReceipt {
        self.custody
    }

    #[cfg(test)]
    pub(crate) fn text_section_mut(&mut self) -> &mut RelocationFreeTextSectionPlacement {
        self.text_section.as_mut()
    }

    #[cfg(test)]
    pub(crate) fn manifest_mut(&mut self) -> &mut ValidatedFunctionFragmentTextSectionManifest {
        &mut self.manifest
    }

    #[cfg(test)]
    pub(crate) fn corrupt_custody_source_fragment_manifest_for_test(&mut self) {
        self.custody.source_fragment_manifest =
            FunctionFragmentEmissionManifestIdentity::from_canonical_bytes(b"corrupt");
    }

    #[cfg(test)]
    pub(crate) fn corrupt_custody_fragments_for_test(&mut self) {
        self.custody.fragments = FunctionFragmentEmissionIdentity::from_canonical_bytes(b"corrupt");
    }

    #[cfg(test)]
    pub(crate) fn corrupt_custody_text_section_for_test(&mut self) {
        self.custody.text_section =
            TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"corrupt");
    }

    #[cfg(test)]
    pub(crate) fn corrupt_custody_manifest_for_test(&mut self) {
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
