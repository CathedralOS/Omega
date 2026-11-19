//! Source supply, ranking and termination contracts carried by resolved declarations.

use semantic_vocabulary::RankingViewId;
use semantic_vocabulary::{ExternalBindingId, ExternalBindingMechanism, SemanticDomainId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, record_storage::Record)]
pub enum MachineSupplyMode {
    #[default]
    CheckedBody,
    Requirement,
    TopLevelRequirement,
    Boundary,
    AdmissionClaim,
    ExternalRealization {
        binding: Option<ExternalBindingId>,
        mechanism: Option<ExternalBindingMechanism>,
    },
    /// A bodyless declaration supplied by a closed compiler catalog.
    /// Consumers still authenticate its exact custody and complete schema;
    /// this source role does not assert a checked body or authorize supply.
    CompilerProvided,
}

impl MachineSupplyMode {
    pub const fn is_boundary_declaration(self) -> bool {
        matches!(
            self,
            Self::TopLevelRequirement | Self::Boundary | Self::AdmissionClaim
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, record_storage::Record)]
pub enum TerminationGuarantee {
    #[default]
    NoGuarantee,
    Terminates {
        premises: Vec<ProgressPremise>,
    },
}

impl TerminationGuarantee {
    pub const fn promises_termination(&self) -> bool {
        matches!(self, Self::Terminates { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct ProgressPremise {
    pub profile: SemanticDomainId,
    pub subject: ProgressSubject,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct ProgressSubject {
    pub root: symbols::SymbolHandle,
    pub projections: Vec<symbols::SymbolHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, record_storage::Record)]
pub struct RankingWitness {
    pub subjects: Vec<String>,
    pub ranking_view: RankingViewId,
    pub view_path: String,
    pub view_arguments: Vec<String>,
    pub rank_range: Option<RankRange>,
}

/// A ranking use retains what name resolution selected. A numeric view
/// tag alone is normalization metadata, never declaration authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, record_storage::Record)]
pub enum RankingViewSelection {
    #[default]
    Unresolved,
    Declaration(symbols::SymbolHandle),
    /// A closed view with no authored declaration, selected only after
    /// ordinary declaration lookup found no shadowing declaration.
    Builtin(RankingViewId),
    /// The subject's typed carrier selected its stable default.
    CanonicalDefault(RankingViewId),
}

#[derive(Debug, Clone, PartialEq, Eq, Default, record_storage::Record)]
pub struct RankRange {
    pub floor: String,
    pub ceiling: String,
    pub ceiling_inclusive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, record_storage::Record)]
pub enum TerminationInterface {
    #[default]
    InternalDerived,
    Published(TerminationGuarantee),
}

impl TerminationInterface {
    pub fn published(&self) -> Option<&TerminationGuarantee> {
        match self {
            Self::InternalDerived => None,
            Self::Published(guarantee) => Some(guarantee),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, record_storage::Record)]
pub struct MachineTerminationPlan {
    pub interface: TerminationInterface,
    pub checked_summary: TerminationGuarantee,
    pub implementation_witness: Option<RankingWitness>,
}
