use crate::{ObligationKind, Site};
/// A typed formation condition whose proof belongs to a later checking stage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypingObligation {
    /// Node in the forest that produced this obligation.
    pub node: omega_resolver::TreeNodeId,
    pub site: Site,
    pub kind: ObligationKind,
}
