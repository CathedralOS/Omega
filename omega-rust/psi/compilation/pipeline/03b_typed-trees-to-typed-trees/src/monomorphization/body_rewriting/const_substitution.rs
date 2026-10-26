//! Substituting exact const binders and normalizing closed const recipes.

use crate::monomorphization::{
    Candidate, ExpressionNode, StatementNode, SymbolHandle, TypeReferenceNode, TypedTrees,
};
use numerics::literals::{IntegerLiteral, IntegerRadix};
use typed_trees::const_integer::{self, ConstIntegerExpression};
use typed_trees::expression::StaticMachineArgument;
use typed_trees::name::Identifier;

pub(crate) fn fixed_array_const_replacements(
    program: &dyn ::validation::CheckingInput,
    candidate: &Candidate,
) -> Vec<(SymbolHandle, String, usize)> {
    candidate
        .template
        .const_parameters
        .iter()
        .zip(candidate.const_bindings.iter())
        .filter_map(|((symbol, name, _), binding)| {
            let TypeReferenceNode::Named {
                name: binding_name, ..
            } = program
                .type_reference_table
                .type_reference(binding.expect("complete const specialization"))
            else {
                return None;
            };
            Some((*symbol, name.clone(), binding_name.as_str().parse().ok()?))
        })
        .collect()
}

pub(crate) fn substitute_fixed_array_const_parameters(
    program: &mut TypedTrees,
    replacements: &[(SymbolHandle, String, usize)],
    type_start: Option<usize>,
) {
    let updates = program
        .type_reference_table
        .fixed_array_lengths()
        .filter(|(handle, _)| type_start.is_none_or(|start| handle.arena_index() as usize >= start))
        .filter_map(|(handle, length)| {
            let typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } = length
            else {
                return None;
            };
            replacements
                .iter()
                .find(|(parameter_symbol, parameter_name, _)| {
                    parameter_symbol == symbol
                        || (!parameter_symbol.is_valid()
                            && !symbol.is_valid()
                            && parameter_name == name.as_str())
                })
                .map(|(_, _, value)| (handle, *value))
        })
        .collect::<Vec<_>>();
    for (handle, value) in updates {
        program
            .type_reference_table
            .set_fixed_array_length(handle, value);
    }
}

/// Only cloned type-expression roots belong to this specialization. Structural
/// traversal reaches constructor fields, but never a callee's declaration body.
pub(crate) fn substitute_const_index_expression_parameters(
    source: Option<&dyn ::validation::CheckingInput>,
    program: &mut TypedTrees,
    candidate: &Candidate,
    type_start: usize,
) -> Result<(), String> {
    let mut replacements = Vec::new();
    for (index, ((parameter, _, carrier), binding)) in candidate
        .template
        .const_parameters
        .iter()
        .zip(candidate.const_bindings.iter())
        .enumerate()
    {
        if candidate.runtime_value_bindings[index].is_some() {
            continue;
        }
        let binding = crate::monomorphization::copy_type_reference(
            source,
            program,
            binding.expect("complete const specialization"),
            &[],
        );
        let carrier = if let Some((index, _)) = candidate
            .template
            .type_parameters
            .iter()
            .enumerate()
            .find(|(_, (symbol, _))| {
                matches!(source.unwrap_or(program).type_reference_table.type_reference(*carrier),
                    TypeReferenceNode::Named { symbol: actual, .. } if actual == symbol)
            }) {
            crate::monomorphization::copy_type_reference(
                source,
                program,
                candidate.type_bindings[index].expect("complete type specialization"),
                &[],
            )
        } else {
            crate::monomorphization::copy_type_reference(source, program, *carrier, &[])
        };
        replacements.push((*parameter, binding, carrier));
    }
    let roots = program
        .type_reference_table
        .const_expression_sites()
        .into_iter()
        .filter(|(handle, _)| handle.arena_index() as usize >= type_start)
        .map(|(_, expression)| expression)
        .collect::<Vec<_>>();
    let mut expressions = Vec::new();
    for root in roots {
        validation::collect_expression_tree(program, root, &mut expressions);
    }
    for expression in expressions {
        let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
            continue;
        };
        let Some((_, binding, carrier)) = replacements
            .iter()
            .find(|(symbol, _, _)| path.symbol.is_valid() && path.symbol == *symbol)
        else {
            continue;
        };
        let value = crate::monomorphization::materialize_static_value(program, *binding, *carrier)
            .ok_or_else(|| {
                "const index binder has no exact canonical value under its declared carrier"
                    .to_owned()
            })?;
        *program.expression_table.expression_mut(expression) = value;
    }
    Ok(())
}

/// Replace a closed const recipe with its existing canonical static argument.
/// Scalar decimal arguments retain their established representation.
///
/// `step<Degree - 1>` inside `power<2>` is `step<2 - 1>` after binder
/// substitution. Leaving that tree as a type reference never selects `step<1>`.
pub(crate) fn fold_closed_const_machine_arguments(program: &mut TypedTrees) -> Result<(), String> {
    let expression_handles = program
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Call(_)).then_some(handle)
        })
        .collect::<Vec<_>>();
    for handle in expression_handles {
        let application = program
            .expression_table
            .call_node(handle)
            .expect("call roster")
            .authored_machine_application;
        let owner = if application.is_valid() {
            program
                .expression_table
                .static_machine_application(application)
                .lexical_owner
        } else {
            SymbolHandle::invalid()
        };
        let mut native_query = program
            .expression_table
            .call_node(handle)
            .and_then(|call| call.native_code_layout.clone());
        if let Some(query) = &mut native_query
            && normalize_const_arguments(program, owner, std::slice::from_mut(&mut query.operand))?
            && let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle)
        {
            call.native_code_layout = native_query;
        }
        let mut arguments = program
            .expression_table
            .call_node(handle)
            .expect("call roster")
            .machine_arguments
            .clone();
        if normalize_const_arguments(program, owner, &mut arguments)? {
            let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
                continue;
            };
            call.machine_arguments = arguments;
        }
    }
    let state_spans = program
        .machines()
        .iter()
        .map(|machine| machine.states)
        .collect::<Vec<_>>();
    let mut statement_handles = Vec::new();
    for states in state_spans {
        for state in program.machine_states.span_or_empty(states) {
            for (handle, statement) in program
                .statement_table
                .iter_statements(state.statement_nodes)
            {
                if statement.as_call().is_some() {
                    statement_handles.push(handle);
                }
            }
        }
    }
    for handle in statement_handles {
        let application = program
            .statement_table
            .statement(handle)
            .as_call()
            .expect("call roster")
            .authored_machine_application;
        let owner = if application.is_valid() {
            program
                .expression_table
                .static_machine_application(application)
                .lexical_owner
        } else {
            SymbolHandle::invalid()
        };
        let mut arguments = program
            .statement_table
            .statement(handle)
            .as_call()
            .expect("call roster")
            .machine_arguments
            .clone();
        if normalize_const_arguments(program, owner, &mut arguments)? {
            let StatementNode::Call(call) = program.statement_table.statement_mut(handle) else {
                continue;
            };
            call.machine_arguments = arguments;
        }
    }
    Ok(())
}

/// Normalize a derived operand list without replacing its authored application.
/// Callers own the scratch list (or the live executable arguments); immutable
/// checking still reads every original recipe and its formation obligations.
pub(crate) fn normalize_const_arguments(
    program: &mut TypedTrees,
    owner: SymbolHandle,
    arguments: &mut [StaticMachineArgument],
) -> Result<bool, String> {
    let mut changed = false;
    for index in 0..arguments.len() {
        if arguments[index].application.is_some() {
            let mut nested = arguments[index]
                .application
                .as_ref()
                .expect("nested static application")
                .arguments
                .clone();
            if normalize_const_arguments(program, owner, &mut nested)? {
                arguments[index]
                    .application
                    .as_mut()
                    .expect("nested static application")
                    .arguments = nested;
                changed = true;
            }
        }
        let Some(value) = closed_const_argument(program, &arguments[index]) else {
            if let Some(value) = closed_structural_argument(program, owner, &arguments[index])? {
                let atom = value.atom();
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: SymbolHandle::invalid(),
                        name: Identifier::generated(atom.clone()),
                    });
                arguments[index] = StaticMachineArgument {
                    path: vec![Identifier::generated(atom)].into_boxed_slice(),
                    ..StaticMachineArgument::default()
                };
                changed = true;
            }
            continue;
        };
        remember_integer_type(program, value);
        arguments[index] = StaticMachineArgument {
            const_literal: Some(integer_literal(value)),
            ..StaticMachineArgument::default()
        };
        changed = true;
    }
    Ok(changed)
}

fn closed_const_argument(program: &TypedTrees, argument: &StaticMachineArgument) -> Option<i128> {
    let TypeReferenceNode::ConstExpression(expression) = program
        .type_reference_table
        .type_reference(argument.type_reference)
    else {
        return None;
    };
    match const_integer::classify(&program.expression_table, *expression, &|_, _| false)? {
        ConstIntegerExpression::Closed(value) => Some(value),
        ConstIntegerExpression::Open => None,
    }
}

fn remember_integer_type(program: &mut TypedTrees, value: i128) {
    let spelling = value.to_string();
    let exists = program
        .type_reference_table
        .named_references()
        .any(|(_, symbol, name)| !symbol.is_valid() && name == spelling);
    if exists {
        return;
    }
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated(spelling),
        });
}

fn integer_literal(value: i128) -> IntegerLiteral {
    if let Ok(narrow) = i64::try_from(value) {
        return IntegerLiteral::from_value(narrow);
    }
    let negative = value < 0;
    let digits = value.unsigned_abs().to_string();
    IntegerLiteral::from_parts(negative, IntegerRadix::Decimal, &digits)
        .expect("decimal digits of a closed const integer")
}

fn closed_structural_argument(
    program: &TypedTrees,
    owner: SymbolHandle,
    argument: &StaticMachineArgument,
) -> Result<Option<language_semantics::const_value::CanonicalConstValue>, String> {
    if !argument.type_reference.is_valid() {
        return Ok(None);
    }
    let TypeReferenceNode::ConstExpression(expression) = program
        .type_reference_table
        .type_reference(argument.type_reference)
    else {
        return Ok(None);
    };
    let carrier = match program.expression_table.expression(*expression) {
        ExpressionNode::StructLiteral(literal) => {
            let retained = program
                .expression_table
                .constructed_value_type_reference(*expression);
            retained.is_valid().then_some(retained).or_else(|| {
                program
                    .type_reference_table
                    .find_named_type_reference(literal.type_symbol)
            })
        }
        _ => {
            validation::parameter_expression_result_type_reference(program, owner, &[], *expression)
        }
    };
    let Some(carrier) = carrier else {
        return Ok(None);
    };
    validation::normalize_closed_const_expression(program, owner, *expression, carrier)
}
