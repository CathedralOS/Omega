use crate::{ContractClause, DeclarationId, Site, TypeId};
/// The predicates apply when a value establishes this particular named domain.
#[derive(Debug, Eq, PartialEq)]
pub struct LoweredDomain<C = TypeId> {
    pub parameters: Vec<crate::TemplateBinder>,
    /// Declaration binder slot supplied by each application argument, in head order.
    pub index_binders: Vec<usize>,
    pub declaration: DeclarationId,
    /// Subject of `requires`: a concrete type or an open carrier recipe scoped by this declaration.
    pub carrier: C,
    /// Selected cleanup of an owned occurrence carrying this domain.
    pub cleanup_target: Option<crate::MachineId>,
    pub public: bool,
    pub requires: Vec<ContractClause>,
    pub site: Site,
}

/// The selected policy definition available for checking before its domain can
/// acquire a validated plan. The conformance identifies the member implementation.
/// This product contains an open machine template and grants no issuance rights.
#[derive(Debug, Eq, PartialEq)]
pub struct SelectedPolicyDefinition {
    pub domain: DeclarationId,
    pub conformance: crate::LoweredConformance,
    pub arguments: Vec<crate::DeclarationTerm>,
    pub plan: crate::MachineTemplate,
    pub site: Site,
}

/// One normalized constituent, including its ordered type/const arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainAtom {
    pub declaration: DeclarationId,
    pub arguments: Vec<crate::DeclarationTerm>,
}

/// A transparent name for the canonical conjunction of domain applications.
#[derive(Debug, Eq, PartialEq)]
pub struct LoweredDomainAlias {
    pub declaration: DeclarationId,
    pub carrier: TypeId,
    pub parameters: Vec<crate::TemplateBinder>,
    /// Declaration binder slot supplied by each application argument, in head order.
    pub index_binders: Vec<usize>,
    pub members: Vec<DomainAtom>,
    pub public: bool,
    pub site: Site,
}
