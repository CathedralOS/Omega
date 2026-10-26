//! Lexical ownership of machine type and expression occurrences before index folding.

use super::*;

/// Runtime bindings keep their authored selection. A local enters the frontier
/// only after its initializer; machine-header expressions use entry parameters.
#[derive(Default)]
pub(in crate::preparation::generic_data) struct RuntimeTemplateCaptures {
    pub(super) expressions: HashSet<ExpressionHandle>,
    pub(super) type_references: HashSet<TypeReferenceHandle>,
}

impl RuntimeTemplateCaptures {
    pub fn captures_type_reference(&self, reference: TypeReferenceHandle) -> bool {
        self.type_references.contains(&reference)
    }

    pub fn captures_type_input(
        &self,
        syntax: &SyntaxTrees,
        reference: TypeReferenceHandle,
    ) -> bool {
        let mut expressions = HashSet::new();
        let mut references = HashSet::new();
        collect_type_expression_handles(syntax, reference, &mut expressions, &mut references);
        references
            .iter()
            .any(|reference| self.type_references.contains(reference))
            || expressions
                .iter()
                .any(|expression| self.expressions.contains(expression))
    }
}

pub(in crate::preparation::generic_data) fn capture_machine_runtime_template_names(
    syntax: &SyntaxTrees,
    machine: &Machine,
    type_watermark: syntax_trees::types::TypeReferenceFrontier,
    expression_watermark: syntax_trees::expression::ExpressionFrontier,
) -> RuntimeTemplateCaptures {
    let states = syntax.items.state_handles(machine.states);
    let parameters = states
        .first()
        .map(|state| syntax.items.state(*state).parameters)
        .unwrap_or_default();
    let mut captured = RuntimeTemplateCaptures::default();
    let header_frontier = ConstructorFrontier {
        parameters,
        prior_statements: &[],
    };
    for (handle, expression) in syntax.expressions.iter_expressions() {
        if expression_watermark.contains(handle)
            && let ExpressionNode::Name(path) = expression
            && header_frontier.captures(syntax, *path)
        {
            captured.expressions.insert(handle);
        }
    }
    for (handle, _) in syntax
        .type_references
        .named_nodes_from(type_watermark, syntax)
    {
        if let Some(name) = syntax.type_references.type_reference(handle).as_named()
            && header_frontier.captures_name(syntax, name)
        {
            captured.type_references.insert(handle);
        }
    }
    capture_machine_scoped_names(syntax, machine, &mut captured);
    captured
}

/// Per-occurrence lexical custody for the original machine forest. Header-only
/// global selection cannot stand in for a machine binder, state input or local.
pub(in crate::preparation::generic_data) fn capture_domain_machine_names(
    syntax: &SyntaxTrees,
) -> RuntimeTemplateCaptures {
    let mut captures = RuntimeTemplateCaptures::default();
    for item in syntax.root_items() {
        let Some(machine) = item.as_machine() else {
            continue;
        };
        let mut owned = RuntimeTemplateCaptures::default();
        capture_machine_scoped_names(syntax, machine, &mut owned);
        captures.expressions.extend(owned.expressions);
        captures.type_references.extend(owned.type_references);
    }
    captures
}

fn capture_machine_scoped_names(
    syntax: &SyntaxTrees,
    machine: &Machine,
    captured: &mut RuntimeTemplateCaptures,
) {
    let mut bound_names = syntax
        .items
        .type_parameters(machine.type_parameters)
        .iter()
        .map(|parameter| parameter.name.as_str(syntax).to_owned())
        .collect::<Vec<_>>();
    bound_names.extend(
        machine
            .conformance_bounds
            .iter()
            .map(|bound| bound.binder.as_str(syntax).to_owned()),
    );
    let mut expressions = HashSet::new();
    let mut references = HashSet::new();
    collect_contract_expression_handles(
        syntax,
        machine.contracts,
        &mut expressions,
        &mut references,
    );
    capture_parameter_types(syntax, machine.type_parameters, &bound_names, captured);
    for fact in syntax.items.proof_facts(machine.where_facts) {
        if let ProofFact::Expression(expression) = fact {
            collect_expression_handles(syntax, *expression, &mut expressions);
        }
    }
    for bound in &machine.conformance_bounds {
        for reference in syntax
            .type_references
            .type_reference_handles(bound.arguments)
        {
            collect_type_expression_handles(syntax, *reference, &mut expressions, &mut references);
        }
    }
    let parameters = syntax
        .items
        .state_handles(machine.states)
        .first()
        .map(|state| syntax.items.state(*state).parameters)
        .unwrap_or_default();
    update_captured_names(
        syntax,
        &mut expressions,
        &mut references,
        &ConstructorFrontier {
            parameters,
            prior_statements: &[],
        },
        &bound_names,
        captured,
    );
    for state in syntax.items.state_handles(machine.states) {
        let state = syntax.items.state(*state);
        let mut signature = HashSet::new();
        let mut signature_types = HashSet::new();
        collect_type_expression_handles(
            syntax,
            state.return_type,
            &mut signature,
            &mut signature_types,
        );
        for parameter in syntax.items.state_parameters(state.parameters) {
            collect_type_expression_handles(
                syntax,
                syntax.items.state_parameter(*parameter).type_reference,
                &mut signature,
                &mut signature_types,
            );
        }
        collect_contract_expression_handles(
            syntax,
            state.contracts,
            &mut signature,
            &mut signature_types,
        );
        let statements = syntax.items.statements(state.statements);
        update_captured_names(
            syntax,
            &mut signature,
            &mut signature_types,
            &ConstructorFrontier {
                parameters: state.parameters,
                prior_statements: &[],
            },
            &bound_names,
            captured,
        );
        for (ordinal, statement) in statements.iter().enumerate() {
            let mut expressions = HashSet::new();
            let mut type_references = HashSet::new();
            collect_statement_expression_handles(syntax, *statement, &mut expressions);
            match syntax.statements.statement(*statement) {
                StatementNode::LocalData(local) => {
                    collect_type_expression_handles(
                        syntax,
                        local.type_reference,
                        &mut expressions,
                        &mut type_references,
                    );
                }
                StatementNode::Call(call) => {
                    collect_static_argument_expression_handles(
                        syntax,
                        &call.machine_arguments,
                        &mut expressions,
                        &mut type_references,
                    );
                }
                _ => {}
            }
            update_captured_names(
                syntax,
                &mut expressions,
                &mut type_references,
                &ConstructorFrontier {
                    parameters: state.parameters,
                    prior_statements: &statements[..ordinal],
                },
                &bound_names,
                captured,
            );
        }
    }
}

fn capture_parameter_types(
    syntax: &SyntaxTrees,
    parameters: HandleSpan<syntax_trees::item::TypeParameter>,
    enclosing_names: &[String],
    captured: &mut RuntimeTemplateCaptures,
) {
    use syntax_trees::item::MachineParameterContract;
    let mut names = enclosing_names.to_vec();
    names.extend(
        syntax
            .items
            .type_parameters(parameters)
            .iter()
            .map(|parameter| parameter.name.as_str(syntax).to_owned()),
    );
    let mut expressions = HashSet::new();
    let mut references = HashSet::new();
    for parameter in syntax.items.type_parameters(parameters) {
        match &parameter.kind {
            TypeParameterKind::Const { type_reference }
            | TypeParameterKind::Value { type_reference } => {
                collect_type_expression_handles(
                    syntax,
                    *type_reference,
                    &mut expressions,
                    &mut references,
                );
            }
            TypeParameterKind::Machine {
                contract:
                    Some(MachineParameterContract::Nominal {
                        trait_arguments, ..
                    }),
            } => {
                for argument in syntax
                    .type_references
                    .type_reference_handles(*trait_arguments)
                {
                    collect_type_expression_handles(
                        syntax,
                        *argument,
                        &mut expressions,
                        &mut references,
                    );
                }
            }
            TypeParameterKind::Machine {
                contract: Some(MachineParameterContract::Structural(signature)),
            } => {
                capture_parameter_types(syntax, signature.type_parameters, &names, captured);
                let mut signature_expressions = HashSet::new();
                let mut signature_types = HashSet::new();
                for fact in syntax.items.proof_facts(signature.where_facts) {
                    if let ProofFact::Expression(expression) = fact {
                        collect_expression_handles(syntax, *expression, &mut signature_expressions);
                    }
                }
                collect_type_expression_handles(
                    syntax,
                    signature.return_type,
                    &mut signature_expressions,
                    &mut signature_types,
                );
                for parameter in syntax.items.state_parameters(signature.parameters) {
                    collect_type_expression_handles(
                        syntax,
                        syntax.items.state_parameter(*parameter).type_reference,
                        &mut signature_expressions,
                        &mut signature_types,
                    );
                }
                collect_contract_expression_handles(
                    syntax,
                    signature.contracts,
                    &mut signature_expressions,
                    &mut signature_types,
                );
                let mut signature_names = names.clone();
                signature_names.extend(
                    syntax
                        .items
                        .type_parameters(signature.type_parameters)
                        .iter()
                        .map(|parameter| parameter.name.as_str(syntax).to_owned()),
                );
                update_captured_names(
                    syntax,
                    &mut signature_expressions,
                    &mut signature_types,
                    &ConstructorFrontier {
                        parameters: signature.parameters,
                        prior_statements: &[],
                    },
                    &signature_names,
                    captured,
                );
            }
            _ => {}
        }
    }
    update_captured_names(
        syntax,
        &mut expressions,
        &mut references,
        &ConstructorFrontier {
            parameters: Default::default(),
            prior_statements: &[],
        },
        &names,
        captured,
    );
}

fn update_captured_names(
    syntax: &SyntaxTrees,
    expressions: &mut HashSet<ExpressionHandle>,
    type_references: &mut HashSet<TypeReferenceHandle>,
    frontier: &ConstructorFrontier<'_>,
    bound_names: &[String],
    captured: &mut RuntimeTemplateCaptures,
) {
    // The executable child walker deliberately excludes type positions. Those
    // positions still belong to the same lexical frontier during substitution.
    let mut visited = HashSet::new();
    loop {
        let pending = expressions
            .difference(&visited)
            .copied()
            .collect::<Vec<_>>();
        if pending.is_empty() {
            break;
        }
        for handle in pending {
            visited.insert(handle);
            visit_expression_type_inputs(syntax, handle, &mut |reference| {
                collect_type_expression_handles(syntax, reference, expressions, type_references);
            });
        }
    }
    for handle in expressions.iter() {
        if let ExpressionNode::Name(path) = syntax.expressions.expression(*handle) {
            if frontier.captures(syntax, *path)
                || matches!(syntax.expressions.identifier_path_members(*path), [name]
                    if bound_names.iter().any(|bound| bound == name.as_str(syntax)))
            {
                captured.expressions.insert(*handle);
            } else {
                captured.expressions.remove(handle);
            }
        }
    }
    for reference in type_references.iter() {
        if let Some(name) = syntax.type_references.type_reference(*reference).as_named() {
            if frontier.captures_name(syntax, name)
                || bound_names.iter().any(|bound| bound == name.as_str(syntax))
            {
                captured.type_references.insert(*reference);
            } else {
                captured.type_references.remove(reference);
            }
        }
    }
}
