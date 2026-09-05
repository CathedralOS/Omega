//! Concatenation derives predicates from the operands at assignment time.
//! The destination retains a materialized-value fact, not expressions that
//! would read the mutable operands again at a later contract boundary.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn append_concatenated_predicates(
    program: &psi_typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    contexts: &FlowBuildContext,
    active: HandleSpan<FlowSemanticContextRef>,
    expression: ExpressionHandle,
    destination: PlaceHandle,
    point: ProgramPoint,
    references: &mut HandleSpan<psi_facts::FactRef>,
) {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Binary(binary)
            if binary.operator == psi_typed_trees::expression::BinaryOperator::Add
    ) {
        return;
    }
    let active_facts: Vec<_> = contexts
        .contexts
        .semantic_context_refs
        .span_or_empty(active)
        .iter()
        .flat_map(|reference| {
            semantic
                .context_view(semantic.contexts.get(reference.context))
                .facts()
        })
        .copied()
        .collect();
    for domain in program.domain_definitions() {
        // Concatenation proves a byte predicate, never a routed qualification.
        if !domain.establishment_routes.is_empty()
            || domain.alias.is_some()
            || !domain.predicate_body.is_present()
            || !crate::field_domain::domain_is_concat_preserving(program, domain.symbol)
            || !value_proves_predicate(
                program,
                semantic,
                &active_facts,
                expression,
                domain.symbol,
                point,
            )
        {
            continue;
        }
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(destination),
            point,
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::DomainMembership {
                value: ExpressionHandle::invalid(),
                domain: HandleSpan::empty(),
                domain_symbol: domain.symbol,
            },
        });
        semantic.append_ref(references, fact);
    }
}

fn value_proves_predicate(
    program: &psi_typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    active: &[Fact],
    expression: ExpressionHandle,
    domain: SymbolHandle,
    point: ProgramPoint,
) -> bool {
    if crate::field_domain::string_literal_expression_grants_domain(program, expression, domain) {
        return true;
    }
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
        && binary.operator == psi_typed_trees::expression::BinaryOperator::Add
    {
        return value_proves_predicate(program, semantic, active, binary.left, domain, point)
            && value_proves_predicate(program, semantic, active, binary.right, domain, point);
    }
    let ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        statement_index,
    } = point
    else {
        return false;
    };
    let Some(place) = contextual_expression_place(
        program,
        semantic,
        machine_symbol,
        state_symbol,
        statement_index,
        expression,
    ) else {
        return false;
    };
    active.iter().any(|fact| {
        let FactPlace::Place(source) = fact.place else {
            return false;
        };
        if !semantic.places_equal(source, place) {
            return false;
        }
        match fact.payload {
            FactPayload::DomainMembership { domain_symbol, .. }
            | FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                // The materialized bytes retain the same primitive predicate
                // across bounded carriers. This is not carrier/domain identity.
                crate::field_domain::domain_byte_predicate(program, domain_symbol)
                    == crate::field_domain::domain_byte_predicate(program, domain)
            }
            FactPayload::AssignedValue { value } => {
                crate::field_domain::string_literal_expression_grants_domain(program, value, domain)
            }
            _ => false,
        }
    })
}
