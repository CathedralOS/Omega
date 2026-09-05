use crate::expression::lower_expression_into_table;
use crate::lowerer::Lowerer;
use crate::name::lower_name;
use crate::operator::lower_operator_definition;
use crate::type_reference::lower_type_reference_handle;
use diagnostics::Diagnostic;
use symbol_resolved_trees::domain::{
    DomainAliasConstituent, DomainAliasDefinition, DomainDefinition, ProofFact, ProofMembershipFact,
};
use symbols::SymbolHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_domain_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    domain: &syntax::item::DomainDefinition,
) -> Result<DomainDefinition, Diagnostic> {
    let type_parameters =
        crate::data::lower_type_parameters(lowerer, syntax_trees, domain.type_parameters)?;
    let mut index_arguments = arena::HandleSpan::empty();
    for argument in syntax_trees
        .type_references
        .type_reference_handles(domain.index_arguments)
    {
        let argument = lower_type_reference_handle(lowerer, syntax_trees, *argument)?;
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .child_type_references
            .append_to_span(&mut index_arguments, argument);
    }
    let alias = domain
        .alias
        .as_ref()
        .map(|alias| lower_domain_alias(lowerer, syntax_trees, alias));
    let authored_routes = domain
        .authored_routes
        .iter()
        .map(|route| route.iter().map(lower_name).collect())
        .collect();
    let facts = lower_proof_facts(lowerer, syntax_trees, domain.facts)?;
    // Visibility inheritance for domain-owned operators remains owner question
    // Q1. Their implementation expressions stay private until that source rule
    // is settled; the domain's own predicate facts retain the domain exposure.
    let operators = lowerer.with_authored_expression_exposure(
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
        |lowerer| lower_domain_operators(lowerer, syntax_trees, domain.operators),
    )?;

    // STR4 checked plans, slice 1: mint the normalized semantic identity
    // ONCE here (declaration order); every downstream layer copies it.
    let semantic_id = lowerer
        .symbol_resolved_trees
        .semantic_domains
        .intern(domain.name.as_str());
    // Until authored denotation declarations land, an authored domain-owned
    // operator is the source-level contribution to the denotation/dimension
    // role. This projection happens once; downstream consumers read the
    // explicit role record and never inspect operator presence.
    let semantic_roles = language_semantics::DomainSemanticRoles {
        denotation_dimension: (!operators.is_empty()).then_some(semantic_id),
        arithmetic_policy: None,
    };

    Ok(DomainDefinition {
        symbol: SymbolHandle::invalid(),
        name: lower_name(&domain.name),
        type_parameters,
        target_type: lower_type_reference_handle(lowerer, syntax_trees, domain.target_type)?,
        index_arguments,
        is_public: domain.is_public,
        alias,
        authored_routes,
        classification: domain.classification,
        predicate_body: domain.predicate_body,
        facts,
        operators,
        semantic_clause_token_count: domain.semantic_clause_token_count,
        semantic_id,
        semantic_roles,
        establishment_routes: Vec::new(),
    })
}

fn lower_domain_alias(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    alias: &syntax::item::DomainAliasDefinition,
) -> DomainAliasDefinition {
    let constituents = alias
        .constituents
        .iter()
        .map(|constituent| {
            let mut domain = arena::HandleSpan::empty();
            for member in syntax_trees.items.identifier_path_members(*constituent) {
                lowerer
                    .symbol_resolved_trees
                    .tables
                    .declarations
                    .domain_path_members
                    .append_to_span(&mut domain, lower_name(member));
            }
            DomainAliasConstituent {
                domain,
                domain_symbol: SymbolHandle::invalid(),
            }
        })
        .collect();
    DomainAliasDefinition { constituents }
}

fn lower_domain_operators(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    operators: arena::HandleSpan<syntax::item::OperatorDefinition>,
) -> Result<arena::HandleSpan<symbol_resolved_trees::operator::OperatorDefinition>, Diagnostic> {
    let mut span = arena::HandleSpan::empty();

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
    facts: arena::HandleSpan<syntax::item::ProofFact>,
) -> Result<arena::HandleSpan<ProofFact>, Diagnostic> {
    let mut lowered = arena::HandleSpan::empty();

    for (offset, fact) in syntax_trees.items.proof_facts(facts).iter().enumerate() {
        let source_fact = arena::Handle::from_parts(
            facts
                .start()
                .arena_index()
                .checked_add(u32::try_from(offset).expect("proof fact offset overflow"))
                .expect("proof fact source handle overflow"),
            facts.start().generation(),
        );
        let source_span = syntax_trees.items.proof_fact_source_span(source_fact);
        let fact = match fact {
            syntax::item::ProofFact::Expression(expression) => {
                let expression = lower_expression_into_table(lowerer, syntax_trees, *expression)?;
                ProofFact::Expression(expression)
            }
            syntax::item::ProofFact::Membership(membership) => {
                let value = lower_expression_into_table(lowerer, syntax_trees, membership.value)?;
                let mut domain = arena::HandleSpan::empty();
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
                    authored_domain_selection: None,
                })
            }
        };

        let is_membership = matches!(fact, ProofFact::Membership(_));
        let fact = lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .proof_facts
            .append_to_span(&mut lowered, fact);
        if let Some(source_span) = source_span {
            lowerer
                .symbol_resolved_trees
                .set_proof_fact_source_span(fact, source_span);
        }
        if is_membership && let Some(exposure) = lowerer.current_authored_expression_exposure {
            lowerer
                .pending_authored_proof_memberships
                .push(crate::lowerer::PendingAuthoredProofMembership { fact, exposure });
        }
    }

    Ok(lowered)
}
