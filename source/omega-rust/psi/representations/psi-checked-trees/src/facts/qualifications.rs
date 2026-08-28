//! STR4 checked plans, slice 2 (decision 19/22): the semantic
//! QUALIFICATION facts -- per machine, the semantic-domain commitments its
//! body's `as`-casts make, as normalized SemanticDomainId sets. V1 covers
//! the compiler-blessed arithmetic policies (Wrapping/Saturating/Trapping
//! casts -- the closed semantic-facet subset); declared-domain
//! qualification joins when its cast spelling lowers. The published
//! AUTHORITY half waits on the permission model (the facets brief's
//! sealed-by-default introduction).

use psi_language_semantics::SemanticDomainId;
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QualificationFacts {
    /// One entry per machine that COMMITS to at least one semantic domain
    /// (cast-free machines carry no entry), in machine order.
    pub machines: Vec<MachineQualifications>,
    /// Every explicit `as` that qualifies into a domain with no predicates or
    /// establishment routes. The cast remains representation-identical, but
    /// the checked artifact records where vacuous evidence originated.
    pub vacuous_uses: Vec<VacuousQualificationUse>,
    /// P1c: owner-selected, closed, normalized content projections. These are
    /// supplementary to whole-claim identity and never inferred from
    /// multiplicity or names of resource operations.
    pub content: crate::ContentProjectionFacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VacuousQualificationUse {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: u32,
    /// The typed expression carrying the explicit qualification.
    pub expression: ExpressionHandle,
    pub domain: SymbolHandle,
    /// Exact normalized instance selected by the cast. For indexed families
    /// this differs from the declaration's family-level semantic identity.
    pub semantic_domain: SemanticDomainId,
}

impl QualificationFacts {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineQualifications> {
        self.machines.iter().find(|fact| fact.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineQualifications {
    pub machine: SymbolHandle,
    /// The semantic domains this body's casts commit to (sorted, deduped --
    /// the body-observed half; ids from the program's SemanticDomainTable).
    pub body_committed: Vec<SemanticDomainId>,
}
