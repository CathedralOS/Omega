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
    let target_type = lower_type_reference_into_table(lowerer, &domain.target_type)?;
    let mut typed_domain = typed::domain::DomainDefinition {
        symbol: domain.symbol,
        name: lower_name(&domain.name),
        subject_name: lower_name(&domain.subject_name),
        subject_symbol: domain.subject_symbol,
        type_parameters: HandleSpan::empty(),
        target_type,
        index_arguments: lowerer
            .source_trees
            .child_type_references(domain.index_arguments)
            .iter()
            .map(|argument| lower_type_reference_into_table(lowerer, argument))
            .collect::<Result<Vec<_>, _>>()?,
        is_public: domain.is_public,
        alias,
        cleanup_selection: domain
            .cleanup_selection
            .as_ref()
            .map(|selection| {
                lower_cleanup_selection(lowerer, selection, target_type, domain.symbol)
            })
            .transpose()?,
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

/// Retain the authored authorization occurrence separately from the compiler's
/// eventual cleanup edge. Ordinary calls to this same symbol remain forbidden.
pub(crate) fn lower_cleanup_selection(
    lowerer: &mut Lowerer,
    selection: &resolved::expression::StaticMachineArgument,
    subject: typed::types::TypeReferenceHandle,
    owner: symbols::SymbolHandle,
) -> Result<typed::domain::CleanupSelection, Diagnostic> {
    let authorization = crate::type_reference::retain_path_selection_occurrence(
        &mut lowerer.typed_trees,
        &selection.path,
        selection.symbol,
        lowerer.type_reference_exposure,
        language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::DomainCleanupSelection,
        "domain cleanup",
    )?;
    let selected = crate::expressions::expression::lower_static_machine_argument(
        Some(lowerer.source_trees),
        &mut lowerer.typed_trees,
        lowerer.type_reference_exposure,
        selection,
    )?;
    let arguments = selected
        .application
        .as_ref()
        .map_or(&[][..], |application| &application.arguments);
    let application = lowerer
        .typed_trees
        .expression_table
        .retain_static_machine_application(
            selected.symbol,
            selected.path.last().cloned().unwrap_or_default(),
            arguments,
        );
    let row = lowerer
        .typed_trees
        .expression_table
        .static_machine_application_mut(application);
    row.lexical_owner = owner;
    row.binding_declaration = selected.symbol;
    if let Some(authored) = &selected.application {
        let target = lowerer
            .source_trees
            .machines
            .iter()
            .find(|machine| machine.symbol == selected.symbol);
        let parameters = target.map_or(&[][..], |machine| &machine.lifetime_parameters);
        if !authored.lifetime_arguments.is_empty()
            && authored.lifetime_arguments.len() != parameters.len()
        {
            return Err(Diagnostic::error(
                "cleanup application lifetime argument arity does not match its selected machine",
            ));
        }
        let lifetimes = parameters
            .iter()
            .zip(authored.lifetime_arguments.iter())
            .map(
                |(parameter, argument)| typed::static_applications::StaticMachineLifetimeBinding {
                    parameter: crate::lowerer::name::lower_name(parameter),
                    argument: argument.clone(),
                },
            )
            .collect::<Vec<_>>();
        lowerer
            .typed_trees
            .expression_table
            .set_static_machine_application_lifetimes(application, &lifetimes);
    }
    Ok(typed::domain::CleanupSelection {
        application,
        subject,
        authorization,
        executable: symbols::SymbolHandle::invalid(),
    })
}
