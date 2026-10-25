//! Read the exact completed value installed at an immutable storage cut.
//! Incoming, unavailable and complete predecessor values are distinct. This
//! query establishes neither access nor a new loan, and never replays source
//! initializers. Construction remains with the owning flow producer.
use arena::Handle;
use checked_trees::FlowOperandFact;
mod query;
pub use query::at_snapshot;

#[derive(Clone)]
pub enum StoredOperand {
    /// One unchanged entry descriptor in a complete predecessor frontier.
    /// This names a slot, not its pointee address, formation or permission.
    /// The consumer must authenticate the exact incoming namespace separately.
    Incoming(crate::CanonicalPlace),
    Unavailable,
    Alternatives(Vec<StoredOperand>),
    Value {
        capture: Handle<FlowOperandFact>,
        reference: typed_trees::types::TypeReferenceHandle,
        projection: Vec<facts::PlaceSegment>,
    },
}

#[derive(Clone, Copy)]
pub enum StoredObservation {
    Binding,
    Contents,
}

#[derive(Clone)]
pub struct StoredCapture {
    pub capture: Handle<FlowOperandFact>,
    pub reference: typed_trees::types::TypeReferenceHandle,
    pub projection: Vec<facts::PlaceSegment>,
}

impl StoredOperand {
    pub fn captures(self) -> Option<Vec<StoredCapture>> {
        match self {
            Self::Unavailable | Self::Incoming(_) => None,
            Self::Value {
                capture,
                reference,
                projection,
            } => Some(vec![StoredCapture {
                capture,
                reference,
                projection,
            }]),
            Self::Alternatives(alternatives) => {
                let mut captures = Vec::new();
                for alternative in alternatives {
                    captures.extend(alternative.captures()?);
                }
                (!captures.is_empty()).then_some(captures)
            }
        }
    }
}

mod backing;
pub use backing::{direct_referent_places, reference_backing_places};
mod captures;
mod descriptor_suppliers;
mod reference_captures;
pub use captures::{
    captured_operand_child_at_path, captured_operand_children_at_path,
    captured_retention_profiles_at_path, finite_projection_paths, operand_child_type,
    project_captured_view_place,
};
pub use descriptor_suppliers::descriptor_suppliers;
pub use reference_captures::{reference_captures_at_path, reference_sources_at_path};

mod frozen_references;
pub use frozen_references::{FrozenReferenceSelection, frozen_reference_leaf};
