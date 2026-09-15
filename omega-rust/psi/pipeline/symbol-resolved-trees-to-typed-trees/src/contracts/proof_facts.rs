use crate::expressions::expression::{
    lower_expression_handle_from_table, lower_expression_handle_from_table_in_fact_position,
};
use crate::lowerer::Lowerer;
use crate::lowerer::name::lower_name;
use crate::type_reference::domain_aliases::expand_domain_reference;
use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(crate) fn lower_proof_facts(
    lowerer: &mut Lowerer,
    facts: HandleSpan<resolved::domain::ProofFact>,
) -> Result<HandleSpan<typed::domain::ProofFact>, Diagnostic> {
    let mut lowered = HandleSpan::empty();

    for (offset, fact) in lowerer.source_trees.proof_facts(facts).iter().enumerate() {
        let source_fact = Handle::from_parts(
            facts
                .start()
                .arena_index()
                .checked_add(u32::try_from(offset).expect("proof fact offset overflow"))
                .expect("proof fact source handle overflow"),
            facts.start().generation(),
        );
        let source_span = lowerer.source_trees.proof_fact_source_span(source_fact);
        match fact {
            resolved::domain::ProofFact::Expression(expression) => {
                if let resolved::expression::ExpressionNode::Call(call) = lowerer
                    .source_trees
                    .tables
                    .bodies
                    .expressions
                    .expression(*expression)
                    && call.target_symbol.is_valid()
                    && matches!(
                        lowerer.source_trees.symbols.get(call.target_symbol).kind,
                        symbols::SymbolKind::Proposition
                            | symbols::SymbolKind::PropositionParameter
                    )
                {
                    let application =
                        crate::expressions::proposition::lower_proposition_application(
                            lowerer, call,
                        )?;
                    let handle = lowerer.typed_trees.proof_facts.append_to_span(
                        &mut lowered,
                        typed::domain::ProofFact::Proposition(application),
                    );
                    if let Some(source_span) = source_span {
                        lowerer
                            .typed_trees
                            .set_proof_fact_source_span(handle, source_span);
                    }
                    continue;
                }
                let expression = lower_expression_handle_from_table_in_fact_position(
                    lowerer.source_trees,
                    &lowerer.source_trees.tables.bodies.expressions,
                    &mut lowerer.typed_trees,
                    *expression,
                )?;
                let handle = lowerer.typed_trees.proof_facts.append_to_span(
                    &mut lowered,
                    typed::domain::ProofFact::Expression(expression),
                );
                if let Some(source_span) = source_span {
                    lowerer
                        .typed_trees
                        .set_proof_fact_source_span(handle, source_span);
                }
            }
            resolved::domain::ProofFact::Membership(membership) => {
                let value = lower_expression_handle_from_table(
                    &lowerer.source_trees.tables.bodies.expressions,
                    &mut lowerer.typed_trees,
                    membership.value,
                )?;
                let authored_path = lowerer
                    .source_trees
                    .domain_path_members(membership.domain)
                    .to_vec();
                let expanded = expand_domain_reference(
                    lowerer.source_trees,
                    membership.domain_symbol,
                    authored_path,
                )?;
                for atom in expanded {
                    let mut domain = HandleSpan::empty();
                    for member in atom.path {
                        lowerer
                            .typed_trees
                            .domain_path_members
                            .append_to_span(&mut domain, lower_name(&member));
                    }
                    let handle = lowerer.typed_trees.proof_facts.append_to_span(
                        &mut lowered,
                        typed::domain::ProofFact::Membership(typed::domain::ProofMembershipFact {
                            value,
                            domain,
                            domain_symbol: atom.symbol,
                            domain_arguments: HandleSpan::empty(),
                            semantic_domain: language_semantics::SemanticDomainId::NULL,
                            authored_domain_selection: membership.authored_domain_selection,
                        }),
                    );
                    if let Some(source_span) = source_span {
                        lowerer
                            .typed_trees
                            .set_proof_fact_source_span(handle, source_span);
                    }
                }
            }
        }
    }

    Ok(lowered)
}
