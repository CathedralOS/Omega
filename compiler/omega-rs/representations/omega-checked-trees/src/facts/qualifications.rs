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
use omega_typed_trees::expression::ExpressionHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QualificationFacts {
    /// One entry per machine that COMMITS to at least one semantic domain
    /// (cast-free machines carry no entry), in machine order.
    pub machines: Vec<MachineQualifications>,
    /// Every accepted use of the closed canonical bodyless-qualification
    /// relationship, retained even though checked lowering erases the
    /// satisfier invocation from the executable tree.
    pub canonical_uses: Vec<CanonicalQualificationUse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalQualificationUseKind {
    ImplicitCast,
    NamedSatisfierCall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalQualificationUse {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: u32,
    /// The typed expression that carried the use before erasure. Statement
    /// calls have no expression root and retain an invalid handle here.
    pub expression: ExpressionHandle,
    pub domain: SymbolHandle,
    pub satisfier: SymbolHandle,
    pub kind: CanonicalQualificationUseKind,
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
