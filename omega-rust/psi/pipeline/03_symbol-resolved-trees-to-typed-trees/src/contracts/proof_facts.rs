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
                let domain_arguments =
                    lower_membership_domain_arguments(lowerer, membership, source_span)?;
                let expanded = expand_domain_reference(
                    lowerer.source_trees,
                    membership.domain_symbol,
                    authored_path,
                )?;
                if !domain_arguments.is_empty() && expanded.len() != 1 {
                    let alias = lowerer
                        .source_trees
                        .domain_path_members(membership.domain)
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::");
                    return Err(with_optional_source_span(
                        Diagnostic::error(format!(
                            "domain alias `{alias}` does not take index arguments"
                        )),
                        source_span,
                    ));
                }
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
                            domain_arguments,
                            // Interned by `intern_proof_membership_instances`
                            // once the typed symbol table exists, exactly when
                            // domain constraints are normalized.
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

fn with_optional_source_span(
    diagnostic: Diagnostic,
    source_span: Option<source::SourceSpan>,
) -> Diagnostic {
    match source_span {
        Some(source_span) => diagnostic.with_source_span(source_span),
        None => diagnostic,
    }
}

/// Lower an indexed application's arguments exactly as a domain constraint's
/// arguments are lowered, and check their count against the family's index
/// binders when the family is already a typed declaration (every declaration
/// is, for machine, trait, and operator contracts; a domain's own facts may
/// name a later family, which the finish pass then checks). Empty for an
/// unindexed membership.
fn lower_membership_domain_arguments(
    lowerer: &mut Lowerer,
    membership: &resolved::domain::ProofMembershipFact,
    source_span: Option<source::SourceSpan>,
) -> Result<HandleSpan<typed::types::TypeReferenceHandle>, Diagnostic> {
    if membership.domain_arguments.is_empty() {
        return Ok(HandleSpan::empty());
    }
    let source_arguments = lowerer
        .source_trees
        .child_type_references(membership.domain_arguments)
        .to_vec();
    let mut arguments = Vec::with_capacity(source_arguments.len());
    for argument in &source_arguments {
        arguments.push(crate::type_reference::lower_type_reference_into_table(
            lowerer, argument,
        )?);
    }
    if let Some(domain) = lowerer
        .typed_trees
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == membership.domain_symbol)
    {
        check_membership_argument_count(&lowerer.typed_trees, domain, arguments.len())
            .map_err(|diagnostic| with_optional_source_span(diagnostic, source_span))?;
    }
    Ok(lowerer
        .typed_trees
        .type_reference_table
        .insert_type_reference_handles(arguments))
}

fn check_membership_argument_count(
    program: &typed::TypedTrees,
    domain: &typed::domain::DomainDefinition,
    supplied: usize,
) -> Result<(), Diagnostic> {
    let index_parameters = typed::domain::index_parameters(program, domain);
    if supplied != index_parameters.len() {
        return Err(Diagnostic::error(format!(
            "domain family `{}` requires {} closed index argument(s), but {} were supplied",
            domain.name,
            index_parameters.len(),
            supplied
        )));
    }
    Ok(())
}

/// Intern the instance identity of every indexed membership fact, exactly as
/// `type_reference::domain_constraints` interns a constrained type's: the
/// family's semantic name applied to the normalized argument identities. This
/// runs beside domain-constraint normalization, after the typed symbol table
/// exists, because a direct binder's identity is spelled through its symbol
/// path; interning during lowering would give the same binder two names. A
/// membership whose family is not a declared domain (an unresolved path, or a
/// compiler carry permission) has no instance and is diagnosed elsewhere.
pub(crate) fn intern_proof_membership_instances(
    program: &mut typed::TypedTrees,
) -> Result<(), Diagnostic> {
    let mut instances = Vec::new();
    for (handle, fact) in program.proof_facts.iter() {
        let typed::domain::ProofFact::Membership(membership) = fact else {
            continue;
        };
        if membership.domain_arguments.is_empty() || !membership.domain_symbol.is_valid() {
            continue;
        }
        let Some(domain) = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == membership.domain_symbol)
        else {
            continue;
        };
        let source_span = program.proof_fact_source_span(handle);
        check_membership_argument_count(program, domain, membership.domain_arguments.len())
            .map_err(|diagnostic| with_optional_source_span(diagnostic, source_span))?;
        let arguments = program
            .type_reference_table
            .type_reference_handles(membership.domain_arguments);
        let identity = typed::domain::indexed_domain_instance_name(
            program,
            domain,
            typed::domain::index_parameters(program, domain),
            arguments,
        )
        .map_err(|diagnostic| with_optional_source_span(diagnostic, source_span))?;
        instances.push((handle, identity));
    }
    for (handle, identity) in instances {
        let semantic_domain = program.semantic_domains.intern(&identity);
        if let typed::domain::ProofFact::Membership(membership) =
            program.proof_facts.get_mut(handle)
        {
            membership.semantic_domain = semantic_domain;
        }
    }
    Ok(())
}
