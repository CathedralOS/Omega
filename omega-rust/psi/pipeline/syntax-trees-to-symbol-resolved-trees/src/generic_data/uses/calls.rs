//! Expected types from calls, returns and receiver paths.

use super::super::*;

/// Closed callable signatures provide exact contextual types without inferring
/// from literal fields. Free-machine overloads and concrete attached-machine
/// overloads each contribute a context only when every same-name candidate on
/// the exact owner agrees on one parameter signature. A direct `self.method`
/// statement call has that exact owner from the enclosing attached machine. An
/// explicitly typed local receiver or direct `self.field` receiver also names
/// one exact nominal owner. Computed, chained, and dynamic receiver selection
/// remains resolver-owned and fail closed here.
pub(in crate::generic_data) fn relabel_closed_data_uses_in_exact_calls_and_returns(
    syntax: &mut SyntaxTrees,
    synthesized_origins: &HashMap<String, String>,
) {
    let mut signatures = HashMap::<String, Option<Vec<TypeReferenceHandle>>>::new();
    let mut attached_signatures =
        HashMap::<(String, String), Option<Vec<TypeReferenceHandle>>>::new();
    for item in syntax.root_items() {
        let Item::Machine(machine) = item else {
            continue;
        };
        if !machine.type_parameters.is_empty() {
            continue;
        }
        let Some(entry) = syntax.tables.items.state_handles(machine.states).first() else {
            continue;
        };
        let parameters = syntax
            .tables
            .items
            .state_parameters(syntax.tables.items.state(*entry).parameters)
            .iter()
            .filter_map(|parameter| {
                let parameter = syntax.tables.items.state_parameter(*parameter);
                (!parameter.is_self).then_some(parameter.type_reference)
            })
            .collect::<Vec<_>>();
        let (signature_map, name) = if let Some(attached) = machine.attached_data.as_ref() {
            let method = machine
                .name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or(machine.name.as_str())
                .to_owned();
            (
                &mut attached_signatures,
                (attached.as_str().to_owned(), method),
            )
        } else {
            let name = machine.name.as_str().to_owned();
            if let Some(signature) = signatures.get_mut(&name) {
                let agrees = signature.as_ref().is_some_and(|prior| {
                    call_context_parameter_types_agree(syntax, prior, &parameters)
                });
                if !agrees {
                    *signature = None;
                }
            } else {
                signatures.insert(name, Some(parameters));
            }
            continue;
        };
        if let Some(signature) = signature_map.get_mut(&name) {
            let agrees = signature.as_ref().is_some_and(|prior| {
                call_context_parameter_types_agree(syntax, prior, &parameters)
            });
            if !agrees {
                *signature = None;
            }
        } else {
            signature_map.insert(name, Some(parameters));
        }
    }

    let concrete_states = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Machine(machine) if machine.type_parameters.is_empty() => Some(machine),
            _ => None,
        })
        .flat_map(|machine| {
            syntax
                .tables
                .items
                .state_handles(machine.states)
                .iter()
                .map(|state| (*state, machine.attached_data.clone()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for (state_handle, attached_data) in concrete_states {
        let state = syntax.tables.items.state(state_handle).clone();
        let statements = syntax.tables.items.statements(state.statements).to_vec();
        let final_statement = statements.last().copied();
        let mut local_owner_types = HashMap::<String, String>::new();
        for parameter in syntax.tables.items.state_parameters(state.parameters) {
            let parameter = syntax.tables.items.state_parameter(*parameter);
            if !parameter.is_self
                && let Some(type_name) = named_type_name(syntax, parameter.type_reference)
            {
                local_owner_types.insert(parameter.name.as_str().to_owned(), type_name);
            }
        }
        for statement in &statements {
            if let StatementNode::LocalData(local) = syntax.tables.statements.statement(*statement)
                && let Some(type_name) = named_type_name(syntax, local.type_reference)
            {
                local_owner_types.insert(local.name.as_str().to_owned(), type_name);
            }
        }
        let self_field_owner_types = attached_data
            .as_ref()
            .and_then(|attached| {
                syntax.root_items().find_map(|item| match item {
                    Item::Data(definition) if definition.name == *attached => Some(
                        syntax
                            .tables
                            .items
                            .data_members(definition.members)
                            .iter()
                            .filter_map(|member| match member {
                                DataMember::Field(field) => {
                                    named_type_name(syntax, field.type_reference)
                                        .map(|owner| (field.name.as_str().to_owned(), owner))
                                }
                                DataMember::Variant(_) | DataMember::Retired(_) => None,
                            })
                            .collect::<HashMap<_, _>>(),
                    ),
                    _ => None,
                })
            })
            .unwrap_or_default();
        for statement in &statements {
            match syntax.tables.statements.statement(*statement) {
                StatementNode::Expression(value)
                    if state.return_type.is_valid() && Some(*statement) == final_statement =>
                {
                    relabel_data_literal_for_expected_type(
                        syntax,
                        *value,
                        state.return_type,
                        synthesized_origins,
                    );
                }
                StatementNode::Call(call) => {
                    let owner = exact_statement_receiver_owner(
                        syntax,
                        call.receiver,
                        call.receiver_starts_at_self,
                        attached_data.as_deref(),
                        &local_owner_types,
                        &self_field_owner_types,
                    );
                    let parameters = if call.receiver.is_empty() {
                        signatures.get(call.target.as_str())
                    } else {
                        let Some(owner) = owner else {
                            continue;
                        };
                        attached_signatures.get(&(owner, call.target.as_str().to_owned()))
                    };
                    let Some(Some(parameters)) = parameters else {
                        continue;
                    };
                    let arguments = syntax
                        .tables
                        .statements
                        .expression_handles(call.arguments)
                        .to_vec();
                    for (argument, expected_type) in arguments.into_iter().zip(parameters) {
                        relabel_data_literal_for_expected_type(
                            syntax,
                            argument,
                            *expected_type,
                            synthesized_origins,
                        );
                    }
                }
                StatementNode::Transition(transition) => {
                    for target in [transition.target, transition.continuation] {
                        if !target.is_valid() {
                            continue;
                        }
                        if let syntax_trees::statement::TransitionTargetNode::Value(value) =
                            syntax.tables.statements.transition_target(target)
                        {
                            relabel_data_literal_for_expected_type(
                                syntax,
                                *value,
                                state.return_type,
                                synthesized_origins,
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        let mut reachable = HashSet::new();
        for statement in statements {
            collect_statement_expression_handles(syntax, statement, &mut reachable);
        }
        let calls = syntax
            .expressions
            .iter_expressions()
            .filter(|(handle, _)| reachable.contains(&handle.arena_index()))
            .filter_map(|(_, expression)| match expression {
                ExpressionNode::Call(call) if !call.receiver.is_valid() => {
                    Some((call.target.clone(), call.arguments, None))
                }
                ExpressionNode::Call(call) => exact_expression_receiver_owner(
                    syntax,
                    call.receiver,
                    attached_data.as_deref(),
                    &local_owner_types,
                    &self_field_owner_types,
                )
                .map(|owner| (call.target.clone(), call.arguments, Some(owner))),
                _ => None,
            })
            .collect::<Vec<_>>();
        for (target, arguments, owner) in calls {
            let parameters = if let Some(owner) = owner {
                attached_signatures.get(&(owner, target.as_str().to_owned()))
            } else {
                signatures.get(target.as_str())
            };
            let Some(Some(parameters)) = parameters else {
                continue;
            };
            let arguments = syntax.expressions.expression_handles(arguments).to_vec();
            for (argument, expected_type) in arguments.into_iter().zip(parameters) {
                relabel_data_literal_for_expected_type(
                    syntax,
                    argument,
                    *expected_type,
                    synthesized_origins,
                );
            }
        }
    }
}

pub(in crate::generic_data) fn exact_statement_receiver_owner(
    syntax: &SyntaxTrees,
    receiver: HandleSpan<Identifier>,
    starts_at_self: bool,
    attached_data: Option<&str>,
    local_owner_types: &HashMap<String, String>,
    self_field_owner_types: &HashMap<String, String>,
) -> Option<String> {
    let members = syntax.tables.statements.identifier_path_members(receiver);
    match members {
        [_] if starts_at_self => attached_data.map(str::to_owned),
        [local] if !starts_at_self => local_owner_types.get(local.as_str()).cloned(),
        [_, field] if starts_at_self => self_field_owner_types.get(field.as_str()).cloned(),
        _ => None,
    }
}

pub(in crate::generic_data) fn exact_expression_receiver_owner(
    syntax: &SyntaxTrees,
    receiver: ExpressionHandle,
    attached_data: Option<&str>,
    local_owner_types: &HashMap<String, String>,
    self_field_owner_types: &HashMap<String, String>,
) -> Option<String> {
    match syntax.expressions.expression(receiver) {
        ExpressionNode::SelfValue => attached_data.map(str::to_owned),
        ExpressionNode::Name(path) => {
            let [local] = syntax.expressions.identifier_path_members(*path) else {
                return None;
            };
            local_owner_types.get(local.as_str()).cloned()
        }
        ExpressionNode::Member(member)
            if matches!(
                syntax.expressions.expression(member.receiver),
                ExpressionNode::SelfValue
            ) =>
        {
            self_field_owner_types.get(member.member.as_str()).cloned()
        }
        _ => None,
    }
}

/// Conservative pre-resolution equality for the expected parameter types that
/// drive generic-literal relabeling. Handles are arena-local occurrences, so
/// comparing the handles themselves would make two textually identical
/// overload signatures look different. Unsupported or expression-bearing
/// details fail closed rather than broadening contextual inference.
pub(in crate::generic_data) fn call_context_parameter_types_agree(
    syntax: &SyntaxTrees,
    left: &[TypeReferenceHandle],
    right: &[TypeReferenceHandle],
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| call_context_types_agree(syntax, *left, *right))
}

pub(in crate::generic_data) fn call_context_types_agree(
    syntax: &SyntaxTrees,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    match (
        syntax.tables.type_references.type_reference(left),
        syntax.tables.type_references.type_reference(right),
    ) {
        (TypeReferenceNode::Named(left), TypeReferenceNode::Named(right)) => left == right,
        (TypeReferenceNode::SelfType, TypeReferenceNode::SelfType)
        | (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
        (
            TypeReferenceNode::Reference {
                referee: left,
                access: left_access,
                lifetime: left_lifetime,
            },
            TypeReferenceNode::Reference {
                referee: right,
                access: right_access,
                lifetime: right_lifetime,
            },
        ) => {
            left_access == right_access
                && left_lifetime == right_lifetime
                && call_context_types_agree(syntax, *left, *right)
        }
        (
            TypeReferenceNode::Slice { element_type: left },
            TypeReferenceNode::Slice {
                element_type: right,
            },
        ) => call_context_types_agree(syntax, *left, *right),
        (
            TypeReferenceNode::FixedArray {
                element_type: left,
                length: left_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: right,
                length: right_length,
            },
        ) => left_length == right_length && call_context_types_agree(syntax, *left, *right),
        (
            TypeReferenceNode::Generic {
                base_name: left_base,
                lifetime_arguments: left_lifetimes,
                arguments: left_arguments,
            },
            TypeReferenceNode::Generic {
                base_name: right_base,
                lifetime_arguments: right_lifetimes,
                arguments: right_arguments,
            },
        ) => {
            left_base == right_base
                && left_lifetimes == right_lifetimes
                && call_context_parameter_types_agree(
                    syntax,
                    syntax
                        .tables
                        .type_references
                        .type_reference_handles(*left_arguments),
                    syntax
                        .tables
                        .type_references
                        .type_reference_handles(*right_arguments),
                )
        }
        (
            TypeReferenceNode::DynamicTrait {
                name: left_name,
                conformance: left_conformance,
            },
            TypeReferenceNode::DynamicTrait {
                name: right_name,
                conformance: right_conformance,
            },
        ) => left_name == right_name && left_conformance == right_conformance,
        // Constraint expressions and pre-evaluated const expressions use
        // occurrence-specific handles here. Treat them as no consensus until
        // the normalized typed identity is available.
        (TypeReferenceNode::Constrained { .. }, TypeReferenceNode::Constrained { .. })
        | (TypeReferenceNode::ConstExpression(_), TypeReferenceNode::ConstExpression(_)) => false,
        _ => false,
    }
}
