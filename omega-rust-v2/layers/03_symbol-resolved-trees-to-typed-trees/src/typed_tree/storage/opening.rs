use crate::Site;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum Projection {
    Field(Box<omega_resolver::DeclarationId>),
    Tuple(usize),
    Index(omega_resolver::TreeNodeId),
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
    pub base: omega_resolver::TreeNodeId,
    pub through: Option<crate::Access>,
    pub projection: Option<Projection>,
}
