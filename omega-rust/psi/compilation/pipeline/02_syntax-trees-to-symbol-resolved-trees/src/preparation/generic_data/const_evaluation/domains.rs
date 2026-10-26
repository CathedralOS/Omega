//! Constant evaluation: domains.
use super::super::{
    CanonicalConstValue, ConstDefinition, Diagnostic, ExpressionNode, HashMap, HashSet, Identifier,
    IntegerLiteral, SyntaxTrees, TypeParameterKind, TypeReferenceHandle, TypeReferenceNode,
};
use syntax_trees::identifier::JoinedSyntaxPath;

use crate::preparation::generic_data::ConstScalarSpelling;
use crate::preparation::generic_data::EvaluatedConst;
use crate::preparation::generic_data::canonicalize_const_definition;
use crate::preparation::generic_data::canonicalize_selected_const_definition;
use crate::preparation::generic_data::collect_data_type_reference_positions;
use crate::preparation::generic_data::const_integer_type;
use crate::preparation::generic_data::evaluate_const_argument_expression;
use crate::preparation::generic_data::syntax_type_identity;
use crate::preparation::generic_data::validate_const_index_type;
use crate::preparation::generic_data::validate_syntax_integer_range;
use crate::preparation::generic_data::work_domain::TypePositionPurpose;

pub(in crate::preparation::generic_data) fn integer_literal_value(
    value: &IntegerLiteral,
) -> Option<i128> {
    value
        .value_i64()
        .map(i128::from)
        .or_else(|| value.value_u64().map(i128::from))
}

pub(in crate::preparation::generic_data) fn qualified_const_name(
    syntax_trees: &SyntaxTrees,
    definition: &ConstDefinition,
) -> String {
    crate::constant::semantic_const_name(syntax_trees, definition)
}

#[derive(Clone)]
pub(in crate::preparation::generic_data) struct ClosedDomainFamily {
    path: String,
    declaration: Identifier,
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

/// The family's index telescope, excluding only a directly named first
/// type binder used as its carrier. Fixed carriers retain every parameter.
pub(in crate::preparation::generic_data) fn domain_index_parameters<'syntax>(
    syntax: &'syntax SyntaxTrees,
    definition: &syntax_trees::item::DomainDefinition,
) -> Option<&'syntax [syntax_trees::item::TypeParameter]> {
    // A non-Named target (for example a fixed-array carrier) can never name a
    // leading type parameter, so the whole telescope is index parameters.
    let target_name = match syntax
        .type_references
        .type_reference(definition.target_type)
    {
        TypeReferenceNode::Named(target) => Some(target.as_str(syntax)),
        _ => None,
    };
    let parameters = syntax.items.type_parameters(definition.type_parameters);
    let generic_carrier = parameters.first().is_some_and(|parameter| {
        matches!(parameter.kind, TypeParameterKind::Type)
            && target_name == Some(parameter.name.as_str(syntax))
    });
    Some(if generic_carrier {
        &parameters[1..]
    } else {
        parameters
    })
}

/// PDI2's closed-index precursor runs beside generic-data canonicalization but
/// does not monomorphize the domain: the family remains nominal and erased.
/// Only its const arguments are rewritten to the same canonical leaves used by
/// PDI1 generic identity.
pub(in crate::preparation::generic_data) fn canonicalize_closed_domain_indices(
    syntax: &mut SyntaxTrees,
    const_definitions: &HashMap<String, ConstDefinition>,
    const_values: &HashMap<String, i128>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    warnings: &mut Vec<Diagnostic>,
    work: &super::super::work_domain::SynthesisWork<'_>,
) -> Result<(), Diagnostic> {
    let mut families = Vec::<ClosedDomainFamily>::new();

    for item in syntax.root_items() {
        let Some(definition) = item.as_domain() else {
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
                    definition.name.as_str(syntax)
                )));
            }
            continue;
        }

        let Some(index_parameters) = domain_index_parameters(syntax, definition) else {
            return Err(Diagnostic::error(format!(
                "indexed domain `{}` must use its carrier binder directly before `::{}`",
                definition.name.as_str(syntax),
                definition.name.as_str(syntax)
            )));
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
                    name: parameter.name.as_str(syntax).to_owned(),
                }),
                TypeParameterKind::Const { type_reference } => {
                    validate_const_index_type(syntax, type_reference, &mut HashSet::new())
                        .map_err(|reason| {
                            Diagnostic::error(format!(
                                "indexed domain `{}::{}` has an ineligible index type: {reason}",
                                definition.name.as_str(syntax),
                                parameter.name.as_str(syntax)
                            ))
                        })?;
                    family_parameters.push(ClosedDomainParameter::Const {
                        name: parameter.name.as_str(syntax).to_owned(),
                        type_reference,
                    });
                }
                _ => {
                    return Err(Diagnostic::error(format!(
                        "indexed domain `{}` indices must be type or proof-static const parameters",
                        definition.name.as_str(syntax)
                    )));
                }
            }
        }
        if header_arguments.len() != family_parameters.len() {
            return Err(Diagnostic::error(format!(
                "indexed domain `{}` declares {} index parameter(s) but selects {} index argument(s) in its family header",
                definition.name.as_str(syntax),
                family_parameters.len(),
                header_arguments.len()
            )));
        }
        for (parameter, argument) in family_parameters.iter().zip(header_arguments) {
            let parameter_name = match parameter {
                ClosedDomainParameter::Type { name }
                | ClosedDomainParameter::Const { name, .. } => name,
            };
            let Some(argument_name) = syntax
                .tables
                .type_references
                .type_reference(*argument)
                .as_named()
            else {
                return Err(Diagnostic::error(format!(
                    "indexed domain `{}` must select each index binder directly in its family header",
                    definition.name.as_str(syntax)
                )));
            };
            if argument_name.as_str(syntax) != parameter_name {
                return Err(Diagnostic::error(format!(
                    "indexed domain `{}` must select index binder `{parameter_name}` in declaration order, not `{argument_name}`",
                    definition.name.as_str(syntax),
                    argument_name = argument_name.as_str(syntax)
                )));
            }
        }
        // The logical path is unique only within a checked package. Retain
        // declaration custody for the selector's exact family join below;
        // rendering its path again would collapse equal foreign paths.
        let family_path = match crate::preparation::generic_data::module_constants::module_path(
            syntax,
            definition.name.source_span().source_id,
        ) {
            Some(module) => format!("{module}::{}", definition.name.as_str(syntax)),
            None => definition.name.as_str(syntax).to_owned(),
        };
        let declaration = definition.name.source_span();
        if families.iter().any(|family| {
            family.path == family_path
                && selection.is_none_or(|selection| {
                    selection.same_checked_package(family.declaration.source_span(), declaration)
                })
        }) {
            return Err(Diagnostic::error(format!(
                "indexed domain family `{}` is declared more than once",
                definition.name.as_str(syntax)
            )));
        }
        families.push(ClosedDomainFamily {
            path: family_path,
            declaration: definition.name.clone(),
            parameters: family_parameters,
        });
    }

    let constraints = if work.is_all() {
        syntax.tables.type_references.domain_constraints()
    } else {
        work.type_positions(syntax, TypePositionPurpose::Identity, true)
            .into_iter()
            .flat_map(|reference| {
                let TypeReferenceNode::Constrained { constraints, .. } =
                    syntax.type_references.type_reference(reference)
                else {
                    return Vec::new();
                };
                syntax
                    .type_references
                    .constraints(*constraints)
                    .iter()
                    .filter_map(|constraint| {
                        if let syntax_trees::types::TypeConstraintNode::Domain(domain) = constraint
                        {
                            Some(domain.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    let declarations = work.declarations(syntax);
    let expressions =
        super::super::uses::selected_machine_expression_handles(syntax, &declarations);
    let mut applications = constraints
        .into_iter()
        .map(|constraint| {
            (
                constraint.name.as_str(syntax).to_owned(),
                constraint.arguments,
                constraint.name.source_span(),
            )
        })
        .collect::<Vec<_>>();
    applications.extend(syntax.expressions.iter_expressions().filter_map(
        |(handle, expression)| {
            if !work.is_all() && !work.owns_new_expression(handle) && !expressions.contains(handle)
            {
                return None;
            }
            let ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            if cast.semantic_domain.is_empty() {
                return None;
            }
            let members = syntax
                .expressions
                .identifier_path_members(cast.semantic_domain);
            let name = members.joined_path_in(syntax);
            let mut reference = members.first()?.source_span();
            for member in members.iter().skip(1) {
                if member.source_span().source_id != reference.source_id {
                    break;
                }
                reference.span.end = member.source_span().span.end;
            }
            Some((name, cast.semantic_domain_arguments, reference))
        },
    ));

    let concrete_data_positions = collect_data_type_reference_positions(syntax, false);
    for (name, argument_span, reference) in applications {
        // The application selects its family by module name law before any
        // argument folds: a qualified path names its exact owner, a
        // module-local family outranks same-leaf foreign or root candidates,
        // and a leaf spelling reaches a foreign family only through a narrow
        // import of the exact declaration. Unreachable, contested, or
        // non-generic owners keep the authored arguments for ordinary
        // resolution rather than borrowing another declaration's telescope.
        let family = match selection {
            Some(selection) => selection
                .domain_family(syntax, &name, reference, None)
                .and_then(|(_, definition)| {
                    families.iter().find(|family| {
                        family.declaration.source_span() == definition.name.source_span()
                            && family.declaration.as_str(syntax) == definition.name.as_str(syntax)
                    })
                }),
            None => families.iter().find(|family| family.path == name),
        };
        let Some(family) = family else {
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
            selection,
            &concrete_data_positions,
            warnings,
        )?;
    }
    Ok(())
}

pub(in crate::preparation::generic_data) fn canonicalize_closed_domain_application(
    syntax: &mut SyntaxTrees,
    family_name: &str,
    family: &ClosedDomainFamily,
    arguments: Vec<TypeReferenceHandle>,
    const_definitions: &HashMap<String, ConstDefinition>,
    const_values: &HashMap<String, i128>,
    selection: Option<&crate::preparation::declaration_selection::ConstantSelection>,
    concrete_data_positions: &[TypeReferenceHandle],
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
                match ConstScalarSpelling::from_bare_name(name.as_str(syntax)) {
                    Some(ConstScalarSpelling::Canonical(value)) => {
                        let required = syntax_type_identity(syntax, *parameter_type)
                            .map_err(Diagnostic::error)?;
                        if value.type_name != required {
                            return Err(Diagnostic::error(format!(
                                "index argument for `{}::{parameter_name}` has canonical type `{}`, expected `{required}`",
                                family_name, value.type_name
                            )));
                        }
                        continue;
                    }
                    Some(ConstScalarSpelling::Integer(_)) => continue,
                    Some(ConstScalarSpelling::Boolean(value)) => {
                        let required = syntax_type_identity(syntax, *parameter_type)
                            .map_err(Diagnostic::error)?;
                        if required != "bool" {
                            return Err(Diagnostic::error(format!(
                                "index argument for `{family_name}::{parameter_name}` has canonical type `bool`, expected `{required}`"
                            )).with_source_span(name.source_span()));
                        }
                        let value = CanonicalConstValue::boolean(value);
                        syntax.type_references.replace_type_reference(
                            argument,
                            TypeReferenceNode::Named(Identifier::new(
                                value.atom(),
                                name.source_span(),
                            )),
                        );
                        continue;
                    }
                    None => {}
                }
                // Exact header selection is limited to concrete data owners.
                // Other owners retain their existing normalization path until
                // their complete lexical selection context is available.
                if let Some(selection) = selection
                    && concrete_data_positions.contains(&argument)
                {
                    let Some(definition) = selection.select(syntax, &name)? else {
                        // Ambiguous or unresolved names belong to ordinary
                        // resolution; never select from the legacy string map.
                        continue;
                    };
                    let value = canonicalize_selected_const_definition(syntax, &definition, *parameter_type, Some(selection))
                        .map_err(|reason| Diagnostic::error(format!(
                            "index argument for `{family_name}::{parameter_name}` is invalid: {reason}"
                        )).with_source_span(name.source_span()))?;
                    let replacement = match value.decode_encoding() {
                        Some(
                            language_semantics::const_value::DecodedCanonicalConstValue::Integer {
                                value,
                                ..
                            },
                        ) => value.to_string(),
                        _ => value.atom(),
                    };
                    let initializer = syntax.expressions.source_span(definition.value);
                    syntax.type_references.retain_const_argument_normalization(
                        argument,
                        name.source_span(),
                        value.encoding.clone(),
                        [syntax_trees::types::ConstArgumentOrigin {
                            reference: name.source_span(),
                            declaration: definition.name.source_span(),
                            initializer,
                            evaluated_value: value.encoding.into(),
                        }],
                        [],
                    );
                    syntax.type_references.replace_type_reference(
                        argument,
                        TypeReferenceNode::Named(Identifier::generated(replacement)),
                    );
                    continue;
                }
                super::module_constant_selection_gate(
                    syntax,
                    &name,
                    const_values,
                    const_definitions,
                )?;
                if let Some(value) = const_values.get(name.as_str(syntax)) {
                    syntax.tables.type_references.replace_type_reference(
                        argument,
                        TypeReferenceNode::Named(Identifier::generated(value.to_string())),
                    );
                    continue;
                }
                let Some(definition) = const_definitions.get(name.as_str(syntax)) else {
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
                // Structured values need the selected declaration's resolved
                // carrier, which this lexical preparation does not own. Keep
                // the authored name until ordinary resolution can retain it;
                // typing publishes the canonical atom from that exact owner.
                if matches!(
                    value.decode_encoding(),
                    Some(
                        language_semantics::const_value::DecodedCanonicalConstValue::Array { .. }
                            | language_semantics::const_value::DecodedCanonicalConstValue::Record { .. }
                            | language_semantics::const_value::DecodedCanonicalConstValue::Variant { .. }
                    )
                ) {
                    continue;
                }
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
                if syntax.const_expression_contains_name(expression)
                    || super::anonymous::requires_const_operator_selection(syntax, expression)
                {
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
