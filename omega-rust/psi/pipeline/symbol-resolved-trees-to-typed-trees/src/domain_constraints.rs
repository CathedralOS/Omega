use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use typed_trees::TypedTrees;
use typed_trees::name::Identifier;
use typed_trees::types::{DomainConstraint, DomainConstraintSubject, TypeConstraintNode};

/// Bind and expand every declared-domain type constraint after the complete
/// typed program exists. Carrier-aware lookup precedes transparent-alias
/// expansion, so a short source name never becomes an identity oracle.
pub(crate) fn normalize_domain_constraints(
    source: &SymbolResolvedTrees,
    program: &mut TypedTrees,
) -> Result<(), Diagnostic> {
    normalize_domain_constraints_from(source, program, 0)
}

/// Normalize only type-reference nodes appended after a retained checkpoint.
/// Existing constrained nodes have already published their exact domain and
/// authored-selection identities and are immutable continuation input.
pub(crate) fn normalize_domain_constraints_from(
    source: &SymbolResolvedTrees,
    program: &mut TypedTrees,
    type_reference_frontier: usize,
) -> Result<(), Diagnostic> {
    let sites = program
        .type_reference_table
        .constrained_type_reference_sites()
        .into_iter()
        .filter(|(site, _, _)| {
            usize::try_from(site.arena_index()).is_ok_and(|index| index > type_reference_frontier)
        })
        .collect::<Vec<_>>();

    for (site, carrier, constraints) in sites {
        normalize_constraint_span(source, program, site, carrier, constraints)?;
    }
    Ok(())
}

/// Normalize one newly-lowered type reference immediately when an earlier
/// lowering decision needs its domain-theory records. The full finish pass remains the
/// completeness rail for all other type sites.
pub(crate) fn normalize_domain_constraints_for_type(
    source: &SymbolResolvedTrees,
    program: &mut TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Result<(), Diagnostic> {
    match program
        .type_reference_table
        .type_reference(type_reference)
        .clone()
    {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            normalize_domain_constraints_for_type(source, program, referee)?;
        }
        typed_trees::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => normalize_constraint_span(source, program, type_reference, base_type, constraints)?,
        _ => {}
    }
    Ok(())
}

fn normalize_constraint_span(
    source: &SymbolResolvedTrees,
    program: &mut TypedTrees,
    site: typed_trees::types::TypeReferenceHandle,
    carrier: typed_trees::types::TypeReferenceHandle,
    constraints: arena::HandleSpan<TypeConstraintNode>,
) -> Result<(), Diagnostic> {
    let carrier_label = program.display_type_reference_with_constraints(carrier);
    let authored = program
        .type_reference_table
        .constraints(constraints)
        .to_vec();
    let mut normalized = Vec::new();
    let mut expanded_alias = false;

    for constraint in authored {
        let TypeConstraintNode::Domain(domain_constraint) = constraint else {
            normalized.push(constraint);
            continue;
        };
        let matches = program
            .domain_definitions()
            .iter()
            .filter(|domain| {
                let full = domain.name.as_str();
                let authored = domain_constraint.name.as_str();
                (full == authored || full.rsplit("::").next().unwrap_or(full) == authored)
                    && domain_accepts_carrier(program, domain, carrier, &carrier_label)
            })
            .cloned()
            .collect::<Vec<_>>();
        let [domain] = matches.as_slice() else {
            // Zero matches is either a compiler-known pseudo-domain or an
            // unknown spelling diagnosed later. Multiple matches are
            // rejected by normalized-domain validation; neither case may
            // guess an identity here.
            normalized.push(TypeConstraintNode::Domain(domain_constraint));
            continue;
        };

        if let Some(authored) = domain_constraint.authored_selection {
            program
                .record_resolved_authored_declaration_selection_once(
                    authored.source_span,
                    authored.exposure,
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::DomainMembership,
                    domain.symbol,
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "failed to retain authored domain-constraint selection: {error:?}"
                    ))
                    .with_source_span(authored.source_span)
                })?;
        }

        let index_parameters = typed_trees::domain::index_parameters(program, domain);
        if domain_constraint.arguments.len() != index_parameters.len() {
            return Err(Diagnostic::error(format!(
                "domain family `{}` requires {} closed index argument(s), but {} were supplied",
                domain.name,
                index_parameters.len(),
                domain_constraint.arguments.len()
            )));
        }
        let instance_name = typed_trees::domain::indexed_domain_instance_name(
            program,
            domain,
            index_parameters,
            &domain_constraint.arguments,
        )?;
        let semantic_id = if domain_constraint.arguments.is_empty() {
            domain.semantic_id
        } else {
            program.semantic_domains.intern(&instance_name)
        };
        let semantic_roles = language_semantics::DomainSemanticRoles {
            denotation_dimension: domain
                .semantic_roles
                .denotation_dimension
                .map(|_| semantic_id),
            arithmetic_policy: domain.semantic_roles.arithmetic_policy.map(|_| semantic_id),
        };

        if domain.alias.is_none() {
            normalized.push(TypeConstraintNode::Domain(DomainConstraint {
                name: domain_constraint.name,
                arguments: domain_constraint.arguments,
                subject: DomainConstraintSubject::Declared,
                symbol: domain.symbol,
                semantic_id,
                classification: domain.classification,
                predicate_body: domain.predicate_body,
                semantic_roles,
                establishment_routes: domain.establishment_routes.clone(),
                authored_selection: None,
            }));
            continue;
        }

        expanded_alias = true;
        let source_domain = source
            .domain_definitions
            .iter()
            .find(|candidate| candidate.symbol == domain.symbol)
            .expect("typed alias declaration must have a resolved source");
        for atom in crate::domain::expand_domain_reference(
            source,
            domain.symbol,
            vec![source_domain.name.clone()],
        )? {
            let name = if atom.symbol.is_valid() {
                atom.path
                    .last()
                    .map(crate::name::lower_name)
                    .unwrap_or_else(|| Identifier::generated(""))
            } else {
                Identifier::generated(
                    atom.path
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::"),
                )
            };
            let declaration = program
                .domain_definitions()
                .iter()
                .find(|candidate| candidate.symbol == atom.symbol);
            let subject = if atom.symbol.is_valid() {
                DomainConstraintSubject::Declared
            } else {
                language_semantics::CarryPermission::from_name(name.as_str())
                    .map(DomainConstraintSubject::Carry)
                    .unwrap_or_default()
            };
            normalized.push(TypeConstraintNode::Domain(DomainConstraint {
                name,
                arguments: Vec::new(),
                subject,
                symbol: atom.symbol,
                semantic_id: declaration
                    .map(|domain| domain.semantic_id)
                    .unwrap_or_default(),
                classification: declaration.and_then(|domain| domain.classification),
                predicate_body: declaration
                    .map(|domain| domain.predicate_body)
                    .unwrap_or_default(),
                semantic_roles: declaration
                    .map(|domain| domain.semantic_roles)
                    .unwrap_or_default(),
                establishment_routes: declaration
                    .map(|domain| domain.establishment_routes.clone())
                    .unwrap_or_default(),
                authored_selection: None,
            }));
        }
    }

    if expanded_alias {
        let normalized = program.type_reference_table.insert_constraints(normalized);
        program
            .type_reference_table
            .set_constraint_span(site, normalized);
    } else {
        for (target, value) in program
            .type_reference_table
            .constraints_mut(constraints)
            .iter_mut()
            .zip(normalized)
        {
            *target = value;
        }
    }
    Ok(())
}

pub(crate) fn domain_accepts_carrier(
    program: &TypedTrees,
    domain: &typed_trees::domain::DomainDefinition,
    carrier: typed_trees::types::TypeReferenceHandle,
    carrier_label: &str,
) -> bool {
    if !typed_trees::domain::has_generic_carrier(program, domain) {
        return program.display_type_reference_with_constraints(domain.target_type)
            == carrier_label;
    }
    let parameters = program.domain_type_parameters(domain);
    let Some(parameter) = parameters.first() else {
        return false;
    };
    let typed_trees::data::TypeParameterKind::Type = parameter.kind else {
        return false;
    };
    let typed_trees::types::TypeReferenceNode::Named { symbol, name } = program
        .type_reference_table
        .type_reference(domain.target_type)
    else {
        return false;
    };
    (*symbol == parameter.symbol || name == &parameter.name) && carrier.is_valid()
}
