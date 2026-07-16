//! STR4 checked plans, slice 2 (decision 19/22): the semantic
//! QUALIFICATION facts -- per machine, the semantic-domain commitments its
//! body's `as`-casts make, as normalized SemanticDomainId sets. V1 covers
//! the compiler-blessed arithmetic policies (Wrapping/Saturating/Trapping
//! casts -- the closed semantic-facet subset); declared-domain
//! qualification joins when its cast spelling lowers. The published
//! AUTHORITY half waits on the permission model (the facets brief's
//! sealed-by-default introduction).

use omega_core::semantics::SemanticDomainId;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QualificationFacts {
    /// One entry per machine that COMMITS to at least one semantic domain
    /// (cast-free machines carry no entry), in machine order.
    pub machines: Vec<MachineQualifications>,
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
