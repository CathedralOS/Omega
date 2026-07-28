use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees::SymbolResolvedTrees;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::name::Identifier;
use omega_typed_trees::types::{DomainConstraint, TypeConstraintNode};

/// Bind and expand every declared-domain type constraint after the complete
/// typed program exists. Carrier-aware lookup precedes transparent-alias
/// expansion, so a short source name never becomes an identity oracle.
pub(crate) fn normalize_domain_constraints(
    source: &SymbolResolvedTrees,
    program: &mut TypedTrees,
) -> Result<(), Diagnostic> {
    let sites = program
        .type_reference_table
        .constrained_type_reference_sites();

    for (site, carrier, constraints) in sites {
        normalize_constraint_span(source, program, site, carrier, constraints)?;
    }
    Ok(())
}

/// Normalize one newly-lowered type reference immediately when an earlier
/// lowering decision needs its facet axes. The full finish pass remains the
/// completeness rail for all other type sites.
pub(crate) fn normalize_domain_constraints_for_type(
    source: &SymbolResolvedTrees,
    program: &mut TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> Result<(), Diagnostic> {
    match program
        .type_reference_table
        .type_reference(type_reference)
        .clone()
    {
        omega_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            normalize_domain_constraints_for_type(source, program, referee)?;
        }
        omega_typed_trees::types::TypeReferenceNode::Constrained {
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
    site: omega_typed_trees::types::TypeReferenceHandle,
    carrier: omega_typed_trees::types::TypeReferenceHandle,
    constraints: omega_core::arena::HandleSpan<TypeConstraintNode>,
) -> Result<(), Diagnostic> {
    let carrier = program.display_type_reference_with_constraints(carrier);
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
                full.rsplit("::").next().unwrap_or(full) == domain_constraint.name.as_str()
                    && program.display_type_reference_with_constraints(domain.target_type)
                        == carrier
            })
            .collect::<Vec<_>>();
        let [domain] = matches.as_slice() else {
            // Zero matches is either a compiler-known pseudo-domain or an
            // unknown spelling diagnosed later. Multiple matches are
            // rejected by normalized-domain validation; neither case may
            // guess an identity here.
            normalized.push(TypeConstraintNode::Domain(domain_constraint));
            continue;
        };

        if domain.alias.is_none() {
            normalized.push(TypeConstraintNode::Domain(DomainConstraint {
                name: domain_constraint.name,
                symbol: domain.symbol,
                semantic_id: domain.semantic_id,
                facets: domain.facets,
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
            let name = atom
                .path
                .last()
                .map(crate::name::lower_name)
                .unwrap_or_else(|| Identifier::generated(""));
            let declaration = program
                .domain_definitions()
                .iter()
                .find(|candidate| candidate.symbol == atom.symbol);
            normalized.push(TypeConstraintNode::Domain(DomainConstraint {
                name,
                symbol: atom.symbol,
                semantic_id: declaration
                    .map(|domain| domain.semantic_id)
                    .unwrap_or_default(),
                facets: declaration.map(|domain| domain.facets).unwrap_or_default(),
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
