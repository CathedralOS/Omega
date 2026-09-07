//! Constant evaluation: templates.

use super::*;

pub(in crate::generic_data) fn replace_const_expression_names_from(
    syntax: &mut SyntaxTrees,
    expression_watermark: u32,
    const_literals: &HashMap<String, IntegerLiteral>,
) {
    let replacements = syntax
        .expressions
        .iter_expressions()
        .filter(|(handle, _)| handle.arena_index() >= expression_watermark)
        .filter_map(|(handle, expression)| {
            let ExpressionNode::Name(path) = expression else {
                return None;
            };
            let [name] = syntax.expressions.identifier_path_members(*path) else {
                return None;
            };
            const_literals
                .get(name.as_str())
                .cloned()
                .map(|literal| (handle, literal))
        })
        .collect::<Vec<_>>();
    for (handle, literal) in replacements {
        syntax
            .expressions
            .replace_expression(handle, ExpressionNode::Integer(literal));
    }
}

/// Generic definitions remain in the tree after their concrete records are
/// synthesized so the normal frontend can validate the template. A symbolic
/// const expression cannot cross that boundary yet, so reduce each template
/// expression to either its concrete value or one declared const-parameter
/// dependency. The concrete clones already carry the fully evaluated value;
/// this placeholder exists only to preserve the established generic type/kind
/// checks on the source template.
pub(in crate::generic_data) fn normalize_generic_template_const_expressions(
    syntax: &mut SyntaxTrees,
    const_values: &HashMap<String, i128>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(), Diagnostic> {
    let templates: Vec<(String, HashSet<String>, Vec<TypeReferenceHandle>)> = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Data(definition) = item else {
                return None;
            };
            if definition.type_parameters.is_empty() {
                return None;
            }
            let symbolic_parameters = syntax
                .tables
                .items
                .type_parameters(definition.type_parameters)
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Const { .. }))
                .map(|parameter| parameter.name.as_str().to_string())
                .collect();
            let fields = syntax
                .tables
                .items
                .data_members(definition.members)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => Some(field.type_reference),
                    DataMember::Variant(_) => None,
                    DataMember::Retired(_) => None,
                })
                .collect();
            Some((
                definition.name.as_str().to_string(),
                symbolic_parameters,
                fields,
            ))
        })
        .collect();

    for (template, symbolic_parameters, fields) in templates {
        for field in fields {
            normalize_template_type_reference(
                syntax,
                field,
                const_values,
                &symbolic_parameters,
                warnings,
            )
            .map_err(|reason| {
                Diagnostic::error(format!(
                    "const argument expression in generic data `{template}` is invalid: {reason}"
                ))
            })?;
        }
    }
    Ok(())
}

pub(in crate::generic_data) fn normalize_template_type_reference(
    syntax: &mut SyntaxTrees,
    type_reference: TypeReferenceHandle,
    const_values: &HashMap<String, i128>,
    symbolic_parameters: &HashSet<String>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let node = syntax
        .tables
        .type_references
        .type_reference(type_reference)
        .clone();
    match node {
        TypeReferenceNode::Reference { referee, .. } => normalize_template_type_reference(
            syntax,
            referee,
            const_values,
            symbolic_parameters,
            warnings,
        ),
        TypeReferenceNode::Constrained { base_type, .. } => normalize_template_type_reference(
            syntax,
            base_type,
            const_values,
            symbolic_parameters,
            warnings,
        ),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => normalize_template_type_reference(
            syntax,
            element_type,
            const_values,
            symbolic_parameters,
            warnings,
        ),
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            let arguments = syntax
                .tables
                .type_references
                .type_reference_handles(arguments)
                .to_vec();
            let integer_types = generic_const_integer_types(syntax, base_name.as_str());
            for (index, argument) in arguments.into_iter().enumerate() {
                let node = syntax
                    .tables
                    .type_references
                    .type_reference(argument)
                    .clone();
                if let TypeReferenceNode::ConstExpression(expression) = node {
                    let placeholder = evaluate_const_argument_expression(
                        syntax,
                        expression,
                        const_values,
                        &HashMap::new(),
                        symbolic_parameters,
                        integer_types.get(index).copied().flatten(),
                        warnings,
                    )?;
                    let name = match placeholder {
                        EvaluatedConst::Concrete(value) => value.to_string(),
                        EvaluatedConst::Symbolic(name) => name,
                    };
                    syntax.tables.type_references.replace_type_reference(
                        argument,
                        TypeReferenceNode::Named(Identifier::generated(name)),
                    );
                } else {
                    normalize_template_type_reference(
                        syntax,
                        argument,
                        const_values,
                        symbolic_parameters,
                        warnings,
                    )?;
                }
            }
            Ok(())
        }
        TypeReferenceNode::ConstExpression(expression) => {
            let placeholder = evaluate_const_argument_expression(
                syntax,
                expression,
                const_values,
                &HashMap::new(),
                symbolic_parameters,
                None,
                warnings,
            )?;
            let name = match placeholder {
                EvaluatedConst::Concrete(value) => value.to_string(),
                EvaluatedConst::Symbolic(name) => name,
            };
            syntax.tables.type_references.replace_type_reference(
                type_reference,
                TypeReferenceNode::Named(Identifier::generated(name)),
            );
            Ok(())
        }
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named(_)
        | TypeReferenceNode::SelfType
        | TypeReferenceNode::Unit => Ok(()),
    }
}

/// Every TYPE-REFERENCE position a generic-data spelling can appear in: data
/// FIELDS plus machine-body `let`-local, state PARAMETER, and RETURN types. Run
/// afresh each fixpoint round so newly-synthesized records' fields are seen.
pub(in crate::generic_data) fn collect_type_reference_positions(
    syntax: &SyntaxTrees,
) -> Vec<TypeReferenceHandle> {
    fn collect(
        syntax: &SyntaxTrees,
        type_reference: TypeReferenceHandle,
        positions: &mut Vec<TypeReferenceHandle>,
    ) {
        positions.push(type_reference);
        match syntax.tables.type_references.type_reference(type_reference) {
            TypeReferenceNode::Reference { referee, .. } => collect(syntax, *referee, positions),
            TypeReferenceNode::Constrained { base_type, .. } => {
                collect(syntax, *base_type, positions)
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                collect(syntax, *element_type, positions)
            }
            TypeReferenceNode::Generic { arguments, .. } => {
                for argument in syntax
                    .tables
                    .type_references
                    .type_reference_handles(*arguments)
                {
                    collect(syntax, *argument, positions);
                }
            }
            TypeReferenceNode::ConstExpression(_)
            | TypeReferenceNode::DynamicTrait { .. }
            | TypeReferenceNode::Named(_)
            | TypeReferenceNode::SelfType
            | TypeReferenceNode::Unit => {}
        }
    }

    let mut positions: Vec<TypeReferenceHandle> = Vec::new();
    for item in syntax.root_items() {
        match item {
            // SKIP the bodies of GENERIC TEMPLATES (defs/machines with type
            // parameters): their `Box<T>` fields carry the type PARAMETER as an
            // argument, not a concrete instantiation -- monomorphizing them would
            // synthesize a bogus `Box<T>` record and corrupt the template. Only
            // concrete records (incl. synthesized instances) and non-generic
            // machine bodies hold real `Box<i32>` spellings.
            Item::Data(definition) if definition.type_parameters.is_empty() => {
                for member in syntax.tables.items.data_members(definition.members) {
                    match member {
                        DataMember::Field(field) => {
                            collect(syntax, field.type_reference, &mut positions)
                        }
                        DataMember::Variant(variant) => {
                            for field in syntax.tables.items.data_payload_fields(variant.payload) {
                                collect(syntax, field.type_reference, &mut positions);
                            }
                        }
                        DataMember::Retired(_) => {}
                    }
                }
            }
            Item::Machine(machine) if machine.type_parameters.is_empty() => {
                // Conformance arguments participate in the same concrete
                // generic-data identity as the machine signature. Rewriting
                // `-> Algebra<Unit>` while leaving
                // `satisfies Trait<Algebra<Unit>>` generic makes an otherwise
                // exact requirement mismatch after instance synthesis.
                for conformance in syntax.tables.items.satisfies_clauses(machine.satisfies) {
                    for argument in syntax
                        .tables
                        .type_references
                        .type_reference_handles(conformance.arguments)
                    {
                        collect(syntax, *argument, &mut positions);
                    }
                }
                for state_handle in syntax.tables.items.state_handles(machine.states) {
                    let state = syntax.tables.items.state(*state_handle);
                    collect(syntax, state.return_type, &mut positions);
                    for parameter_handle in syntax.tables.items.state_parameters(state.parameters) {
                        collect(
                            syntax,
                            syntax
                                .tables
                                .items
                                .state_parameter(*parameter_handle)
                                .type_reference,
                            &mut positions,
                        );
                    }
                    for statement_handle in syntax.tables.items.statements(state.statements) {
                        if let StatementNode::LocalData(local) =
                            syntax.tables.statements.statement(*statement_handle)
                        {
                            collect(syntax, local.type_reference, &mut positions);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Cast targets are type-reference owners in concrete machine bodies too.
    // Rewriting the stated local while leaving `as &Pair<u32>` as a raw
    // Generic node gives downstream representation validation two identities
    // for the same synthesized instance. Walk only expressions reachable from
    // non-generic machines; generic template bodies remain deliberately open.
    let concrete_expressions = super::concrete_machine_expression_handles(syntax);
    for (handle, expression) in syntax.expressions.iter_expressions() {
        if concrete_expressions.contains(&handle.arena_index())
            && let ExpressionNode::Cast(cast) = expression
        {
            collect(syntax, cast.target_type, &mut positions);
        }
    }
    positions
}
