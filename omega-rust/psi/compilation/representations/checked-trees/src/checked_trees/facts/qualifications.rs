//! Source qualification judgments and explicit semantic-domain commitments.
//! Value formations retain exact evaluation coordinates and normalized domain
//! instances; a body's cast commitments are not evidence for another value.
//! The per-machine cast inventory covers
//! the compiler-blessed arithmetic policies (Wrapping/Saturating/Trapping
//! casts -- the closed semantic-facet subset) and declared domains alike:
//! their cast spellings lower to normalized instance identities
//! (`semantic_domain_id` on the typed cast), and sites whose domain carries
//! no predicates or establishment routes are recorded in `vacuous_uses`.
//! The published AUTHORITY half waits on the permission model (the facets
//! brief's sealed-by-default introduction).

use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QualificationFacts {
    /// Membership proved at one value's evaluation point. A later write or
    /// another occurrence of the same place cannot supply this decision.
    /// These source-checking decisions are not portable proof certificates
    /// and do not discharge independent ownership or cleanup obligations.
    pub value_formations: arena::Arena<CheckedValueQualification>,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CheckedValueQualification {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: u32,
    pub expression: ExpressionHandle,
    pub semantic_domain: SemanticDomainId,
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
    /// Publish the source-to-value association once after checking. Membership
    /// rows carry no exposed handles before this join, so group them into one
    /// arena and let each value retain a span instead of making every lowering
    /// search the program's source facts again. Reordering judgments does not
    /// reorder operand evaluation or create ownership authority.
    pub fn link_value_formations(&mut self, values: &mut crate::CheckedStructuralValuePlans) {
        let mut decisions = std::mem::take(&mut self.value_formations)
            .into_items()
            .collect::<Vec<_>>();
        // A vacuous qualification owes nothing, so its judgment succeeded at
        // the cast's own occurrence. A structural value produced there carries
        // that membership exactly as an annotated initializer's value does.
        decisions.extend(
            self.vacuous_uses
                .iter()
                .map(|use_| CheckedValueQualification {
                    machine: use_.machine,
                    state: use_.state,
                    statement_index: use_.statement_index,
                    expression: use_.expression,
                    semantic_domain: use_.semantic_domain,
                }),
        );
        let coordinate = |machine: SymbolHandle,
                          state: SymbolHandle,
                          statement_index: u32,
                          expression: ExpressionHandle| {
            (
                (machine.arena_index(), machine.generation()),
                (state.arena_index(), state.generation()),
                statement_index,
                (expression.arena_index(), expression.generation()),
            )
        };
        let site = |decision: &CheckedValueQualification| {
            coordinate(
                decision.machine,
                decision.state,
                decision.statement_index,
                decision.expression,
            )
        };
        decisions.sort_by_key(|decision| (site(decision), decision.semantic_domain.0));
        decisions.dedup();
        let mut groups = Vec::new();
        let mut start = 0;
        while start < decisions.len() {
            let key = site(&decisions[start]);
            let mut end = start + 1;
            while end < decisions.len() && site(&decisions[end]) == key {
                end += 1;
            }
            let span = self
                .value_formations
                .insert_many(decisions[start..end].iter().copied());
            groups.push((key, span));
            start = end;
        }
        values.nodes.for_each_mut(|_, value| {
            let key = coordinate(
                value.machine,
                value.state,
                value.statement_index,
                value.expression,
            );
            value.qualification_decisions = match groups.binary_search_by_key(&key, |(key, _)| *key)
            {
                Ok(position) => groups[position].1,
                Err(_) => arena::HandleSpan::empty(),
            };
        });
    }

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
