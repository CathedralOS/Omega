//! Complete the selected machine application without changing static binder kinds.

use super::*;

pub(super) fn complete_arguments(
    syntax: &mut SyntaxTrees,
    selected: usize,
    target: &syntax_trees::identifier::Identifier,
    supplied: &[StaticMachineArgument],
    result_type: Option<TypeReferenceHandle>,
    ordinary_types: &[(TypeReferenceHandle, TypeReferenceHandle)],
    sources: &SourceLinks,
    selection: &ConstantSelection<'_>,
) -> Result<(Box<[StaticMachineArgument]>, Option<TypeReferenceHandle>), Diagnostic> {
    let Some((_, declaration)) = sources
        .declarations
        .iter()
        .find(|(index, _)| *index == selected)
    else {
        return Err(Diagnostic::error(
            "retained machine structural equation requires its original syntax for discharge",
        )
        .with_source_span(target.source_span()));
    };
    let all_parameters = syntax
        .items
        .type_parameters(declaration.type_parameters)
        .to_vec();
    // Machines retain their explicit conformance suffix separately from the
    // type/value/machine parameter span. Preserve it verbatim for the normal
    // conformance checker; it never supplies a type or an integer binding.
    let evidence_count = declaration.conformance_bounds.len();
    let Some(prefix_count) = supplied.len().checked_sub(evidence_count) else {
        return Err(Diagnostic::error(
            "machine equation application requires its explicit conformance arguments",
        )
        .with_source_span(target.source_span()));
    };
    let (supplied, evidence) = supplied.split_at(prefix_count);
    let original_equations = type_equations::classify_type_equations(
        syntax,
        &all_parameters,
        syntax.items.proof_facts(declaration.where_facts),
    );
    if original_equations.len() != declaration.where_facts.count() as usize {
        return Err(Diagnostic::error(
            "machine where clause requires a structural type equation or a named conformance bound",
        )
        .with_source_span(target.source_span()));
    }
    if supplied.len() > all_parameters.len() {
        return Err(Diagnostic::error(
            "machine equation application supplies too many static arguments",
        )
        .with_source_span(target.source_span()));
    }
    let mut all_names = all_parameters
        .iter()
        .map(|parameter| parameter.name.as_str(syntax).to_owned())
        .collect::<Vec<_>>();
    all_names.extend(
        declaration
            .conformance_bounds
            .iter()
            .map(|bound| bound.binder.as_str(syntax).to_owned()),
    );
    let signature_mentions = syntax
        .items
        .state_handles(declaration.states)
        .first()
        .map(|entry| syntax.items.state(*entry))
        .into_iter()
        .flat_map(|entry| {
            syntax
                .items
                .state_parameters(entry.parameters)
                .iter()
                .map(|parameter| syntax.items.state_parameter(*parameter).type_reference)
                .chain(std::iter::once(entry.return_type))
        })
        .chain(result_type)
        .flat_map(|reference| {
            type_equations::type_reference_parameter_mentions(syntax, reference, &all_names)
        })
        .collect::<Vec<_>>();
    // Removed telescope slots remain lexical binders, never same-named globals.
    for (index, name) in all_names.iter().enumerate() {
        if all_parameters.get(index).is_some_and(|parameter| {
            matches!(
                parameter.kind,
                TypeParameterKind::Type | TypeParameterKind::Const { .. }
            )
        }) {
            continue;
        }
        if signature_mentions.contains(&index)
            || original_equations
                .iter()
                .any(|equation| equation.mentions_parameter(syntax, &all_names, index))
        {
            return Err(Diagnostic::error(format!(
                "machine structural equation requires a type or const binder; `{name}` retains a non-type selection in its signature"
            ))
            .with_source_span(target.source_span()));
        }
    }
    let mut equation_positions = Vec::new();
    for (index, parameter) in all_parameters.iter().enumerate() {
        if matches!(
            parameter.kind,
            TypeParameterKind::Type | TypeParameterKind::Const { .. }
        ) {
            equation_positions.push(index);
        } else {
            if index >= supplied.len() {
                return Err(Diagnostic::error(format!(
                    "machine equation application requires an explicit argument for `{}`",
                    all_names[index]
                ))
                .with_source_span(target.source_span()));
            }
        }
    }
    let parameters = equation_positions
        .iter()
        .map(|index| all_parameters[*index].clone())
        .collect::<Vec<_>>();
    let mut first = Handle::invalid();
    for parameter in &parameters {
        let handle = syntax.items.append_type_parameter(parameter.clone());
        if !first.is_valid() {
            first = handle;
        }
    }
    let equation_span = arena::HandleSpan::from_parts_or_empty(first, parameters.len() as u32);
    let equations = type_equations::classify_type_equations(
        syntax,
        &parameters,
        syntax.items.proof_facts(declaration.where_facts),
    );
    let parameter_names = parameters
        .iter()
        .map(|parameter| parameter.name.as_str(syntax).to_owned())
        .collect::<Vec<_>>();
    let mut const_parameter_types = Vec::new();
    for parameter in &parameters {
        const_parameter_types.push(match parameter.kind {
            TypeParameterKind::Type => None,
            TypeParameterKind::Const { type_reference } => Some(type_reference),
            _ => {
                return Err(Diagnostic::error(
                    "machine equation completion requires type and const binders",
                )
                .with_source_span(target.source_span()));
            }
        });
    }
    let mut arguments = Vec::new();
    for (index, parameter) in equation_positions.iter().zip(&parameters) {
        let Some(argument) = supplied.get(*index) else {
            break;
        };
        let reference = match &parameter.kind {
            TypeParameterKind::Type if argument.type_reference.is_valid() => argument.type_reference,
            TypeParameterKind::Type if argument.application.is_some() => {
                application_argument_reference(syntax, argument, &parameter.kind, selection)?
            }
            TypeParameterKind::Type if argument.const_literal.is_none() && argument.application.is_none() && argument.evidence_projection.is_none() && !argument.path.is_empty() => {
                let name = argument.path.joined_path_in(syntax);
                syntax.type_references.insert(TypeReferenceNode::Named(syntax_trees::identifier::Identifier::new(name, argument.path[0].source_span())))
            }
            // Supplied literals retain ordinary static-argument carrier checking;
            // unrelated equations must not impose constructor-slot restrictions.
            TypeParameterKind::Const { .. } if argument.const_literal.is_some() => {
                let value = argument.const_literal.as_ref().and_then(IntegerLiteral::value_bignum).ok_or_else(|| Diagnostic::error("machine equation const argument is not an exact integer"))?;
                syntax.type_references.insert(TypeReferenceNode::Named(syntax_trees::identifier::Identifier::generated(value.to_string())))
            }
            TypeParameterKind::Const { .. } => {
                supplied_const_argument_reference(syntax, argument, selection)?
            }
            _ => return Err(Diagnostic::error("machine equation argument mixes type and value kinds or retains an open static argument").with_source_span(target.source_span())),
        };
        if matches!(parameter.kind, TypeParameterKind::Type)
            && super::super::generic_data::closed_argument_identity(
                syntax,
                Some(selection),
                reference,
                false,
            )
            .is_none()
        {
            return Err(Diagnostic::error(
                "machine equation requires a closed explicit type argument",
            )
            .with_source_span(target.source_span()));
        }
        arguments.push(reference);
    }
    let const_values = module_constants::lexical_integer_const_values(syntax);
    let mut warnings = Vec::new();
    let declaration_name = declaration.name.as_str(syntax).to_owned();
    let tuple = type_equations::complete_equation_arguments_with_types(
        syntax,
        EquationTemplate {
            kind: "machine",
            name: &declaration_name,
            parameters: equation_span,
            parameter_names: &parameter_names,
            const_parameter_types: &const_parameter_types,
            type_equations: &equations,
        },
        target,
        &arguments,
        ordinary_types,
        &const_values,
        Some(selection),
        &mut warnings,
    )?;
    // A selected generic callee's result uses this same completed tuple.
    // Reconstruct through the structural owner, retaining the authored type
    // nodes and exact constructor identities rather than guessing from its body.
    let result_type = result_type
        .map(|result| {
            type_equations::materialize_completed_type(
                syntax,
                EquationTemplate {
                    kind: "machine",
                    name: &declaration_name,
                    parameters: equation_span,
                    parameter_names: &parameter_names,
                    const_parameter_types: &const_parameter_types,
                    type_equations: &equations,
                },
                target,
                &tuple,
                result,
                &const_values,
                Some(selection),
            )
        })
        .transpose()?;
    for warning in warnings {
        eprintln!("{warning}");
    }
    let mut completed = supplied.to_vec();
    for ((index, reference), parameter) in equation_positions.iter().zip(&tuple).zip(&parameters) {
        let Some(argument) = completed.get_mut(*index) else {
            continue;
        };
        if matches!(parameter.kind, TypeParameterKind::Type) && argument.application.is_some() {
            argument.type_reference = *reference;
            argument.path = Box::default();
            argument.application = None;
        }
    }
    for ((index, reference), parameter) in equation_positions.into_iter().zip(tuple).zip(parameters)
    {
        if index < supplied.len() {
            continue;
        }
        let mut argument = StaticMachineArgument {
            type_reference: TypeReferenceHandle::invalid(),
            path: Box::default(),
            application: None,
            const_literal: None,
            evidence_projection: None,
        };
        match parameter.kind {
            TypeParameterKind::Type => argument.type_reference = reference,
            TypeParameterKind::Const { .. } => {
                let Some(value) = syntax.type_references.type_reference(reference).as_named()
                else {
                    return Err(Diagnostic::error(
                        "machine equation did not produce a closed const argument",
                    ));
                };
                let text = value.as_str(syntax);
                if let Some(value) = type_equations::normalized_boolean_argument(text) {
                    argument.path = vec![syntax_trees::identifier::Identifier::generated(
                        CanonicalConstValue::boolean(value).atom(),
                    )]
                    .into_boxed_slice();
                    completed.push(argument);
                    continue;
                }
                if CanonicalConstValue::from_atom(text).is_some() {
                    argument = inferred_const_argument(syntax, reference, selection)?;
                    completed.push(argument);
                    continue;
                }
                if text.parse::<i128>().is_err() {
                    argument.path = vec![value.clone()].into_boxed_slice();
                    completed.push(argument);
                    continue;
                }
                argument.const_literal = Some(
                    IntegerLiteral::from_parts(
                        text.starts_with('-'),
                        IntegerRadix::Decimal,
                        text.trim_start_matches('-'),
                    )
                    .map_err(Diagnostic::error)?,
                );
            }
            _ => unreachable!("parameter kinds were admitted above"),
        }
        completed.push(argument);
    }
    completed.extend_from_slice(evidence);
    Ok((completed.into_boxed_slice(), result_type))
}
