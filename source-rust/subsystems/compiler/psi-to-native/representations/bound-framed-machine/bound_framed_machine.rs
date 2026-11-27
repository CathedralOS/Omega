//! Stage 09's installation binding binds the selected frame and installation resources
//! before stage 11 encodes them.
#![forbid(unsafe_code)]

pub mod entries;

use installation_binding_issuer::BindingIssuer;
use machine_code::layout::BoundEncodingRequirements;
use physical_recipes::frame_protocol::PlannedFunctionBody;

/// The checked frame, its selected entry recipes and the obligations encoding must discharge.
/// The publication stays opaque while the encoder checks the resulting bytes against the subject.
pub struct BoundFunctionBody<FrameOrigin, ImportOrigin, Installations, Publication> {
    program: PlannedFunctionBody<FrameOrigin, ImportOrigin>,
    installations: std::sync::Arc<Installations>,
    requirements: BoundEncodingRequirements<Publication>,
}

impl<FrameOrigin, ImportOrigin, Installations, Publication>
    BoundFunctionBody<FrameOrigin, ImportOrigin, Installations, Publication>
{
    pub fn from_binding(
        _issuer: BindingIssuer,
        program: PlannedFunctionBody<FrameOrigin, ImportOrigin>,
        installations: std::sync::Arc<Installations>,
        requirements: BoundEncodingRequirements<Publication>,
    ) -> Self {
        Self {
            program,
            installations,
            requirements,
        }
    }

    pub fn program(&self) -> &PlannedFunctionBody<FrameOrigin, ImportOrigin> {
        &self.program
    }

    pub fn installations(&self) -> &Installations {
        &self.installations
    }

    pub fn into_encoding_parts(
        self,
    ) -> (
        PlannedFunctionBody<FrameOrigin, ImportOrigin>,
        BoundEncodingRequirements<Publication>,
    ) {
        (self.program, self.requirements)
    }
}
