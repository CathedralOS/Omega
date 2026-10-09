//! Elaborated field views of data applications, not new nominal declarations.
//!
//! A caller's `Envelope<Selected<'storage>>` is closed for storage even though
//! syntax-level synthesis cannot select its lexical regions. Retain the exact
//! application and substitute its fields here, while typed arenas are owned.
//! Checking reads those fields without rebuilding types or borrowing one use's
//! region spelling from a layout-equivalent use. Source admission still checks
//! the original declaration and every authored application; this derivation
//! grants no bounds, loan, constructor or predicate authority.

use arena::HandleSpan;
use symbols::SymbolHandle;
use symbols::SymbolKind;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::name::Identifier;
use typed_trees::types::FixedArrayLength;
use typed_trees::types::TypeConstraintNode;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::types::TypeReferenceNode;

pub(super) fn elaborate(program: &mut TypedTrees) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let type_start = program.type_reference_table.type_reference_count();
    let mut pending = program
        .type_reference_table
        .type_reference_rows()
        .map(|(reference, _)| reference)
        .collect::<Vec<_>>();
    let mut next = 0;
    while let Some(&reference) = pending.get(next) {
        next += 1;
        let before = program.type_reference_table.type_reference_count();
        elaborate_application(program, reference);
        if program.type_reference_table.type_reference_count() > before {
            pending.extend(
                program
                    .type_reference_table
                    .type_reference_rows()
                    .filter(|(reference, _)| reference.arena_index() as usize > before)
                    .map(|(reference, _)| reference),
            );
        }
    }
    let produced = program
        .type_reference_table
        .type_reference_rows()
        .filter(|(reference, _)| reference.arena_index() as usize > type_start)
        .map(|(reference, _)| reference)
        .collect::<Vec<_>>();
    crate::monomorphization::finalize_type_domain_instance_identities(program, produced)
        .map_err(|diagnostic| vec![diagnostic])
}

fn elaborate_application(program: &mut TypedTrees, reference: TypeReferenceHandle) {
    if program.data_application_definition(reference).is_some() {
        return;
    }
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = program
        .type_reference_table
        .type_reference(reference)
        .clone()
    else {
        return;
    };
    let Some(template) = program
        .unique_data_definition_by_symbol(base_symbol)
        .cloned()
    else {
        return;
    };
    let parameters = program.data_type_parameters(&template).to_vec();
    let arguments = program
        .type_reference_table
        .type_reference_handles(arguments)
        .to_vec();
    if parameters.len() != arguments.len()
        || template.lifetime_parameters.len() != lifetime_arguments.len()
        || arguments
            .iter()
            .any(|argument| !closed_type(program, *argument))
    {
        return;
    }
    // Const evaluation can already have retained a declaration with exact
    // extents and constructor facts. Read that result; do not evaluate it again.
    if let Some(instance) = program.closed_data_definition(reference).cloned()
        && instance.generic_instance.is_some()
    {
        program.retain_data_application_definition(reference, instance);
        return;
    }
    let substitutions = parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (parameter.symbol, argument))
        .collect::<Vec<_>>();
    let lifetimes = template
        .lifetime_parameters
        .iter()
        .cloned()
        .zip(lifetime_arguments)
        .collect::<Vec<_>>();
    let members = program.data_members(&template).to_vec();
    let mut instantiated = Vec::with_capacity(members.len());
    for member in members {
        let member = match member {
            DataMember::Field(mut field) => {
                field.type_reference =
                    instantiate_type(program, field.type_reference, &substitutions, &lifetimes);
                if !closed_type(program, field.type_reference) {
                    return;
                }
                DataMember::Field(field)
            }
            DataMember::Variant(mut variant) => {
                let mut fields = program.data_payload_fields(&variant).to_vec();
                for field in &mut fields {
                    field.type_reference =
                        instantiate_type(program, field.type_reference, &substitutions, &lifetimes);
                    if !closed_type(program, field.type_reference) {
                        return;
                    }
                }
                variant.payload = program.data_payload_fields.insert_many(fields);
                DataMember::Variant(variant)
            }
        };
        instantiated.push(member);
    }
    let mut application = template;
    application.members = program.data_members.insert_many(instantiated);
    application.type_parameters = HandleSpan::empty();
    application.lifetime_parameters.clear();
    application.generic_instance = Some(reference);
    program.retain_data_application_definition(reference, application);
}

/// This is closure, not admission: unknown/open operands retain no closed view.
fn closed_type(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    let mut pending = vec![reference];
    while let Some(reference) = pending.pop() {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return false;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                if symbol.is_valid()
                    && matches!(
                        program.symbols.get(*symbol).kind,
                        SymbolKind::TypeParameter | SymbolKind::Parameter | SymbolKind::Local
                    )
                {
                    return false;
                }
                if let Some(data) = program.unique_data_definition_by_symbol(*symbol)
                    && !program.data_type_parameters(data).is_empty()
                {
                    return false;
                }
            }
            TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                pending.push(*base_type);
                for constraint in program.type_reference_table.constraints(*constraints) {
                    match constraint {
                        TypeConstraintNode::Domain(domain) => {
                            pending.extend(domain.arguments.iter().copied())
                        }
                        TypeConstraintNode::Range {
                            minimum, maximum, ..
                        } if !closed_expression(program, *minimum)
                            || !closed_expression(program, *maximum) =>
                        {
                            return false;
                        }
                        _ => {}
                    }
                }
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                if !matches!(length, FixedArrayLength::Literal(_)) {
                    return false;
                }
                pending.push(*element_type);
            }
            TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
            TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } => {
                let Some(data) = program.unique_data_definition_by_symbol(*base_symbol) else {
                    return false;
                };
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                if program.data_type_parameters(data).len() != arguments.len()
                    || data.lifetime_parameters.len() != lifetime_arguments.len()
                {
                    return false;
                }
                pending.extend(arguments.iter().copied());
            }
            TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::DynamicTrait { .. } => {
                return false;
            }
            TypeReferenceNode::Unit => {}
        }
    }
    true
}

fn closed_expression(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let mut expressions = Vec::new();
    validation::collect_expression_tree(program, expression, &mut expressions);
    !expressions.iter().any(|expression| {
        matches!(program.expression_table.expression(*expression),
            typed_trees::expression::ExpressionNode::Name(path)
                if path.symbol.is_valid() && matches!(program.symbols.get(path.symbol).kind,
                    SymbolKind::TypeParameter | SymbolKind::Parameter | SymbolKind::Local))
    })
}

fn instantiate_type(
    program: &mut TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    lifetimes: &[(Identifier, Identifier)],
) -> TypeReferenceHandle {
    let original = program
        .type_reference_table
        .type_reference(reference)
        .clone();
    let instantiated = match original.clone() {
        TypeReferenceNode::Named { symbol, .. } => {
            return substitutions
                .iter()
                .find_map(|(parameter, actual)| (*parameter == symbol).then_some(*actual))
                .unwrap_or(reference);
        }
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => TypeReferenceNode::Reference {
            referee: instantiate_type(program, referee, substitutions, lifetimes),
            access,
            lifetime: lifetime.map(|lifetime| instantiate_lifetime(lifetime, lifetimes)),
        },
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            let arguments = program
                .type_reference_table
                .type_reference_handles(arguments)
                .to_vec();
            let arguments = arguments
                .into_iter()
                .map(|argument| instantiate_type(program, argument, substitutions, lifetimes))
                .collect::<Vec<_>>();
            let lifetime_arguments = lifetime_arguments
                .into_iter()
                .map(|lifetime| instantiate_lifetime(lifetime, lifetimes))
                .collect::<Vec<_>>();
            // Reuse exact child handles, not carrier identity. Besides avoiding
            // copies, this closes recursive data references without expanding
            // a fresh equivalent application at every field traversal.
            if let Some((existing, _)) =
                program
                    .type_reference_table
                    .type_reference_rows()
                    .find(|(_, node)| {
                        matches!(node, TypeReferenceNode::Generic {
                    base_symbol: candidate, lifetime_arguments: regions, arguments: children, ..
                } if *candidate == base_symbol && *regions == lifetime_arguments
                    && program.type_reference_table.type_reference_handles(*children) == arguments)
                    })
            {
                return existing;
            }
            TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments: program
                    .type_reference_table
                    .insert_type_reference_handles(arguments),
            }
        }
        TypeReferenceNode::Slice { element_type } => TypeReferenceNode::Slice {
            element_type: instantiate_type(program, element_type, substitutions, lifetimes),
        },
        TypeReferenceNode::FixedArray {
            element_type,
            mut length,
        } => {
            if let FixedArrayLength::ConstParameter { symbol, .. } = &length
                && let Some((_, actual)) = substitutions
                    .iter()
                    .find(|(parameter, _)| parameter == symbol)
                && let TypeReferenceNode::Named { symbol, name } =
                    program.type_reference_table.type_reference(*actual)
                && !symbol.is_valid()
                && let Ok(value) = name.as_str().parse::<usize>()
            {
                length = FixedArrayLength::Literal(value);
            }
            TypeReferenceNode::FixedArray {
                element_type: instantiate_type(program, element_type, substitutions, lifetimes),
                length,
            }
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let base_type = instantiate_type(program, base_type, substitutions, lifetimes);
            let mut constraints = program
                .type_reference_table
                .constraints(constraints)
                .to_vec();
            for constraint in &mut constraints {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    domain.arguments = domain
                        .arguments
                        .iter()
                        .map(|argument| {
                            instantiate_type(program, *argument, substitutions, lifetimes)
                        })
                        .collect();
                }
            }
            TypeReferenceNode::Constrained {
                base_type,
                constraints: program.type_reference_table.insert_constraints(constraints),
            }
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => return reference,
    };
    if instantiated == original {
        return reference;
    }
    let Ok(origins) = program
        .type_reference_table
        .constant_origins(reference)
        .map(<[_]>::to_vec)
    else {
        return TypeReferenceHandle::invalid();
    };
    let result = program.type_reference_table.insert(instantiated);
    if program
        .type_reference_table
        .attach_constant_origins(result, origins)
        .is_err()
    {
        return TypeReferenceHandle::invalid();
    }
    result
}

fn instantiate_lifetime(
    lifetime: Identifier,
    lifetimes: &[(Identifier, Identifier)],
) -> Identifier {
    lifetimes
        .iter()
        .find_map(|(parameter, actual)| (*parameter == lifetime).then(|| actual.clone()))
        .unwrap_or(lifetime)
}
