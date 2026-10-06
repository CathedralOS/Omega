use facts::{FactPayload, FactPlace};
use symbols::SymbolHandle;

use super::labels::{
    ContractTargetParameters, domain_proves_expression_label,
    instantiate_call_contract_expression_label,
};

pub(super) fn prove_boolean_expression_via_context_domain_membership(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    context: &facts::FactContext,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let candidate_label = program.expression_table.display_name(expression);

    context_domain_proves_label(program, semantic, context, &candidate_label)
}

pub(super) fn prove_instantiated_boolean_expression_via_context_domain_membership(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    context: &facts::FactContext,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &crate::semantic::calls::CallSite<'_>,
    target_state: &(impl ContractTargetParameters + ?Sized),
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let candidate_label = instantiate_call_contract_expression_label(
        program,
        caller_state_symbol,
        statement_index,
        call_site,
        target_state,
        expression,
    );

    context_domain_proves_label(program, semantic, context, &candidate_label)
}

/// The context-domain scan both membership proofs run once `candidate_label`
/// is known: a `Place` fact's canonical or display base label must prove the
/// label through the domain's own label rule.
fn context_domain_proves_label(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    context: &facts::FactContext,
    candidate_label: &String,
) -> bool {
    semantic.context_view(context).facts().any(|fact| {
        let (domain_symbol, value, place) = match fact.payload {
            FactPayload::DomainMembership {
                domain_symbol,
                value,
                ..
            }
            | FactPayload::ContractDomainMembership {
                domain_symbol,
                value,
                ..
            } => {
                let FactPlace::Place(place) = fact.place else {
                    return false;
                };
                (domain_symbol, value, place)
            }
            _ => return false,
        };
        let canonical_base_label = semantic.place_label(program, place);
        let display_base_label = program.expression_table.display_name(value);
        domain_proves_expression_label(
            program,
            domain_symbol,
            &canonical_base_label,
            candidate_label,
        ) || domain_proves_expression_label(
            program,
            domain_symbol,
            &display_base_label,
            candidate_label,
        )
    })
}
