use crate::Site;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum Projection {
    Referent,
    Field(Box<omega_resolved::DeclarationId>),
    Tuple(usize),
    Index(omega_resolved::TreeNodeId),
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct Opening {
    pub subject: Site,
    pub fields: Vec<Projection>,
    pub version: Option<Site>,
}

/// One step from an evaluated subject to a selected place. An alias uses its
/// captured base directly. A projected field or tuple component can follow a loan.
#[derive(Debug, Eq, PartialEq)]
pub struct PlaceStep {
    pub base: omega_resolved::TreeNodeId,
    pub through: Option<crate::Access>,
    pub projection: Option<Projection>,
}

impl PlaceStep {
    /// Both `r.field` and `($r).field` cross a loan before selecting the field.
    pub fn projections(&self) -> impl Iterator<Item = &Projection> {
        self.through
            .as_ref()
            .map(|_| &Projection::Referent)
            .into_iter()
            .chain(&self.projection)
    }
}
