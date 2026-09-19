//! Domain definitions: index arguments, aliases, member operators, and the
//! proof facts a domain body declares.

use crate::lowering::expression::lower_expression_into_table;
use crate::lowering::name::lower_name;
use crate::lowering::operator::lower_operator_definition;
use crate::lowering::type_reference::lower_type_reference_handle;
use crate::resolution::lowerer::Lowerer;
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
    let type_parameters = crate::lowering::data::lower_type_parameters(
        lowerer,
        syntax_trees,
        domain.type_parameters,
    )?;
    let index_arguments = crate::lowering::type_reference::lower_child_type_references(
        lowerer,
        syntax_trees,
        domain.index_arguments,
    )?;
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

    // Mint the family identity once. Indexed applications append canonical
    // arguments to this same key, and Terminal retains it after source erasure.
    // Package aliases and physical roots are not owners: only the reconciled
    // package commitment and checked dependency scope distinguish equal paths.
    let mut semantic_identity =
        declared_module_path(syntax_trees, domain.name.source_span().source_id)
            .map(|module| format!("{module}::{}", domain.name.as_str()))
            .unwrap_or_else(|| domain.name.as_str().to_owned());
    let owner = lowerer
        .sources
        .as_deref()
        .map(|sources| {
            sources.file_at(domain.name.source_span()).ok_or_else(|| {
                Diagnostic::error("domain declaration is missing its checked source owner")
                    .with_source_span(domain.name.source_span())
            })
        })
        .transpose()?;
    if let Some(owner) = owner
        && let Some(package) = owner.package_identity
    {
        let scope = match owner.dependency_scope {
            source::DependencyScope::Product => "product",
            source::DependencyScope::Build => "build",
        };
        let hexadecimal = b"0123456789abcdef";
        let digest = package
            .digest()
            .into_iter()
            .flat_map(|byte| {
                [
                    char::from(hexadecimal[usize::from(byte >> 4)]),
                    char::from(hexadecimal[usize::from(byte & 15)]),
                ]
            })
            .collect::<String>();
        semantic_identity = format!("package:{digest}:{scope}::{semantic_identity}");
    }
    // Source-free probes model one package. Unmanaged sources have no portable
    // package commitment yet; keep their legacy key and the independent
    // cross-owner collision rejection rather than inventing path-based identity.
    let semantic_id = lowerer
        .symbol_resolved_trees
        .semantic_domains
        .intern(&semantic_identity);
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

/// The logical module path declared by the source that owns `source`, if any.
/// Scans root items directly so declaration order cannot hide a module header
/// that follows the domain in source order.
fn declared_module_path(syntax_trees: &SyntaxTrees, source: source::SourceId) -> Option<String> {
    syntax_trees.root_items().find_map(|item| {
        let syntax::item::Item::Module(module) = item else {
            return None;
        };
        let members = syntax_trees.items.identifier_path_members(module.path);
        (members.first()?.source_span().source_id == source).then(|| {
            members
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
        })
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

/// The indexed application on a membership fact lowers exactly like a domain
/// constraint's arguments: each argument becomes a child type reference and
/// its retained const-argument selections keep their slot custody.
fn lower_membership_domain_arguments(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    arguments: arena::HandleSpan<syntax::types::TypeReferenceHandle>,
) -> Result<arena::HandleSpan<symbol_resolved_trees::types::TypeReference>, Diagnostic> {
    if arguments.is_empty() {
        return Ok(arena::HandleSpan::empty());
    }
    let selection_start = lowerer.pending_const_argument_selections.len();
    let lowered = crate::lowering::type_reference::lower_child_type_references(
        lowerer,
        syntax_trees,
        arguments,
    )?;
    crate::lowering::type_reference::retain_const_argument_slots(
        lowerer,
        syntax_trees,
        arguments,
        lowered,
        selection_start,
    );
    Ok(lowered)
}

pub(crate) fn lower_proof_facts(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    facts: arena::HandleSpan<syntax::item::ProofFact>,
) -> Result<arena::HandleSpan<ProofFact>, Diagnostic> {
    lower_proof_facts_excluding(lowerer, syntax_trees, facts, &[])
}

/// Lower runtime facts while leaving template instantiation obligations with
/// their synthesis owner. Offsets refer to the original span, preserving source
/// custody for every retained fact rather than copying/reindexing its syntax.
pub(crate) fn lower_proof_facts_excluding(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    facts: arena::HandleSpan<syntax::item::ProofFact>,
    instantiation_obligation_offsets: &[usize],
) -> Result<arena::HandleSpan<ProofFact>, Diagnostic> {
    let mut lowered = arena::HandleSpan::empty();

    for (offset, fact) in syntax_trees.items.proof_facts(facts).iter().enumerate() {
        if instantiation_obligation_offsets.contains(&offset) {
            continue;
        }
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
                let domain_arguments = lower_membership_domain_arguments(
                    lowerer,
                    syntax_trees,
                    membership.domain_arguments,
                )?;
                ProofFact::Membership(ProofMembershipFact {
                    value,
                    domain,
                    domain_symbol: SymbolHandle::invalid(),
                    domain_arguments,
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
            lowerer.pending_authored_proof_memberships.push(
                crate::resolution::lowerer::PendingAuthoredProofMembership { fact, exposure },
            );
        }
    }

    Ok(lowered)
}
