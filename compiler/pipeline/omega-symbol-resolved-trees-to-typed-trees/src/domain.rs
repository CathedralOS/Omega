use crate::expression::lower_expression_handle_from_table;
use crate::name::lower_name;
use crate::program::Lowerer;
use crate::type_reference::lower_type_reference_into_table;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_domain_definition(
    lowerer: &mut Lowerer,
    domain: &resolved::domain::DomainDefinition,
) -> Result<typed::domain::DomainDefinition, Diagnostic> {
    let facts = lower_domain_facts(lowerer, domain.facts)?;

    Ok(typed::domain::DomainDefinition {
        symbol: domain.symbol,
        name: lower_name(&domain.name),
        target_type: lower_type_reference_into_table(lowerer, &domain.target_type)?,
        facts,
        body_token_count: domain.body_token_count,
    })
}

fn lower_domain_facts(
    lowerer: &mut Lowerer,
    facts: HandleSpan<resolved::domain::DomainFact>,
) -> Result<HandleSpan<typed::domain::DomainFact>, Diagnostic> {
    let mut lowered = HandleSpan::empty();

    for fact in lowerer.source_trees.domain_facts(facts) {
        let fact = match fact {
            resolved::domain::DomainFact::Expression(expression) => {
                let expression = lower_expression_handle_from_table(
                    &lowerer.source_trees.tables.bodies.expressions,
                    &mut lowerer.typed_trees.expression_table,
                    *expression,
                )?;
                typed::domain::DomainFact::Expression(expression)
            }
            resolved::domain::DomainFact::Membership(membership) => {
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
                typed::domain::DomainFact::Membership(typed::domain::DomainMembershipFact {
                    value,
                    domain,
                    domain_symbol: membership.domain_symbol,
                })
            }
        };

        lowerer
            .typed_trees
            .domain_facts
            .append_to_span(&mut lowered, fact);
    }

    Ok(lowered)
}
