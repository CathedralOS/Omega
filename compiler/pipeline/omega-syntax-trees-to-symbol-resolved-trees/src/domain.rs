use crate::expression::lower_expression_into_table;
use crate::name::lower_name;
use crate::program::Lowerer;
use crate::type_reference::lower_type_reference_handle;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::domain::{DomainDefinition, DomainFact, DomainMembershipFact};
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_domain_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    domain: &syntax::item::DomainDefinition,
) -> Result<DomainDefinition, Diagnostic> {
    let facts = lower_domain_facts(lowerer, syntax_trees, domain.facts)?;

    Ok(DomainDefinition {
        symbol: SymbolHandle::invalid(),
        name: lower_name(&domain.name),
        target_type: lower_type_reference_handle(lowerer, syntax_trees, domain.target_type)?,
        facts,
        body_token_count: domain.body_token_count,
    })
}

fn lower_domain_facts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    facts: omega_core::arena::HandleSpan<syntax::item::DomainFact>,
) -> Result<omega_core::arena::HandleSpan<DomainFact>, Diagnostic> {
    let mut lowered = omega_core::arena::HandleSpan::empty();

    for fact in syntax_trees.items.domain_facts(facts) {
        let fact = match fact {
            syntax::item::DomainFact::Expression(expression) => {
                let expression = lower_expression_into_table(
                    syntax_trees,
                    &mut lowerer.symbol_resolved_trees.tables.bodies.expressions,
                    *expression,
                )?;
                DomainFact::Expression(expression)
            }
            syntax::item::DomainFact::Membership(membership) => {
                let value = lower_expression_into_table(
                    syntax_trees,
                    &mut lowerer.symbol_resolved_trees.tables.bodies.expressions,
                    membership.value,
                )?;
                let mut domain = omega_core::arena::HandleSpan::empty();
                for member in syntax_trees
                    .items
                    .identifier_path_members(membership.domain)
                {
                    lowerer
                        .symbol_resolved_trees
                        .tables
                        .declarations
                        .domain_path_members
                        .append_to_span(&mut domain, lower_name(member));
                }
                DomainFact::Membership(DomainMembershipFact {
                    value,
                    domain,
                    domain_symbol: SymbolHandle::invalid(),
                })
            }
        };

        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .domain_facts
            .append_to_span(&mut lowered, fact);
    }

    Ok(lowered)
}
