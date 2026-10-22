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
    /// Successful normal-result judgments, before any lowering. These retain
    /// every source exit, including forwarding versus fresh routed issuance.
    pub scalar_returns: Vec<CheckedScalarReturnQualification>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedScalarReturnQualification {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: usize,
    pub transition_target: typed_trees::statement::TransitionTargetHandle,
    /// Which arm `transition_target` names: the transition's continuation
    /// rather than its ordinary target, copied from the checked exit row that
    /// proved this decision. Lowering joins its own exit coordinate to this
    /// arm position instead of reopening the authored transition.
    pub is_continuation: bool,
    pub expression: ExpressionHandle,
    pub semantic_domain: SemanticDomainId,
    pub domain: SymbolHandle,
    pub origin: CheckedScalarReturnOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedScalarReturnOrigin {
    /// Existing membership on this exact completed returned value.
    Forwarded,
    /// Independently proved predicate-only theory.
    Predicate,
    /// Independently proved predicates and this authorized checked route.
    Established(language_semantics::DomainEstablishmentRoute),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CheckedValueQualification {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement_index: u32,
    pub expression: ExpressionHandle,
    pub semantic_domain: SemanticDomainId,
    /// Resolved declaration owns inherited and aliased establishment routes;
    /// the normalized instance identity alone cannot recover that authority.
    pub domain_symbol: SymbolHandle,
    /// Exact constructed value when the producer has no authored operand.
    /// Omitted fields share their parent's diagnostic expression, but never
    /// its membership judgment. Zero means an authored expression occurrence.
    pub structural_value: crate::CheckedStructuralValueHandle,
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
    /// Source judgments for precisely this evaluation occurrence. Construction
    /// plans transport these decisions; receiving declarations supply none.
    pub fn formation_domains_at(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement_index: u32,
        expression: ExpressionHandle,
    ) -> Vec<SemanticDomainId> {
        self.value_formations
            .iter()
            .filter_map(|(_, row)| {
                (!row.structural_value.is_valid()
                    && row.machine == machine
                    && row.state == state
                    && row.statement_index == statement_index
                    && row.expression == expression)
                    .then_some(row.semantic_domain)
            })
            .collect()
    }

    /// Publish the source-to-value association once after checking. Membership
    /// rows carry no exposed handles before this join, so group them into one
    /// arena and let each value retain a span instead of making every lowering
    /// search the program's source facts again. Reordering judgments does not
    /// reorder operand evaluation or create ownership authority.
    pub fn link_value_formations(
        &mut self,
        values: &mut crate::CheckedStructuralValuePlans,
        flow: &crate::FlowFacts,
    ) {
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
                    domain_symbol: use_.domain,
                    structural_value: crate::CheckedStructuralValueHandle::invalid(),
                }),
        );
        // The source judgment may rely on the completion transaction. Attaching
        // that judgment to the raw producer would publish its authority before
        // consuming the parents, then publish it a second time at TransformContent.
        // Only this exact occurrence is deferred; later aliases inherit the
        // completed membership normally, and synthetic values own distinct sites.
        decisions.retain(|decision| !self.transformation_publishes(flow, decision));
        let coordinate =
            |machine: SymbolHandle,
             state: SymbolHandle,
             statement_index: u32,
             expression: ExpressionHandle,
             structural_value: crate::CheckedStructuralValueHandle| {
                (
                    (machine.arena_index(), machine.generation()),
                    (state.arena_index(), state.generation()),
                    statement_index,
                    (expression.arena_index(), expression.generation()),
                    (
                        structural_value.arena_index(),
                        structural_value.generation(),
                    ),
                )
            };
        let site = |decision: &CheckedValueQualification| {
            coordinate(
                decision.machine,
                decision.state,
                decision.statement_index,
                decision.expression,
                decision.structural_value,
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
        values.nodes.for_each_mut(|handle, value| {
            // Synthetic zero construction has its own checked occurrence.
            // Its expression is diagnostic provenance only: consulting that
            // expression would import an enclosing literal's memberships.
            let structural_value = if matches!(
                value.kind,
                crate::CheckedStructuralValueKind::ZeroedStructural { .. }
                    | crate::CheckedStructuralValueKind::ZeroedScalarArray { .. }
            ) {
                handle
            } else {
                crate::CheckedStructuralValueHandle::invalid()
            };
            let key = coordinate(
                value.machine,
                value.state,
                value.statement_index,
                value.expression,
                structural_value,
            );
            value.qualification_decisions = match groups.binary_search_by_key(&key, |(key, _)| *key)
            {
                Ok(position) => groups[position].1,
                Err(_) => arena::HandleSpan::empty(),
            };
        });
    }

    fn transformation_publishes(
        &self,
        flow: &crate::FlowFacts,
        decision: &CheckedValueQualification,
    ) -> bool {
        if decision.structural_value.is_valid() {
            return false;
        }
        self.content.publishes_formation_membership(
            flow,
            decision.state,
            decision.statement_index as usize,
            decision.expression,
            decision.domain_symbol,
            decision.semantic_domain,
        )
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
