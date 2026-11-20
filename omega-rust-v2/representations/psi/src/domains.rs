use crate::{ContractClause, DeclarationId, Site, TypeId};
/// The predicates apply when a value establishes this particular named domain.
#[derive(Debug, Eq, PartialEq)]
pub struct LoweredDomain {
    pub parameters: Vec<crate::TemplateBinder>,
    pub declaration: DeclarationId,
    pub carrier: TypeId,
    pub public: bool,
    pub requires: Vec<ContractClause>,
    pub site: Site,
}

/// A transparent name for the canonical set of required domain identities.
#[derive(Debug, Eq, PartialEq)]
pub struct LoweredDomainAlias {
    pub declaration: DeclarationId,
    pub carrier: TypeId,
    pub members: Vec<DeclarationId>,
    pub public: bool,
    pub site: Site,
}
