use super::direct::InvocationProof;
use super::labels::domain_proves_expression_label;
use facts::FactPayload;

pub(super) fn prove_boolean_expression_via_context_domain_membership(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    context: &facts::FactContext,
    contexts: &[facts::FactContextHandle],
    point: facts::ProgramPoint,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let candidate_label = program.expression_table.display_name(expression);

    context_domain_proves_label(
        program,
        semantic,
        context,
        contexts,
        point,
        &candidate_label,
    )
}

pub(super) fn prove_instantiated_boolean_expression_via_context_domain_membership(
    proof: &mut InvocationProof<'_, '_>,
    context: &facts::FactContext,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let scope = proof.scope();
    let program = scope.program;
    let candidate_label = proof.required_label(expression);

    let Some(machine) = program.machine_holding_state(scope.caller_state_symbol) else {
        return false;
    };
    context_domain_proves_label(
        program,
        &scope.facts.semantic,
        context,
        scope.contexts,
        facts::ProgramPoint::Statement {
            machine_symbol: machine.symbol,
            state_symbol: scope.caller_state_symbol,
            statement_index: scope.statement_index,
        },
        candidate_label,
    )
}

/// The context-domain scan both membership proofs run once `candidate_label`
/// is known: a `Place` fact's canonical or display base label must prove the
/// label through the domain's own label rule.
fn context_domain_proves_label(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    context: &facts::FactContext,
    contexts: &[facts::FactContextHandle],
    point: facts::ProgramPoint,
    candidate_label: &str,
) -> bool {
    let availability = crate::facts::boolean_predicates::CasePremiseAvailability::new(
        program, semantic, contexts, point,
    );
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
                let Some(place) = fact.place.as_place() else {
                    return false;
                };
                (domain_symbol, value, place)
            }
            _ => return false,
        };
        let Some(subject) = crate::flow::canonical_place_from_semantic_place(
            program,
            semantic,
            semantic.places.get(place),
        ) else {
            return false;
        };
        if !availability.allows(&subject) {
            return false;
        }
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
