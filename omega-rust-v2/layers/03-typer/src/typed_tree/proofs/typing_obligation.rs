use crate::{ObligationKind, Site};
/// A typed formation condition whose proof belongs to a later checking stage.
#[derive(Debug, Eq, PartialEq)]
pub struct TypingObligation {
    pub site: Site,
    pub kind: ObligationKind,
}
