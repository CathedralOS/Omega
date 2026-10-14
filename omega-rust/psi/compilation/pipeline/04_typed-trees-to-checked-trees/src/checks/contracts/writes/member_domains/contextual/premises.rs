//! Bind live propositions to their actual namespace. A domain membership's
//! predicates name the domain subject, not a same-spelled caller parameter.
//! Reading those predicates transfers no domain identity or issuance authority.

use facts::{Fact, FactPayload, FactPlan};
use typed_trees::domain::DomainDefinition;
use typed_trees::expression::ExpressionHandle;
use validation::CanonicalPlace;

pub(super) struct Premise<'program> {
    pub expression: ExpressionHandle,
    pub holds: bool,
    domain: Option<(&'program DomainDefinition, CanonicalPlace)>,
}

impl Premise<'_> {
    pub fn owner(&self, caller: symbols::SymbolHandle) -> symbols::SymbolHandle {
        self.domain
            .as_ref()
            .map_or(caller, |(domain, _)| domain.symbol)
    }

    pub fn place(
        &self,
        program: &dyn validation::CheckingInput,
        state: symbols::SymbolHandle,
        statement: usize,
        expression: ExpressionHandle,
    ) -> Option<CanonicalPlace> {
        if let Some((domain, base)) = &self.domain {
            let path =
                crate::flow::relative_place_segments_from_expression(program, expression, domain)?;
            let mut place = base.clone();
            place.extend_segments(&path);
            Some(place)
        } else {
            validation::canonical_place_from_expression_in_state(
                program, state, statement, expression,
            )
        }
    }

    pub fn builtin(
        &self,
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: Option<&typed_trees::state::State>,
        expression: ExpressionHandle,
    ) -> bool {
        if let Some((domain, _)) = &self.domain {
            // Each Boolean connective interpreted by the guard judgment must
            // have its own definition-owned meaning, including nested clauses.
            if !validation::has_builtin_domain_decomposed_guard_meaning(program, domain, expression)
            {
                return false;
            }
            match program.expression_table.expression(expression) {
                typed_trees::expression::ExpressionNode::Binary(binary)
                    if matches!(
                        binary.operator,
                        typed_trees::expression::BinaryOperator::And
                            | typed_trees::expression::BinaryOperator::Or
                    ) =>
                {
                    self.builtin(program, machine, state, binary.left)
                        && self.builtin(program, machine, state, binary.right)
                }
                typed_trees::expression::ExpressionNode::Unary(unary) => {
                    self.builtin(program, machine, state, unary.operand)
                }
                _ => true,
            }
        } else {
            validation::has_builtin_bound_expression_meaning(program, machine, state, expression)
        }
    }
}

pub(super) fn from_fact<'program>(
    program: &'program dyn validation::CheckingInput,
    semantic: &FactPlan,
    fact: &Fact,
) -> Vec<Premise<'program>> {
    // A raw callee contract is not a caller observation, even if its
    // instantiation handle is absent. Completed calls publish separate facts.
    if matches!(
        fact.origin,
        facts::FactOrigin::CallRequires | facts::FactOrigin::CallEnsures
    ) {
        return Vec::new();
    }
    let ordinary = match fact.payload {
        FactPayload::BooleanValue { expression, value } => Some((expression, value)),
        FactPayload::BooleanExpression(expression) => Some((expression, true)),
        FactPayload::ContractBooleanExpression {
            kind: facts::ContractFactKind::Requires,
            expression,
            instantiated,
            ..
        } if !instantiated.is_valid() => Some((expression, true)),
        _ => None,
    };
    if let Some((expression, holds)) = ordinary {
        return vec![Premise {
            expression,
            holds,
            domain: None,
        }];
    }
    let (domain_symbol, identity) = match fact.payload {
        FactPayload::DomainMembership {
            domain_symbol,
            semantic_domain,
            ..
        }
        | FactPayload::ContractDomainMembership {
            kind: facts::ContractFactKind::Requires,
            domain_symbol,
            semantic_domain,
            ..
        } => (domain_symbol, semantic_domain),
        _ => return Vec::new(),
    };
    let Some(domain) = program.domain_definition(domain_symbol) else {
        return Vec::new();
    };
    // Declaration-only predicates cannot substitute indexed instances or an
    // alias theory. Keep those on their exact normalized-instance proof route.
    if domain.alias.is_some()
        || domain.semantic_id != identity
        || !typed_trees::domain::has_symbol_only_proof(program, domain_symbol)
    {
        return Vec::new();
    }
    // Semantic construction retains the membership subject here; separate
    // StorageDependency rows invalidate it but never carry membership payloads.
    let Some(subject) = fact.place.as_place().and_then(|place| {
        crate::flow::canonical_place_from_semantic_place(
            program,
            semantic,
            semantic.places.get(place),
        )
    }) else {
        return Vec::new();
    };
    program
        .proof_facts(domain)
        .iter()
        .filter_map(|fact| {
            let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                return None;
            };
            Some(Premise {
                expression: *expression,
                holds: true,
                domain: Some((domain, subject.clone())),
            })
        })
        .collect()
}
