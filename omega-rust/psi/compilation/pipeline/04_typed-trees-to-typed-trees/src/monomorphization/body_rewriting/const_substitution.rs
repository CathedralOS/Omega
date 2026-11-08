//! Substituting exact const binders and normalizing closed const recipes.

use crate::monomorphization::{
    Candidate, ExpressionNode, StatementNode, SymbolHandle, TypeReferenceNode, TypedTrees,
};
use crate::tables::{expressions_of, expressions_of_mut};
use numerics::literals::{IntegerLiteral, IntegerRadix};
use typed_trees::const_integer::{self, ConstIntegerExpression};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, StaticMachineArgument};
use typed_trees::name::Identifier;
use typed_trees::statement::TransitionTargetNode;
use typed_trees::types::TypeReferenceHandle;
use validation::parameter_expression_result_type_reference;

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
    if replacements.is_empty() {
        return;
    }
    let updates = match type_start {
        Some(start) => program
            .type_reference_table
            .references_from(start)
            .filter_map(|(handle, node)| {
                let TypeReferenceNode::FixedArray {
                    length: typed_trees::types::FixedArrayLength::ConstParameter { symbol, name },
                    ..
                } = node
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
            .collect::<Vec<_>>(),
        None => program
            .type_reference_table
            .fixed_array_lengths()
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
            .collect::<Vec<_>>(),
    };
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
    if candidate.template.const_parameters.is_empty() {
        return Ok(());
    }
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
        )?;
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
            )?
        } else {
            crate::monomorphization::copy_type_reference(source, program, *carrier, &[])?
        };
        replacements.push((*parameter, binding, carrier));
    }
    let roots = program
        .type_reference_table
        .references_from(type_start)
        .filter_map(|(_, node)| match node {
            TypeReferenceNode::ConstExpression(expression) => Some(*expression),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut expressions = Vec::new();
    for root in roots {
        validation::collect_expression_tree(program, root, &mut expressions);
    }
    for expression in expressions {
        let ExpressionNode::Name(path) = expressions_of(program).expression(expression) else {
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
        *expressions_of_mut(program).expression_mut(expression) = value;
    }
    Ok(())
}

/// Replace a closed const recipe with its existing canonical static argument.
/// Scalar decimal arguments retain their established representation.
///
/// `step<Degree - 1>` inside `power<2>` is `step<2 - 1>` after binder
/// substitution. Leaving that tree as a type reference never selects `step<1>`.
pub(crate) fn fold_closed_const_machine_arguments(program: &mut TypedTrees) -> Result<(), String> {
    let state_spans = program
        .machines()
        .iter()
        .map(|machine| machine.states)
        .collect::<Vec<_>>();
    fold_closed_const_machine_arguments_in(program, 0, &state_spans)
}

pub(crate) fn fold_closed_const_machine_arguments_in(
    program: &mut TypedTrees,
    expression_start: usize,
    state_spans: &[arena::HandleSpan<typed_trees::state::State>],
) -> Result<(), String> {
    let expression_handles = expressions_of(program)
        .iter_expressions_from(expression_start)
        .filter_map(|(handle, expression)| {
            matches!(expression, ExpressionNode::Call(_)).then_some(handle)
        })
        .collect::<Vec<_>>();
    for handle in expression_handles {
        let application = expressions_of(program)
            .call_node(handle)
            .expect("call roster")
            .authored_machine_application;
        let owner = if application.is_valid() {
            expressions_of(program)
                .static_machine_application(application)
                .lexical_owner
        } else {
            SymbolHandle::invalid()
        };
        let mut request_call = expressions_of(program)
            .call_node(handle)
            .expect("call roster")
            .clone();
        let mut request_changed = false;
        for argument in request_call.request_static_arguments_mut() {
            request_changed |=
                normalize_const_arguments(program, owner, std::slice::from_mut(argument))?;
        }
        if request_changed
            && let ExpressionNode::Call(call) = expressions_of_mut(program).expression_mut(handle)
        {
            *call = request_call;
        }
        let has_machine_args = expressions_of(program)
            .call_node(handle)
            .is_some_and(|call| !call.machine_arguments.is_empty());
        if !has_machine_args {
            continue;
        }
        let call = expressions_of(program)
            .call_node(handle)
            .expect("call roster");
        let mut arguments = call.machine_arguments.clone();
        let carriers = callee_value_carriers(program, call.target_symbol, &call.machine_arguments);
        if normalize_const_arguments_for(program, owner, &carriers, &mut arguments)? {
            let ExpressionNode::Call(call) = expressions_of_mut(program).expression_mut(handle)
            else {
                continue;
            };
            call.machine_arguments = arguments;
        }
    }
    let mut statement_handles = Vec::new();
    for states in state_spans {
        for state in program.machine_states.span_or_empty(*states) {
            for (handle, statement) in program
                .statement_table
                .iter_statements(state.statement_nodes)
            {
                if statement.as_call().is_some_and(|call| {
                    !call.machine_arguments.is_empty()
                        || call.installed_interrupt_operation.is_some()
                }) {
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
            expressions_of(program)
                .static_machine_application(application)
                .lexical_owner
        } else {
            SymbolHandle::invalid()
        };
        let call = program
            .statement_table
            .statement(handle)
            .as_call()
            .expect("call roster");
        let mut call = call.clone();
        let carriers = callee_value_carriers(program, call.target_symbol, &call.machine_arguments);
        let mut changed =
            normalize_const_arguments_for(program, owner, &carriers, &mut call.machine_arguments)?;
        for argument in call.request_static_arguments_mut() {
            changed |= normalize_const_arguments(program, owner, std::slice::from_mut(argument))?;
        }
        if changed {
            let StatementNode::Call(destination) = program.statement_table.statement_mut(handle)
            else {
                continue;
            };
            *destination = call;
        }
    }
    Ok(())
}

/// The selected machine's declared value carriers, aligned with the call's
/// pack. A source call may name the declaration or its entry state. Machine
/// evidence binders are conformance bounds rather than telescope entries, so
/// their implementation actuals occupy no declared carrier position.
fn callee_value_carriers(
    program: &TypedTrees,
    target: SymbolHandle,
    arguments: &[StaticMachineArgument],
) -> Vec<Option<TypeReferenceHandle>> {
    if !target.is_valid() {
        return Vec::new();
    }
    let Some(machine) = program
        .machine_by_symbol(target)
        .or_else(|| program.machine_holding_state(target))
    else {
        return Vec::new();
    };
    let mut declared =
        declared_value_carriers(program.machine_type_parameters(machine)).into_iter();
    let carriers = arguments
        .iter()
        .map(|argument| {
            let evidence = !machine.conformance_bounds.is_empty()
                && program.conformance_by_symbol(argument.symbol).is_some();
            if evidence {
                Some(None)
            } else {
                declared.next()
            }
        })
        .collect::<Option<Vec<_>>>();
    match carriers {
        Some(carriers) if declared.next().is_none() => carriers,
        _ => Vec::new(),
    }
}

/// Normalize a derived operand list without replacing its authored application.
/// Callers own the scratch list (or the live executable arguments); immutable
/// checking still reads every original recipe and its formation obligations.
pub(crate) fn normalize_const_arguments(
    program: &mut TypedTrees,
    owner: SymbolHandle,
    arguments: &mut [StaticMachineArgument],
) -> Result<bool, String> {
    normalize_const_arguments_for(program, owner, &[], arguments)
}

/// `carriers` are the receiving telescope's declared value carriers, aligned
/// with `arguments` (empty when the receiver is not known here). A recipe
/// with no nominal carrier of its own, such as an array literal, lands only
/// under that destination carrier.
fn normalize_const_arguments_for(
    program: &mut TypedTrees,
    owner: SymbolHandle,
    carriers: &[Option<TypeReferenceHandle>],
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
            let nested_carriers = program
                .conformance_by_symbol(arguments[index].symbol)
                .map(|declaration| {
                    declared_value_carriers(program.conformance_type_parameters(declaration))
                })
                .unwrap_or_default();
            if normalize_const_arguments_for(program, owner, &nested_carriers, &mut nested)? {
                arguments[index]
                    .application
                    .as_mut()
                    .expect("nested static application")
                    .arguments = nested;
                changed = true;
            }
        }
        if let Some(application) = &arguments[index].application
            && application.member.is_valid()
        {
            let handle = application.member;
            let mut member = expressions_of(program)
                .static_member(handle)
                .ok_or("qualified static member has no owning expression record")?
                .clone();
            if normalize_const_arguments_for(
                program,
                owner,
                &[],
                std::slice::from_mut(&mut member),
            )? {
                *expressions_of_mut(program)
                    .static_member_mut(handle)
                    .ok_or("qualified static member lost its owning expression record")? = member;
                changed = true;
            }
        }
        let Some(value) = closed_const_argument(program, &arguments[index]) else {
            let expected = carriers
                .get(index)
                .copied()
                .flatten()
                .filter(|_| carriers.len() == arguments.len());
            if let Some(value) =
                closed_structural_argument(program, owner, &arguments[index], expected)?
            {
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
    match const_integer::classify(expressions_of(program), *expression, &|_, _| false)? {
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
    expected: Option<TypeReferenceHandle>,
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
    let carrier = match expressions_of(program).expression(*expression) {
        ExpressionNode::StructLiteral(literal) => {
            let retained = expressions_of(program).constructed_value_type_reference(*expression);
            retained.is_valid().then_some(retained).or_else(|| {
                program
                    .type_reference_table
                    .find_named_type_reference(literal.type_symbol)
            })
        }
        // An array literal has no nominal type of its own; its elements
        // land under the receiving binder's declared array carrier.
        ExpressionNode::ArrayLiteral(_) => expected,
        _ => parameter_expression_result_type_reference(program, owner, &[], *expression),
    };
    let Some(carrier) = carrier else {
        return Ok(None);
    };
    validation::normalize_closed_const_expression(program, owner, *expression, carrier)
}

/// A specialization whose value actuals are closed integers rewrites
/// `Nodes == 0 && Total == 0` into `3 == 0 && 7 == 0`. The arm that calls
/// under that fact is not a live obligation of this instance. Other
/// instances keep the arm, including one whose own actuals are still the
/// runtime parameters.
pub(crate) fn drop_refuted_closed_transition_arms(
    program: &mut TypedTrees,
    candidate: &Candidate,
    machine: &typed_trees::machine::Machine,
) {
    let mut rewrites = Vec::new();
    for (index, state) in program.machine_states(machine).iter().enumerate() {
        let statements = program
            .statement_table
            .statements(state.statement_nodes)
            .to_vec();
        if !statements.iter().any(|statement| {
            transition_arm_requires_are_closed_false(program, candidate, machine, statement)
        }) {
            continue;
        }
        let live = statements
            .into_iter()
            .filter(|statement| {
                !transition_arm_requires_are_closed_false(program, candidate, machine, statement)
            })
            .collect::<Vec<_>>();
        if live
            .iter()
            .any(|statement| matches!(statement, StatementNode::Transition(_)))
        {
            rewrites.push((index, live));
        }
    }
    for (index, live) in rewrites {
        let body = program.statement_table.insert_statements(live);
        program.machine_states_mut(machine)[index].statement_nodes = body;
    }
}

fn transition_callee_symbol(
    program: &TypedTrees,
    target: typed_trees::statement::TransitionTargetHandle,
) -> Option<SymbolHandle> {
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { path, .. } => path.symbol.is_valid().then_some(path.symbol),
        TransitionTargetNode::Value(expression) => {
            let expression = *expression;
            let ExpressionNode::Call(call) = expressions_of(program).expression(expression) else {
                return None;
            };
            call.target_symbol.is_valid().then_some(call.target_symbol)
        }
        TransitionTargetNode::Terminal => None,
    }
}

fn transition_arm_requires_are_closed_false(
    program: &TypedTrees,
    candidate: &Candidate,
    machine: &typed_trees::machine::Machine,
    statement: &StatementNode,
) -> bool {
    let StatementNode::Transition(transition) = statement else {
        return false;
    };
    let Some(path_symbol) = transition_callee_symbol(program, transition.target) else {
        return false;
    };
    // A nested `done` is its own machine. Its entry is not a state of the
    // caller, and the caller clone is not indexed yet when the arm is one of
    // its own states. Prefer the indexed owner, then this clone's span.
    let owner = program.machine_holding_state(path_symbol).or(program
        .machine_states(machine)
        .iter()
        .any(|state| state.symbol == path_symbol)
        .then_some(machine));
    let Some(owner) = owner else {
        return false;
    };
    let Some(state) = program
        .machine_states(owner)
        .iter()
        .find(|state| state.symbol == path_symbol)
    else {
        return false;
    };
    let requirements = program
        .contract_expressions(
            program
                .machine_contracts(owner)
                .iter()
                .chain(program.state_contracts(state)),
            typed_trees::signature::SignatureContractKind::Requires,
        )
        .collect::<Vec<_>>();
    requirements
        .into_iter()
        .any(|expression| closed_bool(program, candidate, expression) == Some(false))
}

fn closed_bool(
    program: &TypedTrees,
    candidate: &Candidate,
    expression: ExpressionHandle,
) -> Option<bool> {
    match expressions_of(program).expression(expression) {
        ExpressionNode::Boolean(value) => Some(*value),
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => Some(
            closed_bool(program, candidate, binary.left)?
                && closed_bool(program, candidate, binary.right)?,
        ),
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
            let left = integer_spelling(program, candidate, binary.left)?;
            let right = integer_spelling(program, candidate, binary.right)?;
            Some(left == right)
        }
        _ => None,
    }
}

fn integer_spelling(
    program: &TypedTrees,
    candidate: &Candidate,
    expression: ExpressionHandle,
) -> Option<String> {
    match expressions_of(program).expression(expression) {
        ExpressionNode::Integer(literal) => Some(literal.canonical_spelling().as_str().to_owned()),
        ExpressionNode::Name(path) => {
            let origin = program.symbols.get(path.symbol).generated_from;
            let index = candidate
                .template
                .const_parameters
                .iter()
                .position(|(parameter, _, _)| *parameter == path.symbol || *parameter == origin)?;
            let binding = candidate.const_bindings[index]?;
            let literal = program.static_const_literal_from_type_reference(binding)?;
            Some(literal.canonical_spelling().as_str().to_owned())
        }
        _ => None,
    }
}

fn declared_value_carriers(
    parameters: &[typed_trees::data::TypeParameter],
) -> Vec<Option<TypeReferenceHandle>> {
    parameters
        .iter()
        .map(|parameter| match parameter.kind {
            typed_trees::data::TypeParameterKind::Const { type_reference }
            | typed_trees::data::TypeParameterKind::Value { type_reference } => {
                Some(type_reference)
            }
            _ => None,
        })
        .collect()
}
