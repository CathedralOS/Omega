use crate::expression::{
    lower_expression_handle_from_table, lower_expression_handle_from_table_in_fact_position,
};
use crate::lowerer::Lowerer;
use crate::name::lower_name;
use crate::operator::lower_operator_definition;
use crate::type_reference::lower_type_reference_into_table;
use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees as resolved;
use psi_typed_trees as typed;

#[derive(Debug, Clone)]
pub(crate) struct ExpandedDomainReference {
    pub(crate) symbol: psi_symbols::SymbolHandle,
    pub(crate) path: Vec<resolved::name::DiagnosticName>,
}

/// Expand a transparent domain alias to its atomic declared domains. This is
/// the shared pre-normalization operation for proof facts, executable
/// membership, and constrained types.
pub(crate) fn expand_domain_reference(
    program: &resolved::SymbolResolvedTrees,
    symbol: psi_symbols::SymbolHandle,
    path: Vec<resolved::name::DiagnosticName>,
) -> Result<Vec<ExpandedDomainReference>, Diagnostic> {
    fn expand(
        program: &resolved::SymbolResolvedTrees,
        symbol: psi_symbols::SymbolHandle,
        path: Vec<resolved::name::DiagnosticName>,
        stack: &mut Vec<psi_symbols::SymbolHandle>,
    ) -> Result<Vec<ExpandedDomainReference>, Diagnostic> {
        let name = path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        if name == "Carry::Portable" {
            return Ok(psi_language_semantics::CarryPermission::ALL
                .into_iter()
                .map(|permission| {
                    let [namespace, member] = permission
                        .name()
                        .split("::")
                        .collect::<Vec<_>>()
                        .try_into()
                        .expect("compiler carry permission has two path members");
                    ExpandedDomainReference {
                        symbol: psi_symbols::SymbolHandle::invalid(),
                        path: vec![
                            resolved::name::DiagnosticName::generated_static(namespace),
                            resolved::name::DiagnosticName::generated_static(member),
                        ],
                    }
                })
                .collect());
        }
        let Some(domain) = program
            .domain_definitions
            .iter()
            .find(|domain| domain.symbol == symbol)
        else {
            // Preserve unresolved spellings for the validation layer, which
            // owns the ordinary unknown-domain diagnostic.
            return Ok(vec![ExpandedDomainReference { symbol, path }]);
        };
        let Some(alias) = domain.alias.as_ref() else {
            return Ok(vec![ExpandedDomainReference { symbol, path }]);
        };
        if let Some(cycle_start) = stack.iter().position(|candidate| *candidate == symbol) {
            let cycle = stack[cycle_start..]
                .iter()
                .copied()
                .chain(std::iter::once(symbol))
                .filter_map(|candidate| {
                    program
                        .domain_definitions
                        .iter()
                        .find(|domain| domain.symbol == candidate)
                })
                .map(|domain| domain.name.to_string())
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(Diagnostic::error(format!("domain alias cycle: {cycle}")));
        }
        if alias.constituents.is_empty() {
            return Err(Diagnostic::error(format!(
                "domain alias `{}` must name at least one constituent",
                domain.name
            )));
        }

        stack.push(symbol);
        let mut expanded = Vec::new();
        for constituent in &alias.constituents {
            let constituent_path = program.domain_path_members(constituent.domain).to_vec();
            expanded.extend(expand(
                program,
                constituent.domain_symbol,
                constituent_path,
                stack,
            )?);
        }
        stack.pop();
        Ok(expanded)
    }

    expand(program, symbol, path, &mut Vec::new())
}

pub(crate) fn lower_domain_definition(
    lowerer: &mut Lowerer,
    domain: &resolved::domain::DomainDefinition,
) -> Result<typed::domain::DomainDefinition, Diagnostic> {
    let facts = lower_proof_facts(lowerer, domain.facts)?;
    let alias = domain.alias.as_ref().map(|alias| {
        let constituents = alias
            .constituents
            .iter()
            .map(|constituent| {
                let mut path = HandleSpan::empty();
                for member in lowerer.source_trees.domain_path_members(constituent.domain) {
                    lowerer
                        .typed_trees
                        .domain_path_members
                        .append_to_span(&mut path, lower_name(member));
                }
                typed::domain::DomainAliasConstituent {
                    domain: path,
                    domain_symbol: constituent.domain_symbol,
                }
            })
            .collect();
        typed::domain::DomainAliasDefinition { constituents }
    });
    let mut typed_domain = typed::domain::DomainDefinition {
        symbol: domain.symbol,
        name: lower_name(&domain.name),
        type_parameters: HandleSpan::empty(),
        target_type: lower_type_reference_into_table(lowerer, &domain.target_type)?,
        index_arguments: lowerer
            .source_trees
            .child_type_references(domain.index_arguments)
            .iter()
            .map(|argument| lower_type_reference_into_table(lowerer, argument))
            .collect::<Result<Vec<_>, _>>()?,
        is_public: domain.is_public,
        alias,
        predicate_body: domain.predicate_body,
        facts,
        operators: Default::default(),
        semantic_clause_token_count: domain.semantic_clause_token_count,
        // Copied, never re-derived (the STR3 propagation rule).
        semantic_id: domain.semantic_id,
        semantic_roles: domain.semantic_roles,
        establishment_routes: domain.establishment_routes.clone(),
    };

    for parameter in lowerer
        .source_trees
        .data_type_parameters(domain.type_parameters)
    {
        let parameter = crate::data::lower_type_parameter(lowerer, parameter)?;
        lowerer
            .typed_trees
            .push_domain_type_parameter(&mut typed_domain, parameter);
    }

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
                        psi_symbols::SymbolKind::Proposition
                            | psi_symbols::SymbolKind::PropositionParameter
                    )
                {
                    let application =
                        crate::proposition::lower_proposition_application(lowerer, call)?;
                    lowerer.typed_trees.proof_facts.append_to_span(
                        &mut lowered,
                        typed::domain::ProofFact::Proposition(application),
                    );
                    continue;
                }
                let expression = lower_expression_handle_from_table_in_fact_position(
                    lowerer.source_trees,
                    &lowerer.source_trees.tables.bodies.expressions,
                    &mut lowerer.typed_trees,
                    *expression,
                )?;
                lowerer.typed_trees.proof_facts.append_to_span(
                    &mut lowered,
                    typed::domain::ProofFact::Expression(expression),
                );
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
                    lowerer.typed_trees.proof_facts.append_to_span(
                        &mut lowered,
                        typed::domain::ProofFact::Membership(typed::domain::ProofMembershipFact {
                            value,
                            domain,
                            domain_symbol: atom.symbol,
                        }),
                    );
                }
            }
        }
    }

    Ok(lowered)
}
