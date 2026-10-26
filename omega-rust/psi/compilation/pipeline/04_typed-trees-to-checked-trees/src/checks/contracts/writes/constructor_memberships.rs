//! Construction domains are obligations of exact evaluated occurrences.
//!
//! Operand captures already retain contextual destination types. Index those
//! once by state, statement and expression; source-inferred array types omit
//! receiving qualifications, and a later lookup must not invent a new type.
//! This index supplies obligations only. The child's live completion facts and
//! authorized provenance must separately establish every membership.

use super::member_domains::{
    captured_value_satisfies_indexed_domain, value_satisfies_domain_instance_predicates_in_contexts,
};
use super::{
    CheckFacts, ConstructionWalk, Diagnostic, ExpressionHandle, SymbolHandle, record_qualification,
    value_proves_qualification_in_contexts,
};
use crate::labels::{machine_name, symbol_name};

type Occurrence = ((u32, u32), usize, (u32, u32));

pub(in crate::checks::contracts) struct ConstructorMemberships<'facts> {
    captures: Vec<&'facts checked_trees::FlowOperandFact>,
}

impl<'facts> ConstructorMemberships<'facts> {
    pub(in crate::checks::contracts) fn new(facts: &'facts CheckFacts) -> Self {
        let mut captures = facts
            .flow
            .control
            .operands
            .iter()
            .map(|(_, capture)| capture)
            .filter(|capture| capture.expression.is_valid() && capture.type_reference.is_valid())
            .collect::<Vec<_>>();
        captures.sort_by_key(|capture| occurrence(capture));
        Self { captures }
    }

    pub(super) fn array_element_domains<'program>(
        &self,
        program: &'program dyn validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        expression: ExpressionHandle,
    ) -> Vec<&'program typed_trees::types::DomainConstraint> {
        let mut domains = Vec::new();
        for capture in self.at(state, statement, expression) {
            let Some(reference) =
                validation::unwrapped_type_reference(program, capture.type_reference)
            else {
                continue;
            };
            let typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
                program.type_reference_table.type_reference(reference)
            else {
                continue;
            };
            domains.extend(
                validation::field_domains::declared_domain_constraint_records(
                    program,
                    *element_type,
                ),
            );
        }
        domains.sort_by_key(|constraint| {
            (
                (
                    constraint.symbol.arena_index(),
                    constraint.symbol.generation(),
                ),
                constraint.semantic_id.0,
            )
        });
        domains.dedup();
        domains
    }

    pub(super) fn at(
        &self,
        state: SymbolHandle,
        statement: usize,
        expression: ExpressionHandle,
    ) -> &[&'facts checked_trees::FlowOperandFact] {
        let key = (
            (state.arena_index(), state.generation()),
            statement,
            (expression.arena_index(), expression.generation()),
        );
        let start = self
            .captures
            .partition_point(|capture| occurrence(capture) < key);
        let end = self
            .captures
            .partition_point(|capture| occurrence(capture) <= key);
        &self.captures[start..end]
    }
}

fn occurrence(capture: &checked_trees::FlowOperandFact) -> Occurrence {
    (
        (
            capture.state_symbol.arena_index(),
            capture.state_symbol.generation(),
        ),
        capture.statement_index,
        (
            capture.expression.arena_index(),
            capture.expression.generation(),
        ),
    )
}

impl ConstructionWalk<'_, '_> {
    /// Construction obligations share one judgment across records and arrays.
    /// Predicate-only membership comes from the completed child's facts; routed
    /// membership still needs its exact authorized provenance. The destination's
    /// type cannot establish either. Retain only successful source judgments for
    /// ordinary child completion and subsequent membership relocation.
    pub(super) fn check_constructor_membership(
        &mut self,
        value: ExpressionHandle,
        constraint: &typed_trees::types::DomainConstraint,
        contexts: &[facts::FactContextHandle],
        owner: &str,
    ) {
        let program = self.program;
        let facts = self.facts;
        let state = self.state_flow;
        let statement_index = self.statement_index;
        let constructor_memberships = self.constructor_memberships;
        let frames = self.frames;
        let domain_symbol = constraint.symbol;
        let semantic_domain = constraint.semantic_id;
        // Completed transformations supply the destination membership; publishing
        // it on the raw child would invent authority before parent consumption.
        if facts.qualifications.content.publishes_formation_membership(
            &facts.flow,
            state.state_symbol,
            statement_index,
            value,
            domain_symbol,
            semantic_domain,
        ) {
            return;
        }
        let predicate_only =
            !validation::field_domains::domain_requires_provenance(program, domain_symbol);
        let vacuous = predicate_only
            && validation::field_domains::domain_is_vacuous(
                program,
                domain_symbol,
                &mut Vec::new(),
            );
        let proves = |capture: Option<&checked_trees::FlowOperandFact>,
                      contexts: &[facts::FactContextHandle]| {
            value_proves_qualification_in_contexts(
                program,
                facts,
                state,
                statement_index,
                value,
                domain_symbol,
                semantic_domain,
                contexts,
            ) || vacuous
                || live_bounds_satisfy_domain(
                    program,
                    facts,
                    state,
                    statement_index,
                    value,
                    constraint,
                    contexts,
                )
                || (predicate_only
                    && capture.is_some()
                    && (value_satisfies_domain_instance_predicates_in_contexts(
                        program,
                        facts,
                        state,
                        statement_index,
                        value,
                        domain_symbol,
                        semantic_domain,
                        contexts,
                        &mut Vec::new(),
                        capture,
                        frames,
                    ) || capture.is_some_and(|capture| {
                        captured_value_satisfies_indexed_domain(
                            program, facts, state, capture, constraint, frames,
                        )
                    })))
        };
        let captures = constructor_memberships.at(state.state_symbol, statement_index, value);
        let proven = if captures.is_empty() {
            proves(None, contexts)
        } else {
            // Membership transport and predicate discharge share the same universal
            // occurrence check. No shortcut may choose one capture or use another
            // child's later facts to qualify the value completed here.
            captures.iter().all(|capture| {
                let contexts = crate::flow::operand_contexts(
                    program,
                    &facts.flow.contexts,
                    &facts.flow.control,
                    &facts.semantic,
                    capture,
                    capture.type_reference,
                    capture.constraints,
                );
                proves(Some(capture), &contexts)
            })
        };
        if proven {
            record_qualification(
                self.decisions,
                state,
                statement_index,
                value,
                semantic_domain,
                domain_symbol,
            );
        } else {
            self.diagnostics.push(Diagnostic::error(format!(
            "{owner} is not proven in domain `{}` in {} state `{}` at statement {statement_index}; a construction value must establish the domain's predicates and any required provenance",
            symbol_name(program, domain_symbol),
            machine_name(program, state.machine_symbol),
            symbol_name(program, state.state_symbol),
        )));
        }
    }
}

/// A range proves membership only when it describes the entire selected
/// predicate domain. Necessary numeric consequences alone are insufficient;
/// routed domains and unrecognized conjuncts decline the exact-domain query.
#[allow(clippy::too_many_arguments)]
fn live_bounds_satisfy_domain(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &checked_trees::FlowStateFact,
    statement: usize,
    value: ExpressionHandle,
    constraint: &typed_trees::types::DomainConstraint,
    contexts: &[facts::FactContextHandle],
) -> bool {
    let Some(domain) = program.domain_definition(constraint.symbol) else {
        return false;
    };
    let Some(carrier) = program.primitive_type_reference(domain.target_type) else {
        return false;
    };
    let Some((minimum, maximum)) =
        validation::exact_declared_domain_carrier_interval(program, carrier, constraint)
    else {
        return false;
    };
    let Some(subject) = validation::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement,
        value,
    ) else {
        return false;
    };
    crate::values::integer_bounds_at_place(
        program,
        &facts.semantic,
        contexts
            .iter()
            .map(|context| facts.semantic.contexts.get(*context)),
        &subject,
    )
    .is_some_and(|bounds| minimum <= bounds.minimum && bounds.maximum <= maximum)
}
