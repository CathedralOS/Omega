use crate::expression::lower_expression_into_table;
use crate::lowerer::Lowerer;
use crate::name::lower_name;
use crate::operator::lower_operator_definition;
use crate::type_reference::lower_type_reference_handle;
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_symbol_resolved_trees::domain::{DomainDefinition, ProofFact, ProofMembershipFact};
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_domain_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    domain: &syntax::item::DomainDefinition,
) -> Result<DomainDefinition, Diagnostic> {
    let facts = lower_proof_facts(lowerer, syntax_trees, domain.facts)?;
    let operators = lower_domain_operators(lowerer, syntax_trees, domain.operators)?;
    let classifier = if domain.classifier.is_valid() {
        lower_expression_into_table(
            syntax_trees,
            &mut lowerer.symbol_resolved_trees.tables.bodies.expressions,
            domain.classifier,
        )?
    } else {
        omega_symbol_resolved_trees::expression::ExpressionHandle::invalid()
    };

    Ok(DomainDefinition {
        symbol: SymbolHandle::invalid(),
        name: lower_name(&domain.name),
        target_type: lower_type_reference_handle(lowerer, syntax_trees, domain.target_type)?,
        classifier,
        facts,
        operators,
        body_token_count: domain.body_token_count,
    })
}

fn lower_domain_operators(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    operators: omega_core::arena::HandleSpan<syntax::item::OperatorDefinition>,
) -> Result<
    omega_core::arena::HandleSpan<omega_symbol_resolved_trees::operator::OperatorDefinition>,
    Diagnostic,
> {
    let mut span = omega_core::arena::HandleSpan::empty();

    for operator in syntax_trees.items.operators(operators) {
        let operator = lower_operator_definition(lowerer, syntax_trees, operator)?;
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .operator_definitions
            .append_to_span(&mut span, operator);
    }

    Ok(span)
}

pub(crate) fn lower_proof_facts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    facts: omega_core::arena::HandleSpan<syntax::item::ProofFact>,
) -> Result<omega_core::arena::HandleSpan<ProofFact>, Diagnostic> {
    let mut lowered = omega_core::arena::HandleSpan::empty();

    for fact in syntax_trees.items.proof_facts(facts) {
        let fact = match fact {
            syntax::item::ProofFact::Expression(expression) => {
                let expression = lower_expression_into_table(
                    syntax_trees,
                    &mut lowerer.symbol_resolved_trees.tables.bodies.expressions,
                    *expression,
                )?;
                ProofFact::Expression(expression)
            }
            syntax::item::ProofFact::Membership(membership) => {
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
                ProofFact::Membership(ProofMembershipFact {
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
            .proof_facts
            .append_to_span(&mut lowered, fact);
    }

    Ok(lowered)
}
