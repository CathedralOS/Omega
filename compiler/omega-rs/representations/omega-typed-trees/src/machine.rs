use crate::expression::ExpressionHandle;
use crate::name::Identifier;
use crate::signature::SignatureContract;
use crate::state::State;
use crate::types::TypeReferenceHandle;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub attached_data: Option<Identifier>,
    pub boundary: bool,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub contains: HandleSpan<ContainedObject>,
    pub owned_data: HandleSpan<OwnedData>,
    pub satisfies: HandleSpan<TraitConformance>,
    pub termination_guarantee: omega_core::termination::TerminationGuarantee,
    pub ranking_witness: RankingWitness,
    pub effects: HandleSpan<Identifier>,
    pub contracts: HandleSpan<SignatureContract>,
    pub states: HandleSpan<State>,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            attached_data: None,
            boundary: false,
            type_parameters: HandleSpan::empty(),
            contains: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            termination_guarantee: omega_core::termination::TerminationGuarantee::None,
            ranking_witness: RankingWitness::default(),
            effects: HandleSpan::empty(),
            contracts: HandleSpan::empty(),
            states: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RankingWitness {
    pub subjects: HandleSpan<ExpressionHandle>,
    /// Always explicit after canonical-default elaboration in the checked-tree
    /// entry pipeline. User measures are never selected implicitly.
    pub view: HandleSpan<Identifier>,
    pub view_arguments: HandleSpan<ExpressionHandle>,
    pub range: RankingRange,
}

impl RankingWitness {
    pub fn is_present(self) -> bool {
        !self.subjects.is_empty()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RankingRange {
    pub start: ExpressionHandle,
    pub end: ExpressionHandle,
    pub end_inclusive: bool,
}

impl RankingRange {
    pub fn is_present(self) -> bool {
        self.start.is_valid() && self.end.is_valid()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainedObject {
    pub symbol: SymbolHandle,
    pub type_symbol: SymbolHandle,
    pub name: Identifier,
    pub type_name: Identifier,
}

impl Default for ContainedObject {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            type_symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            type_name: Identifier::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedData {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
    pub initial_value: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitConformance {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    /// The single-requirement binding (`satisfies Trait::requirement`,
    /// rearrange settle 2026-07-18): `Some` conforms the machine to that one
    /// requirement instead of the whole trait; `alias` names the satisfier
    /// (`as Name`) for plural algebras / signature collisions.
    pub requirement: Option<Identifier>,
    pub alias: Option<Identifier>,
}

impl Default for TraitConformance {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            requirement: None,
            alias: None,
        }
    }
}

impl Default for OwnedData {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            type_reference: TypeReferenceHandle::invalid(),
            initial_value: ExpressionHandle::invalid(),
        }
    }
}
