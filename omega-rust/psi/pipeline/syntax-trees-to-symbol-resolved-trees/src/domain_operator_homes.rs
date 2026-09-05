use arena::{Arena, HandleSpan, OrderedRootArena};
use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;

/// Move an ordinary top-level operator into its exact domain's semantic
/// operator family before symbols are assigned. The home is supplied either
/// by a qualified declaration name (`operator Quantity::Additive::add ...`)
/// or by one unique declared-domain constraint across the operand tuple
/// (`operator add(left: i32::Degrees, ...)`).
///
/// The declaration remains an ordinary root item in source. This one lowering
/// point owns the semantic association; domain bodies are reserved for exact
/// establishment requirements and no checked consumer reconstructs ownership
/// from an operator name later.
pub(crate) fn normalize_domain_operator_homes(
    program: &mut SymbolResolvedTrees,
) -> Result<(), Diagnostic> {
    let domain_names = program
        .domain_definitions
        .iter()
        .map(|domain| domain.name.as_str().to_owned())
        .collect::<Vec<_>>();
    let mut operators_by_domain = program
        .domain_definitions
        .iter()
        .map(|domain| program.operator_definitions(domain.operators).to_vec())
        .collect::<Vec<_>>();

    let authored_roots = std::mem::take(&mut program.operators);
    let mut remaining_roots = OrderedRootArena::new();

    for operator in &authored_roots {
        let path = program.operator_path_members(operator.name).to_vec();
        let Some((leaf, owner_path)) = path.split_last() else {
            remaining_roots.push(operator.clone());
            continue;
        };
        let explicit_owner = owner_path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let explicit_matches = domain_names
            .iter()
            .enumerate()
            .filter(|(_, domain)| !explicit_owner.is_empty() && domain.as_str() == explicit_owner)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let inferred_matches = inferred_domain_homes(program, operator, &domain_names);
        let matches = if explicit_matches.is_empty() {
            inferred_matches
        } else if inferred_matches.is_empty() {
            explicit_matches
        } else {
            explicit_matches
                .into_iter()
                .filter(|candidate| inferred_matches.contains(candidate))
                .collect()
        };

        let [domain_index] = matches.as_slice() else {
            if matches.is_empty() {
                if !inferred_domain_homes(program, operator, &domain_names).is_empty() {
                    return Err(Diagnostic::error(format!(
                        "operator `{}` names a domain home that conflicts with its operand domains",
                        operator_label(program, operator)
                    )));
                }
                remaining_roots.push(operator.clone());
                continue;
            }
            return Err(Diagnostic::error(format!(
                "operator `{}` has more than one possible domain home; use an exact \
                 `operator Type::Domain::operation ...` declaration",
                operator_label(program, operator)
            )));
        };

        let leaf = program
            .tables
            .declarations
            .operator_path_members
            .append(leaf.clone());
        let mut operator = operator.clone();
        operator.name = HandleSpan::from_parts(leaf, 1);
        operators_by_domain[*domain_index].push(operator);
    }

    let mut rebuilt = Arena::new();
    let mut domain_index = 0usize;
    program.domain_definitions.for_each_mut(|domain| {
        let mut operators = HandleSpan::empty();
        for operator in operators_by_domain[domain_index].drain(..) {
            rebuilt.append_to_span(&mut operators, operator);
        }
        domain.operators = operators;
        if !operators.is_empty() {
            domain.semantic_roles.denotation_dimension = Some(domain.semantic_id);
        }
        domain_index += 1;
    });

    program.tables.declarations.operator_definitions = rebuilt;
    program.operators = remaining_roots;
    Ok(())
}

fn inferred_domain_homes(
    program: &SymbolResolvedTrees,
    operator: &symbol_resolved_trees::operator::OperatorDefinition,
    domain_names: &[String],
) -> Vec<usize> {
    let mut matches = Vec::new();
    for parameter in program.state_parameters(operator.parameters) {
        collect_type_domain_homes(
            program,
            &parameter.type_reference,
            domain_names,
            &mut matches,
        );
    }
    matches
}

fn collect_type_domain_homes(
    program: &SymbolResolvedTrees,
    type_reference: &symbol_resolved_trees::types::TypeReference,
    domain_names: &[String],
    matches: &mut Vec<usize>,
) {
    use symbol_resolved_trees::types::{TypeConstraint, TypeReference};

    match type_reference {
        TypeReference::Reference(reference) => collect_type_domain_homes(
            program,
            program.child_type_reference(reference.referee),
            domain_names,
            matches,
        ),
        TypeReference::Constrained(constrained) => {
            let carrier = program.child_type_reference(constrained.base_type);
            for constraint in program
                .tables
                .types
                .constraints
                .span_or_empty(constrained.constraints)
            {
                let TypeConstraint::Domain(authored) = constraint else {
                    continue;
                };
                for (index, domain) in program.domain_definitions.iter().enumerate() {
                    let full = &domain_names[index];
                    if (full == authored.name.as_str()
                        || full.rsplit("::").next() == Some(authored.name.as_str()))
                        && domain_accepts_carrier(
                            program,
                            domain,
                            carrier,
                            authored.arguments.len(),
                        )
                        && !matches.contains(&index)
                    {
                        matches.push(index);
                    }
                }
            }
            collect_type_domain_homes(program, carrier, domain_names, matches);
        }
        TypeReference::FixedArray(_)
        | TypeReference::Slice(_)
        | TypeReference::Generic(_)
        | TypeReference::ConstExpression(_)
        | TypeReference::DynamicTrait { .. }
        | TypeReference::Named { .. }
        | TypeReference::SelfType { .. }
        | TypeReference::Unit => {}
    }
}

fn domain_accepts_carrier(
    program: &SymbolResolvedTrees,
    domain: &symbol_resolved_trees::domain::DomainDefinition,
    carrier: &symbol_resolved_trees::types::TypeReference,
    argument_count: usize,
) -> bool {
    let parameters = program.data_type_parameters(domain.type_parameters);
    if parameters.is_empty() {
        return argument_count == 0 && type_references_match(program, carrier, &domain.target_type);
    }
    let Some(parameter) = parameters.first() else {
        return false;
    };
    if !matches!(
        parameter.kind,
        symbol_resolved_trees::data::TypeParameterKind::Type
    ) || argument_count != parameters.len().saturating_sub(1)
    {
        return false;
    }
    matches!(
        &domain.target_type,
        symbol_resolved_trees::types::TypeReference::Named { name, .. }
            if name.as_str() == parameter.name.as_str()
    )
}

fn type_references_match(
    program: &SymbolResolvedTrees,
    left: &symbol_resolved_trees::types::TypeReference,
    right: &symbol_resolved_trees::types::TypeReference,
) -> bool {
    use symbol_resolved_trees::types::TypeReference;

    match (left, right) {
        (TypeReference::Constrained(left), _) => {
            type_references_match(program, program.child_type_reference(left.base_type), right)
        }
        (_, TypeReference::Constrained(right)) => {
            type_references_match(program, left, program.child_type_reference(right.base_type))
        }
        (TypeReference::Reference(left), TypeReference::Reference(right)) => {
            left.access == right.access
                && type_references_match(
                    program,
                    program.child_type_reference(left.referee),
                    program.child_type_reference(right.referee),
                )
        }
        (TypeReference::FixedArray(left), TypeReference::FixedArray(right)) => {
            left.length == right.length
                && type_references_match(
                    program,
                    program.child_type_reference(left.element_type),
                    program.child_type_reference(right.element_type),
                )
        }
        (TypeReference::Slice(left), TypeReference::Slice(right)) => type_references_match(
            program,
            program.child_type_reference(left.element_type),
            program.child_type_reference(right.element_type),
        ),
        (TypeReference::Generic(left), TypeReference::Generic(right)) => {
            left.base_name == right.base_name
                && left.lifetime_arguments == right.lifetime_arguments
                && program
                    .child_type_references(left.arguments)
                    .iter()
                    .zip(program.child_type_references(right.arguments))
                    .all(|(left, right)| type_references_match(program, left, right))
                && left.arguments.len() == right.arguments.len()
        }
        (
            TypeReference::DynamicTrait { name: left, .. },
            TypeReference::DynamicTrait { name: right, .. },
        )
        | (TypeReference::Named { name: left, .. }, TypeReference::Named { name: right, .. }) => {
            left == right
        }
        (TypeReference::SelfType { symbol: left }, TypeReference::SelfType { symbol: right }) => {
            left == right
        }
        (TypeReference::Unit, TypeReference::Unit) => true,
        _ => false,
    }
}

fn operator_label(
    program: &SymbolResolvedTrees,
    operator: &symbol_resolved_trees::operator::OperatorDefinition,
) -> String {
    program
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}
