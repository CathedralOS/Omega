use crate::contracts::proof_facts::lower_proof_facts;
use crate::declarations::operator::lower_operator_definition;
use crate::lowerer::Lowerer;
use crate::lowerer::name::lower_name;
use crate::type_reference::lower_type_reference_into_table;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(crate) fn lower_domain_definition(
    lowerer: &mut Lowerer,
    domain: &resolved::domain::DomainDefinition,
) -> Result<typed::domain::DomainDefinition, Diagnostic> {
    let facts = lower_proof_facts(lowerer, domain.facts)?;
    let alias = if let Some(alias) = domain.alias.as_ref() {
        let constituents = alias
            .constituents
            .iter()
            .map(|constituent| -> Result<_, Diagnostic> {
                let mut path = HandleSpan::empty();
                let members = lowerer.source_trees.domain_path_members(constituent.domain);
                crate::type_reference::retain_static_path_selection(
                    &mut lowerer.typed_trees,
                    members,
                    constituent.domain_symbol,
                    lowerer.type_reference_exposure,
                    "domain-alias",
                )?;
                for member in members {
                    lowerer
                        .typed_trees
                        .domain_path_members
                        .append_to_span(&mut path, lower_name(member));
                }
                Ok(typed::domain::DomainAliasConstituent {
                    domain: path,
                    domain_symbol: constituent.domain_symbol,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Some(typed::domain::DomainAliasDefinition { constituents })
    } else {
        None
    };
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
        classification: domain.classification,
        predicate_body: domain.predicate_body,
        facts,
        operators: Default::default(),
        semantic_clause_token_count: domain.semantic_clause_token_count,
        // Copied, never re-derived (the STR3 propagation rule).
        semantic_id: domain.semantic_id,
        semantic_roles: domain.semantic_roles,
        establishment_routes: domain.establishment_routes.clone(),
    };

    typed_domain.type_parameters =
        crate::signatures::type_parameters::lower_type_parameters(lowerer, domain.type_parameters)?;

    for operator in lowerer.source_trees.operator_definitions(domain.operators) {
        let operator = lowerer.with_type_reference_exposure(
            if operator.is_public {
                language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
            } else {
                language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
            },
            |lowerer| lower_operator_definition(lowerer, operator),
        )?;
        lowerer
            .typed_trees
            .push_domain_operator(&mut typed_domain, operator);
    }

    Ok(typed_domain)
}
