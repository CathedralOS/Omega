//! Construction domains are obligations of exact evaluated occurrences.
//!
//! Operand captures already retain contextual destination types. Index those
//! once by state, statement and expression; source-inferred array types omit
//! receiving qualifications, and a later lookup must not invent a new type.
//! This index supplies obligations only. The child's live completion facts and
//! authorized provenance must separately establish every membership.

use super::{
    CheckFacts, Diagnostic, ExpressionHandle, FlowStateFact, SymbolHandle, record_qualification,
    value_proves_qualification_in_contexts, value_satisfies_domain_predicates_in_contexts,
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

    pub(super) fn array_element_domains(
        &self,
        program: &dyn validation::CheckingInput,
        state: SymbolHandle,
        statement: usize,
        expression: ExpressionHandle,
    ) -> Vec<(SymbolHandle, language_semantics::SemanticDomainId)> {
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
            domains.extend(validation::field_domains::domain_constraint_identities(
                program,
                *element_type,
            ));
        }
        domains.sort_by_key(|(symbol, domain)| {
            ((symbol.arena_index(), symbol.generation()), domain.0)
        });
        domains.dedup();
        domains
    }

    fn at(
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

/// Construction obligations share one judgment across records and arrays.
/// Predicate-only membership comes from the completed child's facts; routed
/// membership still needs its exact authorized provenance. The destination's
/// type cannot establish either. Retain only successful source judgments for
/// ordinary child completion and subsequent membership relocation.
#[allow(clippy::too_many_arguments)]
pub(super) fn check_constructor_membership(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    constructor_memberships: &ConstructorMemberships<'_>,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    domain_symbol: SymbolHandle,
    semantic_domain: language_semantics::SemanticDomainId,
    contexts: &[facts::FactContextHandle],
    decisions: &mut arena::Arena<checked_trees::CheckedValueQualification>,
    diagnostics: &mut Vec<Diagnostic>,
    frames: Option<&validation::CallFrameResolver<'_>>,
    owner: &str,
) {
    let predicate_only =
        !validation::field_domains::domain_requires_provenance(program, domain_symbol);
    let vacuous = predicate_only
        && validation::field_domains::domain_is_vacuous(program, domain_symbol, &mut Vec::new());
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
            || (predicate_only
                && capture.is_some()
                && value_satisfies_domain_predicates_in_contexts(
                    program,
                    facts,
                    state,
                    statement_index,
                    value,
                    domain_symbol,
                    contexts,
                    &mut Vec::new(),
                    capture,
                    frames,
                ))
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
                &facts.flow,
                &facts.semantic,
                capture,
                capture.type_reference,
                capture.constraints,
            );
            proves(Some(capture), &contexts)
        })
    };
    if proven {
        record_qualification(decisions, state, statement_index, value, semantic_domain);
    } else {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} is not proven in domain `{}` in {} state `{}` at statement {statement_index}; a construction value must establish the domain's predicates and any required provenance",
            symbol_name(program, domain_symbol),
            machine_name(program, state.machine_symbol),
            symbol_name(program, state.state_symbol),
        )));
    }
}
