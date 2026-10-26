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
            Self::Unavailable => None,
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
pub use backing::reference_backing_places;
mod captures;
mod descriptor_suppliers;
mod reference_captures;
pub use captures::{
    captured_operand_child_at_path, captured_retention_profiles_at_path, operand_child_type,
    project_captured_view_place,
};
pub use descriptor_suppliers::descriptor_suppliers;
pub use reference_captures::reference_captures_at_path;
mod descriptor_extents;
pub use descriptor_extents::descriptor_extent;

mod predecessors;
pub use predecessors::{
    predecessors, predecessors_in_control, predecessors_in_flow, terminal_alternatives,
};
mod memberships;
pub use memberships::completed_membership;

mod occurrences;
pub use occurrences::consumer_captures;
