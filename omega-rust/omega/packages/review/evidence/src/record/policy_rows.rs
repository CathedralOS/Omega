//! Complete, inert comparison rows. Classification is not an acceptance decision.

mod limits;
pub use limits::{PackagePolicyRowLimits, PackagePolicyRowUsage};

/// Independent of legacy review rows and of the complete baseline grammar.
pub const PACKAGE_POLICY_ROW_VERSION: u16 = 1;

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
}

impl PackagePolicyRowKind {
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::Header => 0,
            Self::PublicTrait => 1,
            Self::PublicConformance => 2,
            Self::PublicDomain => 3,
            Self::PublicProposition => 4,
            Self::PublicConst => 5,
            Self::PublicOperator => 6,
            Self::PublicData => 7,
            Self::Callable => 8,
            Self::SelectedProviderAssociation => 9,
            Self::TerminalService => 10,
            Self::TerminalPermission => 11,
            Self::RepresentationTarget => 12,
            Self::RepresentationDeclaration => 13,
            Self::RepresentationAvailability => 14,
            Self::RepresentationSelection => 15,
            Self::RepresentationDemand => 16,
            Self::ExternalSupply => 17,
            Self::DangerousCapability => 18,
            Self::DangerousSlack => 19,
            Self::SemanticDependency => 20,
            Self::SymbolicBoundaryDemand => 21,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::PublicTrait => "public_trait",
            Self::PublicConformance => "public_conformance",
            Self::PublicDomain => "public_domain",
            Self::PublicProposition => "public_proposition",
            Self::PublicConst => "public_const",
            Self::PublicOperator => "public_operator",
            Self::PublicData => "public_data",
            Self::Callable => "callable",
            Self::SelectedProviderAssociation => "selected_provider_association",
            Self::TerminalService => "terminal_service",
            Self::TerminalPermission => "terminal_permission",
            Self::RepresentationTarget => "representation_target",
            Self::RepresentationDeclaration => "representation_declaration",
            Self::RepresentationAvailability => "representation_availability",
            Self::RepresentationSelection => "representation_selection",
            Self::RepresentationDemand => "representation_demand",
            Self::ExternalSupply => "external_supply",
            Self::DangerousCapability => "dangerous_capability",
            Self::DangerousSlack => "dangerous_slack",
            Self::SemanticDependency => "semantic_dependency",
            Self::SymbolicBoundaryDemand => "symbolic_boundary_demand",
        }
    }

    pub const fn update_requires_decision(self) -> bool {
        match self {
            Self::Header
            | Self::RepresentationTarget
            | Self::RepresentationDeclaration
            | Self::RepresentationAvailability
            | Self::RepresentationSelection
            | Self::RepresentationDemand
            | Self::DangerousSlack => false,
            Self::PublicTrait
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
            | Self::ExternalSupply
            | Self::DangerousCapability
            | Self::SemanticDependency
            | Self::SymbolicBoundaryDemand => true,
        }
    }

    pub const fn audit_recommended_on_change(self) -> bool {
        match self {
            Self::RepresentationDeclaration
            | Self::RepresentationAvailability
            | Self::RepresentationSelection
            | Self::RepresentationDemand
            | Self::DangerousSlack
            | Self::ExternalSupply
            | Self::DangerousCapability => true,
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
    pub const fn initial_requires_decision(&self) -> bool {
        self.initial_requires_decision
    }
    pub const fn update_requires_decision(&self) -> bool {
        self.kind.update_requires_decision()
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
