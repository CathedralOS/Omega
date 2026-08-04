use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;

use crate::{
    BooleanFact, DomainMembershipFact, Fact, FactPayload, FactPlace, FactPlan, FactRef,
    PlaceHandle, ProgramPoint, TypeConstraintFact,
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

    pub fn boolean_facts(self) -> impl Iterator<Item = BooleanFact> + 'facts {
        self.facts().filter_map(|fact| match fact.payload {
            FactPayload::BooleanExpression(expression)
            | FactPayload::ContractBooleanExpression { expression, .. } => {
                Some(BooleanFact { expression })
            }
            _ => None,
        })
    }

    pub fn domain_memberships(self) -> impl Iterator<Item = DomainMembershipFact> + 'facts {
        self.facts().filter_map(|fact| match fact.payload {
            FactPayload::DomainMembership {
                value,
                domain,
                domain_symbol,
            }
            | FactPayload::ContractDomainMembership {
                value,
                domain,
                domain_symbol,
                ..
            } => Some(DomainMembershipFact {
                value,
                domain,
                domain_symbol,
            }),
            _ => None,
        })
    }

    pub fn proves_domain_membership(
        self,
        value: ExpressionHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        self.domain_memberships().any(|fact| {
            fact.value == value && self.plan.domain_implies(fact.domain_symbol, domain_symbol)
        })
    }

    pub fn proves_boolean_expression_in_program(
        self,
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> bool {
        self.proves_boolean_expression_for_place_in_program(program, expression, None)
    }

    pub fn proves_proposition_label(self, program: &TypedTrees, required_label: &str) -> bool {
        self.facts().any(|fact| {
            self.plan
                .proposition_fact_label(program, fact)
                .is_some_and(|candidate| candidate == required_label)
        })
    }

    pub fn proves_boolean_expression_for_place_in_program(
        self,
        program: &TypedTrees,
        expression: ExpressionHandle,
        place: Option<PlaceHandle>,
    ) -> bool {
        let required_label = program.expression_table.display_name(expression);
        self.facts().any(|fact| {
            let Some(candidate_label) = self.plan.boolean_fact_label(program, fact) else {
                return false;
            };

            candidate_label == required_label
                || place.is_some_and(|required_place| {
                    matches!(fact.place, FactPlace::Place(candidate_place)
                        if self.plan.places_match(program, candidate_place, required_place))
                })
        })
    }

    pub fn proves_place_domain_membership(
        self,
        place: PlaceHandle,
        domain_symbol: SymbolHandle,
    ) -> bool {
        self.facts().any(|fact| {
            matches!(
                fact.payload,
                FactPayload::DomainMembership { domain_symbol: fact_domain, .. }
                    | FactPayload::ContractDomainMembership { domain_symbol: fact_domain, .. }
                    if self.plan.fact_place_equals(fact.place, place)
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

            self.plan.domain_implies(fact_domain, domain_symbol)
                && self.plan.places_match(program, fact_place, place)
        })
    }

    pub fn references_domain(self, domain_symbol: SymbolHandle) -> bool {
        self.domain_memberships()
            .any(|fact| self.plan.domain_implies(fact.domain_symbol, domain_symbol))
    }

    pub fn type_constraints(self) -> impl Iterator<Item = TypeConstraintFact> + 'facts {
        self.facts().filter_map(|fact| match fact.payload {
            FactPayload::TypeConstraint { constraint } => Some(TypeConstraintFact { constraint }),
            _ => None,
        })
    }
}
