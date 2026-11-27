use crate::{ContractClause, DeclarationId, Site, TypeId};

/// Who may introduce a domain's qualification on a value that lacks it. A
/// value that already carries the qualification moves and forwards either way.
///
/// `source-rust/subsystems/compiler/psi/artifact/wire/encoding.md` saves this as the domain row's optional
/// establishment routes: `Unrestricted` is an absent list and `Nobody` is a
/// present list with no route. A list that names routes has no value here
/// because no validated access plan can supply one yet.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DomainIssuers {
    /// The domain has no access plan, or its plan selects `Issuance::Unrestricted`.
    #[default]
    Unrestricted,
    /// The domain's validated access plan selects `Issuance::Only` with no route.
    Nobody,
}

/// The predicates apply when a value establishes this particular named domain.
///
/// `I` is what the row states about its issuers. A published row states
/// [`DomainIssuers`]. Lowering uses another `I` for a row whose access plan
/// has not been answered, so that row cannot be read as a decision.
#[derive(Debug, Eq, PartialEq)]
pub struct LoweredDomain<C = TypeId, I = DomainIssuers> {
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
    /// Taken from the domain's validated access plan, never from its policy's
    /// source, its constructor arguments or the presence of a template.
    pub issuers: I,
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
