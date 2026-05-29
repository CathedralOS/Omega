use crate::expression::{
    lower_expression_handle_from_table, lower_expression_handle_from_table_in_program,
};
use crate::lowerer::Lowerer;
use crate::name::lower_name;
use crate::operator::lower_operator_definition;
use crate::type_reference::lower_type_reference_into_table;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_domain_definition(
    lowerer: &mut Lowerer,
    domain: &resolved::domain::DomainDefinition,
) -> Result<typed::domain::DomainDefinition, Diagnostic> {
    let facts = lower_proof_facts(lowerer, domain.facts)?;
    let mut typed_domain = typed::domain::DomainDefinition {
        symbol: domain.symbol,
        name: lower_name(&domain.name),
        target_type: lower_type_reference_into_table(lowerer, &domain.target_type)?,
        facts,
        operators: Default::default(),
        body_token_count: domain.body_token_count,
    };

    for operator in lowerer.source_trees.operator_definitions(domain.operators) {
        let operator = lower_operator_definition(lowerer, operator)?;
        lowerer
            .typed_trees
            .push_domain_operator(&mut typed_domain, operator);
    }

    Ok(typed_domain)
}

pub(crate) fn lower_proof_facts(
    lowerer: &mut Lowerer,
    facts: HandleSpan<resolved::domain::ProofFact>,
) -> Result<HandleSpan<typed::domain::ProofFact>, Diagnostic> {
    let mut lowered = HandleSpan::empty();

    for fact in lowerer.source_trees.proof_facts(facts) {
        let fact = match fact {
            resolved::domain::ProofFact::Expression(expression) => {
                let expression = lower_expression_handle_from_table_in_program(
                    lowerer.source_trees,
                    &lowerer.source_trees.tables.bodies.expressions,
                    &mut lowerer.typed_trees.expression_table,
                    *expression,
                )?;
                typed::domain::ProofFact::Expression(expression)
            }
            resolved::domain::ProofFact::Membership(membership) => {
                let value = lower_expression_handle_from_table(
                    &lowerer.source_trees.tables.bodies.expressions,
                    &mut lowerer.typed_trees.expression_table,
                    membership.value,
                )?;
                let mut domain = HandleSpan::empty();
                for member in lowerer.source_trees.domain_path_members(membership.domain) {
                    lowerer
                        .typed_trees
                        .domain_path_members
                        .append_to_span(&mut domain, lower_name(member));
                }
                typed::domain::ProofFact::Membership(typed::domain::ProofMembershipFact {
                    value,
                    domain,
                    domain_symbol: membership.domain_symbol,
                })
            }
        };

        lowerer
            .typed_trees
            .proof_facts
            .append_to_span(&mut lowered, fact);
    }

    Ok(lowered)
}
