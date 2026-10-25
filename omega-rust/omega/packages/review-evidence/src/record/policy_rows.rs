//! Complete, inert comparison rows. Classification is not an acceptance decision.

mod limits;
pub use limits::{PackagePolicyRowLimits, PackagePolicyRowUsage};

/// Independent of legacy review rows and of the complete baseline grammar.
pub const PACKAGE_POLICY_ROW_VERSION: u16 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyRowKind {
    Header,
    PublicTrait,
    PublicConformance,
    PublicDomain,
    PublicProposition,
    PublicConst,
    PublicOperator,
    PublicData,
    Callable,
    SelectedProviderAssociation,
    TerminalService,
    TerminalPermission,
    RepresentationTarget,
    RepresentationDeclaration,
    RepresentationAvailability,
    RepresentationSelection,
    RepresentationDemand,
    ExternalSupply,
    DangerousCapability,
    DangerousSlack,
    SemanticDependency,
    SymbolicBoundaryDemand,
    RestrictedBuildRequest,
    AssumedBindingContract,
    ServiceAudit,
}

impl PackagePolicyRowKind {
    /// One row per kind: the persisted tag and the authored spelling, so the
    /// encoded byte and the canonical text cannot disagree about the roster.
    pub const fn tag_and_name(self) -> (u8, &'static str) {
        match self {
            Self::Header => (0, "header"),
            Self::PublicTrait => (1, "public_trait"),
            Self::PublicConformance => (2, "public_conformance"),
            Self::PublicDomain => (3, "public_domain"),
            Self::PublicProposition => (4, "public_proposition"),
            Self::PublicConst => (5, "public_const"),
            Self::PublicOperator => (6, "public_operator"),
            Self::PublicData => (7, "public_data"),
            Self::Callable => (8, "callable"),
            Self::SelectedProviderAssociation => (9, "selected_provider_association"),
            Self::TerminalService => (10, "terminal_service"),
            Self::TerminalPermission => (11, "terminal_permission"),
            Self::RepresentationTarget => (12, "representation_target"),
            Self::RepresentationDeclaration => (13, "representation_declaration"),
            Self::RepresentationAvailability => (14, "representation_availability"),
            Self::RepresentationSelection => (15, "representation_selection"),
            Self::RepresentationDemand => (16, "representation_demand"),
            Self::ExternalSupply => (17, "external_supply"),
            Self::DangerousCapability => (18, "dangerous_capability"),
            Self::DangerousSlack => (19, "dangerous_slack"),
            Self::SemanticDependency => (20, "semantic_dependency"),
            Self::SymbolicBoundaryDemand => (21, "symbolic_boundary_demand"),
            Self::RestrictedBuildRequest => (22, "restricted_build_request"),
            Self::AssumedBindingContract => (23, "assumed_binding_contract"),
            Self::ServiceAudit => (24, "service_audit"),
        }
    }

    pub const fn canonical_tag(self) -> u8 {
        self.tag_and_name().0
    }

    pub const fn as_str(self) -> &'static str {
        self.tag_and_name().1
    }

    pub const fn audit_recommended_on_change(self) -> bool {
        match self {
            Self::RepresentationDeclaration
            | Self::RepresentationAvailability
            | Self::RepresentationSelection
            | Self::RepresentationDemand
            | Self::DangerousSlack
            | Self::ExternalSupply
            | Self::DangerousCapability
            | Self::RestrictedBuildRequest
            | Self::AssumedBindingContract
            | Self::ServiceAudit => true,
            Self::Header
            | Self::RepresentationTarget
            | Self::PublicTrait
            | Self::PublicConformance
            | Self::PublicDomain
            | Self::PublicProposition
            | Self::PublicConst
            | Self::PublicOperator
            | Self::PublicData
            | Self::Callable
            | Self::SelectedProviderAssociation
            | Self::TerminalService
            | Self::TerminalPermission
            | Self::SemanticDependency
            | Self::SymbolicBoundaryDemand => false,
        }
    }

    pub(crate) const fn row_change_audit(self, initial: bool, present: bool) -> bool {
        present || self.audit_recommended_on_change() || (matches!(self, Self::Callable) && initial)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyRow {
    pub(crate) kind: PackagePolicyRowKind,
    pub(crate) key_bytes: Vec<u8>,
    pub(crate) canonical_bytes: Vec<u8>,
    pub(crate) canonical_text: String,
    pub(crate) initial_requires_decision: bool,
    pub(crate) audit_recommended_when_present: bool,
}

impl PackagePolicyRow {
    pub const fn kind(&self) -> PackagePolicyRowKind {
        self.kind
    }
    pub fn key_bytes(&self) -> &[u8] {
        &self.key_bytes
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    pub fn canonical_text(&self) -> &str {
        &self.canonical_text
    }
    pub fn into_canonical_text(self) -> String {
        self.canonical_text
    }
    pub const fn initial_requires_decision(&self) -> bool {
        self.initial_requires_decision
    }
    pub const fn update_requires_decision(&self) -> bool {
        self.initial_requires_decision
    }
    pub const fn audit_recommended_when_present(&self) -> bool {
        self.audit_recommended_when_present
    }
    pub const fn audit_recommended_on_change(&self) -> bool {
        self.kind.row_change_audit(
            self.initial_requires_decision,
            self.audit_recommended_when_present,
        )
    }
}
