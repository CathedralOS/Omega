use arena::HandleSpan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

use crate::{
    DomainMembershipFact, Fact, FactPayload, FactPlace, FactPlan, FactRef, PlaceHandle,
    ProgramPoint, TypeConstraintFact, fact_plan::as_domain_membership,
};

#[derive(Debug, Clone, Copy)]
pub struct FactContextView<'facts> {
    pub(crate) plan: &'facts FactPlan,
    pub point: ProgramPoint,
    pub(crate) facts: HandleSpan<FactRef>,
}

impl<'facts> FactContextView<'facts> {
    pub fn facts(self) -> impl Iterator<Item = &'facts Fact> {
        self.plan
            .refs
            .span_or_empty(self.facts)
            .iter()
            .map(move |reference| self.plan.facts.get(reference.fact))
    }

    pub fn domain_memberships(self) -> impl Iterator<Item = DomainMembershipFact> + 'facts {
        self.facts()
            .filter_map(|fact| as_domain_membership(fact.payload))
    }

    pub fn proves_proposition_label(self, program: &TypedTrees, required_label: &str) -> bool {
        self.facts().any(|fact| {
            self.plan
                .proposition_fact_label(program, fact)
                .is_some_and(|candidate| candidate == required_label)
        })
    }

    pub fn proves_place_domain_membership(
        self,
        program: &TypedTrees,
        place: PlaceHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        if !typed_trees::domain::supports_symbol_only_proof(program, domain_symbol) {
            return false;
        }
        self.facts().any(|fact| {
            matches!(
                fact.payload,
                FactPayload::DomainMembership { domain_symbol: fact_domain, .. }
                    | FactPayload::ContractDomainMembership { domain_symbol: fact_domain, .. }
                    if typed_trees::domain::supports_symbol_only_proof(program, fact_domain)
                        && self.plan.fact_place_equals(fact.place, place)
                && self.plan.domain_implies(fact_domain, domain_symbol)
            )
        })
    }

    pub fn proves_place_domain_membership_in_program(
        self,
        program: &TypedTrees,
        place: PlaceHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        // A declaration-only query cannot select an indexed application. Such
        // obligations must compare the exact retained semantic instance.
        if !typed_trees::domain::supports_symbol_only_proof(program, domain_symbol) {
            return false;
        }
        self.facts().any(|fact| {
            let (fact_domain, fact_place) = match fact.payload {
                FactPayload::DomainMembership { domain_symbol, .. }
                | FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                    let FactPlace::Place(place) = fact.place else {
                        return false;
                    };
                    (domain_symbol, place)
                }
                _ => return false,
            };

            typed_trees::domain::supports_symbol_only_proof(program, fact_domain)
                && self.plan.domain_implies(fact_domain, domain_symbol)
                && self.plan.places_match(program, fact_place, place)
        })
    }

    pub fn type_constraints(self) -> impl Iterator<Item = TypeConstraintFact> + 'facts {
        self.facts().filter_map(|fact| match fact.payload {
            FactPayload::TypeConstraint { constraint } => Some(TypeConstraintFact { constraint }),
            _ => None,
        })
    }
}
