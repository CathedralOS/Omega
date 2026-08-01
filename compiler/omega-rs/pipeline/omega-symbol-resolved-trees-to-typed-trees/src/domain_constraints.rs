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
/// lowering decision needs its domain-theory records. The full finish pass remains the
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

        let index_parameters = program.domain_type_parameters(domain);
        let index_parameters = if index_parameters.is_empty() {
            &[][..]
        } else {
            &index_parameters[1..]
        };
        if domain_constraint.arguments.len() != index_parameters.len() {
            return Err(Diagnostic::error(format!(
                "domain family `{}` requires {} closed index argument(s), but {} were supplied",
                domain.name,
                index_parameters.len(),
                domain_constraint.arguments.len()
            )));
        }
        let instance_name = indexed_domain_instance_name(
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
        let semantic_roles = omega_core::semantics::DomainSemanticRoles {
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
                symbol: domain.symbol,
                semantic_id,
                predicate_body: domain.predicate_body,
                semantic_roles,
                establishment_routes: domain.establishment_routes.clone(),
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
            normalized.push(TypeConstraintNode::Domain(DomainConstraint {
                name,
                arguments: Vec::new(),
                symbol: atom.symbol,
                semantic_id: declaration
                    .map(|domain| domain.semantic_id)
                    .unwrap_or_default(),
                predicate_body: declaration
                    .map(|domain| domain.predicate_body)
                    .unwrap_or_default(),
                semantic_roles: declaration
                    .map(|domain| domain.semantic_roles)
                    .unwrap_or_default(),
                establishment_routes: declaration
                    .map(|domain| domain.establishment_routes.clone())
                    .unwrap_or_default(),
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

fn domain_accepts_carrier(
    program: &TypedTrees,
    domain: &omega_typed_trees::domain::DomainDefinition,
    carrier: omega_typed_trees::types::TypeReferenceHandle,
    carrier_label: &str,
) -> bool {
    let parameters = program.domain_type_parameters(domain);
    if parameters.is_empty() {
        return program.display_type_reference_with_constraints(domain.target_type)
            == carrier_label;
    }
    let Some(parameter) = parameters.first() else {
        return false;
    };
    let omega_typed_trees::data::TypeParameterKind::Type = parameter.kind else {
        return false;
    };
    let omega_typed_trees::types::TypeReferenceNode::Named { symbol, name } = program
        .type_reference_table
        .type_reference(domain.target_type)
    else {
        return false;
    };
    (*symbol == parameter.symbol || name == &parameter.name) && carrier.is_valid()
}

fn indexed_domain_instance_name(
    program: &TypedTrees,
    domain: &omega_typed_trees::domain::DomainDefinition,
    parameters: &[omega_typed_trees::data::TypeParameter],
    arguments: &[omega_typed_trees::types::TypeReferenceHandle],
) -> Result<String, Diagnostic> {
    if arguments.is_empty() {
        return Ok(program
            .semantic_domains
            .name(domain.semantic_id)
            .unwrap_or(domain.name.as_str())
            .to_owned());
    }
    let mut identities = Vec::with_capacity(arguments.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let omega_typed_trees::data::TypeParameterKind::Const { type_reference } = parameter.kind
        else {
            return Err(Diagnostic::error(format!(
                "domain family `{}` has a non-const index binder `{}`",
                domain.name, parameter.name
            )));
        };
        let expected = const_index_type_name(program, type_reference)?;
        identities.push(closed_domain_argument_identity(
            program, *argument, &expected,
        )?);
    }
    let base = program
        .semantic_domains
        .name(domain.semantic_id)
        .unwrap_or(domain.name.as_str());
    Ok(format!("{base}<{}>", identities.join(",")))
}

fn const_index_type_name(
    program: &TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> Result<String, Diagnostic> {
    use omega_typed_trees::types::{FixedArrayLength, TypeReferenceNode};
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => Ok(name.as_str().to_owned()),
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => Ok(format!(
            "[{}; {length}]",
            const_index_type_name(program, *element_type)?
        )),
        TypeReferenceNode::Constrained { base_type, .. } => {
            const_index_type_name(program, *base_type)
        }
        TypeReferenceNode::Unit => Ok("()".to_owned()),
        _ => Err(Diagnostic::error(
            "indexed-domain const parameter types must have canonical structural identity",
        )),
    }
}

fn closed_domain_argument_identity(
    program: &TypedTrees,
    argument: omega_typed_trees::types::TypeReferenceHandle,
    expected: &str,
) -> Result<String, Diagnostic> {
    let omega_typed_trees::types::TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
    else {
        return Err(Diagnostic::error(
            "closed indexed-domain arguments must be canonical const values or direct const binders",
        ));
    };
    if let Some(value) = omega_core::const_value::CanonicalConstValue::from_atom(name.as_str()) {
        if value.type_name != expected {
            return Err(Diagnostic::error(format!(
                "indexed-domain argument has canonical type `{}`, expected `{expected}`",
                value.type_name
            )));
        }
        return Ok(format!(
            "const:{}:{}:{}",
            value.type_name,
            value.encoding.len(),
            value.encoding
        ));
    }
    if name.as_str().parse::<i128>().is_ok() {
        return Ok(format!("integer:{expected}:{}", name.as_str()));
    }
    // A direct generic const binder remains open only until ordinary machine
    // specialization. It is not PDI3's computed expression surface.
    Ok(format!(
        "binder:{}:{}",
        if symbol.is_valid() {
            program.symbols.display_path(*symbol, "::")
        } else {
            name.as_str().to_owned()
        },
        expected
    ))
}
