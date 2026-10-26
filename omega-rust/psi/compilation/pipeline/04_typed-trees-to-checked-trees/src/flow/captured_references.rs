//! Scratch reference geometry keeps the exact descriptor-formation association.
//! Geometry-only consumers may intentionally project `place`; value-flow stores
//! and projections retain the pair, including distinct origins at one address.

use arena::Handle;
use checked_trees::{FlowControlFacts, FlowOperandReferent, FlowReferentFormation};
use facts::PlaceSegment;
use validation::CanonicalPlace;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CapturedReferencePlace {
    pub place: CanonicalPlace,
    pub formation: Handle<FlowReferentFormation>,
}

impl CapturedReferencePlace {
    pub fn unformed(place: CanonicalPlace) -> Self {
        Self {
            place,
            formation: Handle::invalid(),
        }
    }

    pub fn from_referent(
        control: &FlowControlFacts,
        referent: &FlowOperandReferent,
    ) -> Option<Self> {
        Some(Self {
            place: CanonicalPlace {
                root: referent.root,
                segments: control
                    .operand_referent_segments
                    .span(referent.segments)?
                    .to_vec(),
            },
            formation: referent.formation,
        })
    }

    pub fn projected(self, suffix: &[PlaceSegment]) -> Option<Self> {
        Some(Self {
            place: super::project_captured_view_place(self.place, suffix)?,
            formation: self.formation,
        })
    }
}
