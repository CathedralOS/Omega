use omega_typed_trees::TypedTrees;
use omega_typed_trees::types::TypeConstraintNode;

/// DOM1/STR2: bind every declared-domain type constraint after the complete
/// typed program exists. A short domain name is not an identity: the carrier
/// participates in resolution, so this pass deliberately runs after all
/// domain target types and all constrained use-site carriers are available.
pub(crate) fn normalize_domain_constraints(program: &mut TypedTrees) {
    let sites = program.type_reference_table.constrained_type_references();

    for (carrier, constraints) in sites {
        normalize_constraint_span(program, carrier, constraints);
    }
}

/// Normalize one newly-lowered type reference immediately when an earlier
/// lowering decision needs its facet axes. The full finish pass remains the
/// completeness rail for all other type sites.
pub(crate) fn normalize_domain_constraints_for_type(
    program: &mut TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) {
    match program
        .type_reference_table
        .type_reference(type_reference)
        .clone()
    {
        omega_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            normalize_domain_constraints_for_type(program, referee);
        }
        omega_typed_trees::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => normalize_constraint_span(program, base_type, constraints),
        _ => {}
    }
}

fn normalize_constraint_span(
    program: &mut TypedTrees,
    carrier: omega_typed_trees::types::TypeReferenceHandle,
    constraints: omega_core::arena::HandleSpan<TypeConstraintNode>,
) {
    let carrier = program.display_type_reference_with_constraints(carrier);
    let normalized = program
        .type_reference_table
        .constraints(constraints)
        .iter()
        .map(|constraint| {
            let TypeConstraintNode::Domain(domain_constraint) = constraint else {
                return None;
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
                return None;
            };
            Some((domain.symbol, domain.semantic_id, domain.facets))
        })
        .collect::<Vec<_>>();

    for (constraint, normalized) in program
        .type_reference_table
        .constraints_mut(constraints)
        .iter_mut()
        .zip(normalized)
    {
        let (TypeConstraintNode::Domain(domain_constraint), Some(normalized)) =
            (constraint, normalized)
        else {
            continue;
        };
        domain_constraint.symbol = normalized.0;
        domain_constraint.semantic_id = normalized.1;
        domain_constraint.facets = normalized.2;
    }
}
