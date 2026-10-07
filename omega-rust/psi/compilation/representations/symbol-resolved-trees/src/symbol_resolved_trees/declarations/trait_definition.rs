use crate::name::DiagnosticName;
use crate::signature::StateSignature;
use arena::HandleSpan;
use std::ops::{Deref, DerefMut};
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraitDefinition {
    pub symbol: SymbolHandle,
    pub is_boundary: bool,
    /// Source visibility retained independently from symbol and requirement
    /// identity.
    pub is_public: bool,
    pub name: DiagnosticName,
    pub storage: TraitStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraitStorage {
    pub lifetime_parameters: Vec<DiagnosticName>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub conformance_bounds: Vec<crate::machine::GenericConformanceBound>,
    pub requires: HandleSpan<TraitRequirement>,
    pub machines: HandleSpan<StateSignature>,
    /// The `= Base` head of a transparent refinement. A refinement is a
    /// structural bound over an existing base conformance, never a nominal
    /// conformance target; `None` marks an ordinary trait.
    pub refines: Option<TraitRequirement>,
    /// `machine *` / `machine Base::requirement` narrowing clauses. Each
    /// signature retains only operational axes (`reaches`, `suspends`,
    /// `blocks`, `terminates`); a refinement narrows, never widens, the base.
    pub refinement_clauses: Vec<TraitRefinementClause>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitRefinementClause {
    /// The authored base requirement name; `None` for the `machine *`
    /// wildcard covering every base requirement.
    pub requirement: Option<DiagnosticName>,
    /// Operational axes only.
    pub signature: StateSignature,
    /// Exact authored `reaches` keyword occurrences on this clause. An
    /// authored `reaches;` (empty names with a keyword span) is a narrowed
    /// empty row; an omitted clause inherits the base row.
    pub service_reach_keyword_source_spans: Vec<source::SourceSpan>,
    /// Authored `reaches` names retained until row binding; clause reach rows
    /// intern against the bound fit check, not the signature pending pass.
    pub service_reaches: Vec<DiagnosticName>,
}

/// One named implementation: a closed map from every inherited requirement to
/// exactly one realization row. The implementation has no data subject; its
/// trait application's types are ordinary arguments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conformance {
    pub symbol: SymbolHandle,
    /// Source visibility retained independently from trait and rows.
    pub is_public: bool,
    pub lifetime_parameters: Vec<DiagnosticName>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub trait_name: DiagnosticName,
    /// Exact trait declaration selected by `trait_name`.
    pub trait_symbol: SymbolHandle,
    /// Declaration-site target-trait lifetime arguments, still source-named
    /// until typed lowering resolves them against this conformance's lifetime
    /// telescope.
    pub trait_lifetime_arguments: Vec<DiagnosticName>,
    pub arguments: HandleSpan<crate::types::TypeReference>,
    pub alias: Option<DiagnosticName>,
    pub rows: Vec<ConformanceRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceRow {
    /// Exact declaring trait after normalization. Inline short names begin
    /// with an invalid symbol and empty diagnostic name, then resolve against
    /// the inherited requirement closure before typed lowering.
    pub declaring_trait: SymbolHandle,
    pub declaring_trait_name: DiagnosticName,
    pub requirement: SymbolHandle,
    pub requirement_name: DiagnosticName,
    /// Pre-normalization declaration ordinal used only by synthesized trait
    /// defaults. Exact requirement symbols replace it before typed lowering.
    pub provisional_requirement_ordinal: Option<usize>,
    /// Exact authored realization. A selected trait-default template keeps
    /// these invalid until per-conformance default instantiation creates its
    /// checked machine; checked dynamic lowering rejects it meanwhile.
    pub realization_machine: SymbolHandle,
    pub realization_state: SymbolHandle,
    pub realization_name: DiagnosticName,
    /// Exact authored target path for an explicit
    /// `Trait::requirement = Machine::entry` row. Inline/default rows have no
    /// independently selected realization declaration.
    pub authored_realization_source_span: Option<source::SourceSpan>,
    /// Pre-normalization root-machine ordinal for inline/default members.
    /// This prevents same-named overloads from being re-selected by text.
    pub provisional_realization_ordinal: Option<usize>,
    pub source: ConformanceRowSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConformanceRowSource {
    Inline,
    Reference,
    TraitDefault,
    /// A sealed core-trait row the compiler emitted as an ordinary checked
    /// member body (structural `Equatable<T>::equals`). An authored member or
    /// reference row suppresses it, so a block holds at most one per slot.
    Synthesized,
}

impl ConformanceRowSource {
    /// Whether the row's realization machine is a member of its own closed
    /// block rather than an independently declared machine the row references.
    pub fn owns_realization(self) -> bool {
        matches!(self, Self::Inline | Self::TraitDefault | Self::Synthesized)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitRequirement {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    /// Erased borrow-region arguments authored on a header parent. These are
    /// semantic relationship data even though they do not affect runtime
    /// generic identity.
    pub lifetime_arguments: Vec<DiagnosticName>,
    /// Generic arguments authored on a header parent (`Policy<C>`). Empty for
    /// the body-level `requires Policy;` form.
    pub arguments: HandleSpan<crate::types::TypeReference>,
}

impl Default for TraitRequirement {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::default(),
            lifetime_arguments: Vec::new(),
            arguments: HandleSpan::empty(),
        }
    }
}

impl Deref for TraitDefinition {
    type Target = TraitStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for TraitDefinition {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}
