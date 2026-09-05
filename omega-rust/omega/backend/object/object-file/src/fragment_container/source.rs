use crate::RelocationFreeTextSectionPlacement;

use crate::RelocationFreeObjectContainerError;
use machine_emission::{
    StagedOptimizedFixedFrameTextSection, StagedOptimizedFunctionFragmentEmission,
    StagedOptimizedRelocationFreeTextSection, ValidatedFunctionFragmentTextSectionManifest,
    validate_optimized_fixed_frame_text_section, validate_optimized_relocation_free_text_section,
};

/// Closed text-stage source role accepted by the relocation-free object backend.
///
/// Both variants retain their complete authenticated source. The text manifest
/// identity already commits role-specific custody, including an exact
/// fixed-frame application identity, so generic artifact layers bind that child
/// identity rather than copying its fields.
#[derive(Debug)]
pub enum StagedOptimizedObjectTextSectionSource {
    Direct(StagedOptimizedRelocationFreeTextSection),
    FixedFrame(StagedOptimizedFixedFrameTextSection),
}

impl StagedOptimizedObjectTextSectionSource {
    pub(super) fn validate(&self) -> Result<(), RelocationFreeObjectContainerError> {
        match self {
            Self::Direct(source) => {
                validate_optimized_relocation_free_text_section(source).map(|_| ())
            }
            Self::FixedFrame(source) => {
                validate_optimized_fixed_frame_text_section(source).map(|_| ())
            }
        }
        .map_err(RelocationFreeObjectContainerError::Source)?;
        Ok(())
    }

    pub fn source(&self) -> &StagedOptimizedFunctionFragmentEmission {
        match self {
            Self::Direct(source) => source.source(),
            Self::FixedFrame(source) => source.source().source(),
        }
    }

    pub fn text_section(&self) -> &RelocationFreeTextSectionPlacement {
        match self {
            Self::Direct(source) => source.text_section(),
            Self::FixedFrame(source) => source.text_section(),
        }
    }

    pub const fn manifest(&self) -> &ValidatedFunctionFragmentTextSectionManifest {
        match self {
            Self::Direct(source) => source.manifest(),
            Self::FixedFrame(source) => source.manifest(),
        }
    }
}

impl From<StagedOptimizedRelocationFreeTextSection> for StagedOptimizedObjectTextSectionSource {
    fn from(source: StagedOptimizedRelocationFreeTextSection) -> Self {
        Self::Direct(source)
    }
}

impl From<StagedOptimizedFixedFrameTextSection> for StagedOptimizedObjectTextSectionSource {
    fn from(source: StagedOptimizedFixedFrameTextSection) -> Self {
        Self::FixedFrame(source)
    }
}
