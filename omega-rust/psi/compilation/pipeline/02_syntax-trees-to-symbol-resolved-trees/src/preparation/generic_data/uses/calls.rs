//! Expected types from calls, returns and receiver paths.
use super::super::{
    ExpressionHandle, ExpressionNode, HandleSpan, HashMap, HashSet, Identifier, StatementNode,
    SyntaxTrees, TypeReferenceHandle, TypeReferenceNode,
};
use crate::preparation::generic_data::uses::{
    ConstructorFrontier, collect_statement_expression_handles, named_type_name,
    relabel_data_literal_for_expected_type,
};

use crate::preparation::generic_data::Instantiation;

use crate::preparation::declaration_selection;

/// Closed callable signatures provide exact contextual types without inferring
/// from literal fields. Free-machine overloads and concrete attached-machine
/// overloads each contribute a context only when every same-name candidate on
/// the exact owner agrees on one parameter signature. An explicitly typed
/// binding receiver names one exact nominal owner. Computed, chained, and
/// dynamic receiver selection remains resolver-owned and fails closed here.
pub(in crate::preparation::generic_data) fn relabel_closed_data_uses_in_exact_calls_and_returns(
    syntax: &mut SyntaxTrees,
    instances: &[Instantiation],
    selection: Option<&declaration_selection::ConstantSelection>,
    declarations: &[syntax_trees::item::ItemHandle],
) {
    let mut signatures = HashMap::<String, Option<Vec<TypeReferenceHandle>>>::new();
    let mut attached_signatures =
        HashMap::<(String, String), Option<Vec<TypeReferenceHandle>>>::new();
    for item in syntax.root_items() {
        let Some(machine) = item.as_machine() else {
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
            .map(|parameter| {
                syntax
                    .tables
                    .items
                    .state_parameter(*parameter)
                    .type_reference
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

    let concrete_states = super::selected_machine_state_handles(syntax, declarations);
    for state_handle in concrete_states {
        let state = syntax.tables.items.state(state_handle).clone();
        let statements = syntax.tables.items.statements(state.statements).to_vec();
        let final_statement = statements.last().copied();
        let mut local_owner_types = HashMap::<String, String>::new();
        for parameter in syntax.tables.items.state_parameters(state.parameters) {
            let parameter = syntax.tables.items.state_parameter(*parameter);
            if let Some(type_name) = named_type_name(syntax, parameter.type_reference) {
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
        for (index, statement) in statements.iter().enumerate() {
            let frontier = ConstructorFrontier {
                parameters: state.parameters,
                prior_statements: &statements[..index],
            };
            let mut reachable = HashSet::new();
            collect_statement_expression_handles(syntax, *statement, &mut reachable);
            let mut reachable = reachable.into_iter().collect::<Vec<_>>();
            reachable.sort_unstable_by_key(|handle| (handle.generation(), handle.arena_index()));
            let calls = reachable
                .into_iter()
                .filter_map(|handle| match syntax.expressions.expression(handle) {
                    ExpressionNode::Call(call) if !call.receiver.is_valid() => {
                        Some((call.target.clone(), call.arguments, None))
                    }
                    ExpressionNode::Call(call) => {
                        exact_expression_receiver_owner(syntax, call.receiver, &local_owner_types)
                            .map(|owner| (call.target.clone(), call.arguments, Some(owner)))
                    }
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
                        instances,
                        selection,
                        &frontier,
                    );
                }
            }
            match syntax.tables.statements.statement(*statement) {
                StatementNode::Expression(value)
                    if state.return_type.is_valid() && Some(*statement) == final_statement =>
                {
                    relabel_data_literal_for_expected_type(
                        syntax,
                        *value,
                        state.return_type,
                        instances,
                        selection,
                        &frontier,
                    );
                }
                StatementNode::Call(call) => {
                    let owner =
                        exact_statement_receiver_owner(syntax, call.receiver, &local_owner_types);
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
                            instances,
                            selection,
                            &frontier,
                        );
                    }
                }
                StatementNode::Transition(transition) => {
                    for target in transition.targets() {
                        if let syntax_trees::statement::TransitionTargetNode::Value(value) =
                            syntax.tables.statements.transition_target(target)
                        {
                            relabel_data_literal_for_expected_type(
                                syntax,
                                *value,
                                state.return_type,
                                instances,
                                selection,
                                &frontier,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

pub(in crate::preparation::generic_data) fn exact_statement_receiver_owner(
    syntax: &SyntaxTrees,
    receiver: HandleSpan<Identifier>,
    local_owner_types: &HashMap<String, String>,
) -> Option<String> {
    let members = syntax.tables.statements.identifier_path_members(receiver);
    match members {
        [local] => local_owner_types.get(local.as_str()).cloned(),
        _ => None,
    }
}

pub(in crate::preparation::generic_data) fn exact_expression_receiver_owner(
    syntax: &SyntaxTrees,
    receiver: ExpressionHandle,
    local_owner_types: &HashMap<String, String>,
) -> Option<String> {
    match syntax.expressions.expression(receiver) {
        ExpressionNode::Name(path) => {
            let [local] = syntax.expressions.identifier_path_members(*path) else {
                return None;
            };
            local_owner_types.get(local.as_str()).cloned()
        }
        _ => None,
    }
}

/// Conservative pre-resolution equality for the expected parameter types that
/// drive generic-literal relabeling. Handles are arena-local occurrences, so
/// comparing the handles themselves would make two textually identical
/// overload signatures look different. Unsupported or expression-bearing
/// details fail closed rather than broadening contextual inference.
pub(in crate::preparation::generic_data) fn call_context_parameter_types_agree(
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

pub(in crate::preparation::generic_data) fn call_context_types_agree(
    syntax: &SyntaxTrees,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    match (
        syntax.tables.type_references.type_reference(left),
        syntax.tables.type_references.type_reference(right),
    ) {
        (TypeReferenceNode::Named(left), TypeReferenceNode::Named(right)) => left == right,
        (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
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
                trait_name: left_name,
                trait_arguments: left_arguments,
            },
            TypeReferenceNode::DynamicTrait {
                trait_name: right_name,
                trait_arguments: right_arguments,
            },
        ) => {
            left_name == right_name
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
        // Constraint expressions and pre-evaluated const expressions use
        // occurrence-specific handles here. Treat them as no consensus until
        // the normalized typed identity is available.
        (TypeReferenceNode::Constrained { .. }, TypeReferenceNode::Constrained { .. })
        | (TypeReferenceNode::ConstExpression(_), TypeReferenceNode::ConstExpression(_)) => false,
        _ => false,
    }
}
