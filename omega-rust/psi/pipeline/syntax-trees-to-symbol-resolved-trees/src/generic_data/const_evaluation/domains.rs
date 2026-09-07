//! Constant evaluation: domains.

use super::*;

pub(in crate::generic_data) fn integer_literal_value(value: &IntegerLiteral) -> Option<i128> {
    value
        .value_i64()
        .map(i128::from)
        .or_else(|| value.value_u64().map(i128::from))
}

pub(in crate::generic_data) fn qualified_const_name(definition: &ConstDefinition) -> String {
    if definition.scope.as_str().is_empty() {
        definition.name.as_str().to_owned()
    } else {
        format!(
            "{}::{}",
            definition.scope.as_str(),
            definition.name.as_str()
        )
    }
}

#[derive(Clone)]
pub(in crate::generic_data) struct ClosedDomainFamily {
    parameters: Vec<ClosedDomainParameter>,
}

#[derive(Clone)]
enum ClosedDomainParameter {
    Type {
        name: String,
    },
    Const {
        name: String,
        type_reference: TypeReferenceHandle,
    },
}

/// PDI2's closed-index precursor runs beside generic-data canonicalization but
/// does not monomorphize the domain: the family remains nominal and erased.
/// Only its const arguments are rewritten to the same canonical leaves used by
/// PDI1 generic identity.
pub(in crate::generic_data) fn canonicalize_closed_domain_indices(
    syntax: &mut SyntaxTrees,
    const_definitions: &HashMap<String, ConstDefinition>,
    const_values: &HashMap<String, i128>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(), Diagnostic> {
    let mut families = HashMap::<String, ClosedDomainFamily>::new();

    for item in syntax.root_items() {
        let Item::Domain(definition) = item else {
            continue;
        };
        let parameters = syntax
            .tables
            .items
            .type_parameters(definition.type_parameters);
        let header_arguments = syntax
            .tables
            .type_references
            .type_reference_handles(definition.index_arguments);

        if parameters.is_empty() {
            if !header_arguments.is_empty() {
                return Err(Diagnostic::error(format!(
                    "domain `{}` supplies index arguments but declares no generic carrier/const telescope",
                    definition.name
                )));
            }
            continue;
        }

        let TypeReferenceNode::Named(target) = syntax
            .tables
            .type_references
            .type_reference(definition.target_type)
        else {
            return Err(Diagnostic::error(format!(
                "indexed domain `{}` must use its carrier binder directly before `::{}`",
                definition.name, definition.name
            )));
        };
        let generic_carrier = parameters.first().is_some_and(|parameter| {
            matches!(parameter.kind, TypeParameterKind::Type)
                && target.as_str() == parameter.name.as_str()
        });
        let index_parameters = if generic_carrier {
            &parameters[1..]
        } else {
            parameters
        };
        if index_parameters.is_empty() {
            // A carrier-polymorphic, unindexed domain (`domain<T> T::D`)
            // needs no closed-family canonicalization. Its exact declaration
            // symbol is the complete nominal identity retained downstream.
            continue;
        }
        let mut family_parameters = Vec::with_capacity(index_parameters.len());
        for parameter in index_parameters {
            match parameter.kind {
                TypeParameterKind::Type => family_parameters.push(ClosedDomainParameter::Type {
                    name: parameter.name.as_str().to_owned(),
                }),
                TypeParameterKind::Const { type_reference } => {
                    validate_const_index_type(syntax, type_reference, &mut HashSet::new())
                        .map_err(|reason| {
                            Diagnostic::error(format!(
                                "indexed domain `{}::{}` has an ineligible index type: {reason}",
                                definition.name, parameter.name
                            ))
                        })?;
                    family_parameters.push(ClosedDomainParameter::Const {
                        name: parameter.name.as_str().to_owned(),
                        type_reference,
                    });
                }
                _ => {
                    return Err(Diagnostic::error(format!(
                        "indexed domain `{}` indices must be type or proof-static const parameters",
                        definition.name
                    )));
                }
            }
        }
        if header_arguments.len() != family_parameters.len() {
            return Err(Diagnostic::error(format!(
                "indexed domain `{}` declares {} index parameter(s) but selects {} index argument(s) in its family header",
                definition.name,
                family_parameters.len(),
                header_arguments.len()
            )));
        }
        for (parameter, argument) in family_parameters.iter().zip(header_arguments) {
            let parameter_name = match parameter {
                ClosedDomainParameter::Type { name }
                | ClosedDomainParameter::Const { name, .. } => name,
            };
            let TypeReferenceNode::Named(argument_name) =
                syntax.tables.type_references.type_reference(*argument)
            else {
                return Err(Diagnostic::error(format!(
                    "indexed domain `{}` must select each index binder directly in its family header",
                    definition.name
                )));
            };
            if argument_name.as_str() != parameter_name {
                return Err(Diagnostic::error(format!(
                    "indexed domain `{}` must select index binder `{parameter_name}` in declaration order, not `{argument_name}`",
                    definition.name
                )));
            }
        }
        if families
            .insert(
                definition.name.as_str().to_owned(),
                ClosedDomainFamily {
                    parameters: family_parameters,
                },
            )
            .is_some()
        {
            return Err(Diagnostic::error(format!(
                "indexed domain family `{}` is declared more than once",
                definition.name
            )));
        }
    }

    let mut applications = syntax
        .tables
        .type_references
        .domain_constraints()
        .into_iter()
        .map(|constraint| (constraint.name.as_str().to_owned(), constraint.arguments))
        .collect::<Vec<_>>();
    applications.extend(
        syntax
            .expressions
            .iter_expressions()
            .filter_map(|(_, expression)| {
                let ExpressionNode::Cast(cast) = expression else {
                    return None;
                };
                if cast.semantic_domain.is_empty() {
                    return None;
                }
                let name = syntax
                    .expressions
                    .identifier_path_members(cast.semantic_domain)
                    .iter()
                    .map(|member| member.as_str())
                    .collect::<Vec<_>>()
                    .join("::");
                Some((name, cast.semantic_domain_arguments))
            }),
    );

    for (name, argument_span) in applications {
        let Some(family) = families.get(&name) else {
            continue;
        };
        let arguments = syntax
            .tables
            .type_references
            .type_reference_handles(argument_span)
            .to_vec();
        canonicalize_closed_domain_application(
            syntax,
            &name,
            family,
            arguments,
            const_definitions,
            const_values,
            warnings,
        )?;
    }
    Ok(())
}

pub(in crate::generic_data) fn canonicalize_closed_domain_application(
    syntax: &mut SyntaxTrees,
    family_name: &str,
    family: &ClosedDomainFamily,
    arguments: Vec<TypeReferenceHandle>,
    const_definitions: &HashMap<String, ConstDefinition>,
    const_values: &HashMap<String, i128>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(), Diagnostic> {
    if arguments.len() != family.parameters.len() {
        return Err(Diagnostic::error(format!(
            "indexed domain `{}` requires {} closed index argument(s), but {} were supplied",
            family_name,
            family.parameters.len(),
            arguments.len()
        )));
    }
    for (parameter, argument) in family.parameters.iter().zip(arguments) {
        let ClosedDomainParameter::Const {
            name: parameter_name,
            type_reference: parameter_type,
        } = parameter
        else {
            continue;
        };
        let node = syntax
            .tables
            .type_references
            .type_reference(argument)
            .clone();
        match node {
            TypeReferenceNode::Named(name) => {
                if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
                    let required =
                        syntax_type_identity(syntax, *parameter_type).map_err(Diagnostic::error)?;
                    if value.type_name != required {
                        return Err(Diagnostic::error(format!(
                            "index argument for `{}::{parameter_name}` has canonical type `{}`, expected `{required}`",
                            family_name, value.type_name
                        )));
                    }
                    continue;
                }
                if let Some(value) = const_values.get(name.as_str()) {
                    syntax.tables.type_references.replace_type_reference(
                        argument,
                        TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                    );
                    continue;
                }
                let Some(definition) = const_definitions.get(name.as_str()) else {
                    // A direct generic const binder is resolved and checked
                    // later in its declaration context. Unknown names fail
                    // there as well; never guess that a type is a value.
                    continue;
                };
                let value = canonicalize_const_definition(syntax, definition, *parameter_type)
                    .map_err(|reason| {
                        Diagnostic::error(format!(
                            "index argument for `{}::{parameter_name}` is invalid: {reason}",
                            family_name
                        ))
                    })?;
                syntax.tables.type_references.replace_type_reference(
                    argument,
                    TypeReferenceNode::Named(Identifier::generated(value.atom())),
                );
            }
            TypeReferenceNode::ConstExpression(expression) => {
                // PDI3 open indexed-domain expressions must survive this
                // pre-resolution pass so binder names and selected operators
                // can acquire exact symbols later. Closed integer arithmetic
                // keeps the existing eager fold and diagnostics.
                if const_expression_contains_name(syntax, expression) {
                    continue;
                }
                let destination =
                    syntax_type_identity(syntax, *parameter_type).map_err(Diagnostic::error)?;
                let value = evaluate_const_argument_expression(
                    syntax,
                    expression,
                    const_values,
                    &HashMap::new(),
                    &HashSet::new(),
                    const_integer_type(syntax, *parameter_type),
                    warnings,
                )
                .and_then(EvaluatedConst::into_concrete)
                .map_err(|reason| {
                    Diagnostic::error(format!(
                        "index argument expression for `{}` is invalid for `{destination}`: {reason}",
                        family_name
                    )).with_source_span(syntax.expressions.source_span(expression))
                })?;
                if const_integer_type(syntax, *parameter_type).is_some() {
                    let required =
                        syntax_type_identity(syntax, *parameter_type).map_err(Diagnostic::error)?;
                    validate_syntax_integer_range(&required, value).map_err(Diagnostic::error)?;
                }
                syntax.tables.type_references.replace_type_reference(
                    argument,
                    TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                );
            }
            _ => {
                return Err(Diagnostic::error(format!(
                    "indexed domain `{}::{parameter_name}` requires a closed const value or direct const binder",
                    family_name
                )));
            }
        }
    }
    Ok(())
}

pub(in crate::generic_data) fn const_expression_contains_name(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
) -> bool {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Name(_) => true,
        ExpressionNode::Binary(binary) => {
            const_expression_contains_name(syntax, binary.left)
                || const_expression_contains_name(syntax, binary.right)
        }
        ExpressionNode::Unary(unary) => const_expression_contains_name(syntax, unary.operand),
        _ => false,
    }
}
