use crate::ValueType;
use omega_resolved::{DeclarationId, TreeNodeId};

/// The clause node supplies the authored subject, view arguments and bounds.
/// Typing adds the selected view and the carrier its ranking theory produces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankingWitness {
    pub node: TreeNodeId,
    pub machine: DeclarationId,
    pub view: DeclarationId,
    pub rank: ValueType,
}
