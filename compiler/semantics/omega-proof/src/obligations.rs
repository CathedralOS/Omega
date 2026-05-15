use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{BinaryOperator, Expression, FloatLiteral};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::name::ProgramName;
use omega_typed_trees::signature::StateParameter;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    Assignment, Call, Transition, TransitionGuard, TransitionTarget,
};
use omega_typed_trees::types::{TypeConstraint, TypeReference};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofPlan {
    pub obligations: Vec<ProofObligation>,
    pub type_constraints: Arena<TypeConstraint>,
}

impl ProofPlan {
    fn store_constraints(&mut self, constraints: &[TypeConstraint]) -> HandleSpan<TypeConstraint> {
        self.type_constraints
            .insert_many(constraints.iter().cloned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofObligation {
    BoundedAssignment(BoundedAssignmentObligation),
    BoundedCallArgument(BoundedCallArgumentObligation),
    BoundedInitializer(BoundedInitializerObligation),
    BoundedStateReturn(BoundedStateReturnObligation),
    BoundedValue(BoundedValueObligation),
    BoundedTransitionArgument(BoundedTransitionArgumentObligation),
    GuardedTransition(GuardedTransitionObligation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedValueObligation {
    pub owner: String,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardedTransitionObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: String,
    pub state_symbol: SymbolHandle,
    pub state: String,
    pub target: TransitionTarget,
    pub guard: TransitionGuard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedAssignmentObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: String,
    pub state_symbol: SymbolHandle,
    pub state: String,
    pub state_guard: Option<TransitionGuard>,
    pub target: Expression,
    pub value: Expression,
    pub value_constraints: HandleSpan<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedCallArgumentObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: String,
    pub state_symbol: SymbolHandle,
    pub state: String,
    pub receiver: Option<String>,
    pub target: String,
    pub parameter: String,
    pub argument: Expression,
    pub argument_constraints: HandleSpan<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedInitializerObligation {
    pub owner: String,
    pub value: Expression,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedStateReturnObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: String,
    pub state_symbol: SymbolHandle,
    pub state: String,
    pub value: Expression,
    pub value_constraints: HandleSpan<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedTransitionArgumentObligation {
    pub machine_symbol: SymbolHandle,
    pub machine: String,
    pub state_symbol: SymbolHandle,
    pub state: String,
    pub target: TransitionTarget,
    pub parameter: String,
    pub argument: Expression,
    pub argument_constraints: HandleSpan<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
    pub guard: TransitionGuard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntegerRange {
    minimum: i64,
    maximum: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FloatRange {
    minimum: f64,
    maximum: f64,
}

pub fn build_proof_plan(program: &TypedTrees) -> ProofPlan {
    let mut proof_plan = ProofPlan::default();

    for machine in program.machines() {
        for owned_data in program.machine_owned_data(machine) {
            collect_bounded_value_obligation(
                program,
                format!(
                    "machine `{}` owned data `{}`",
                    machine.name, owned_data.name
                ),
                &owned_data.type_reference,
                &mut proof_plan,
            );
            if let Some(initial_value) = &owned_data.initial_value {
                collect_bounded_initializer_obligation(
                    program,
                    format!(
                        "machine `{}` owned data `{}`",
                        machine.name, owned_data.name
                    ),
                    &owned_data.type_reference,
                    initial_value,
                    &mut proof_plan,
                );
            }
        }

        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                collect_bounded_value_obligation(
                    program,
                    format!(
                        "machine `{}` state `{}` parameter `{}`",
                        machine.name, state.name, parameter.name
                    ),
                    &parameter.type_reference,
                    &mut proof_plan,
                );
            }

            if let Some(return_type) = &state.return_type {
                collect_bounded_value_obligation(
                    program,
                    format!(
                        "machine `{}` state `{}` return value",
                        machine.name, state.name
                    ),
                    return_type,
                    &mut proof_plan,
                );
                collect_bounded_state_return_obligation(
                    program,
                    machine,
                    state,
                    return_type,
                    &mut proof_plan,
                );
            }

            for statement in program.state_statements(state) {
                let transition = match statement {
                    omega_typed_trees::statement::Statement::Assignment(assignment) => {
                        collect_bounded_assignment_obligation(
                            program,
                            machine,
                            state,
                            assignment,
                            &mut proof_plan,
                        );
                        continue;
                    }
                    omega_typed_trees::statement::Statement::Call(call) => {
                        collect_bounded_call_argument_obligations(
                            program,
                            machine,
                            state,
                            call,
                            &mut proof_plan,
                        );
                        continue;
                    }
                    omega_typed_trees::statement::Statement::Transition(transition) => transition,
                    _ => continue,
                };

                if let TransitionGuard::When(_) = &transition.guard {
                    proof_plan
                        .obligations
                        .push(ProofObligation::GuardedTransition(
                            GuardedTransitionObligation {
                                machine_symbol: machine.symbol,
                                machine: machine.name.to_string(),
                                state_symbol: state.symbol,
                                state: state.name.to_string(),
                                target: transition.target.clone(),
                                guard: transition.guard.clone(),
                            },
                        ));
                }

                collect_bounded_transition_argument_obligations(
                    program,
                    machine,
                    state,
                    transition,
                    &mut proof_plan,
                );
            }
        }
    }

    proof_plan
}

fn collect_bounded_value_obligation(
    program: &TypedTrees,
    owner: String,
    type_reference: &TypeReference,
    proof_plan: &mut ProofPlan,
) {
    match type_reference {
        TypeReference::Reference { referee, .. } => {
            collect_bounded_value_obligation(program, owner, referee, proof_plan);
        }
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));
            proof_plan
                .obligations
                .push(ProofObligation::BoundedValue(BoundedValueObligation {
                    owner,
                    base_type: base_type.as_ref().clone(),
                    constraints,
                }));
        }
        TypeReference::FixedArray { element_type, .. } => {
            collect_bounded_value_obligation(program, owner, element_type, proof_plan);
        }
        TypeReference::Slice { element_type } => {
            collect_bounded_value_obligation(program, owner, element_type, proof_plan);
        }
        TypeReference::Generic { .. } => {
            for argument in program.type_reference_arguments(type_reference) {
                collect_bounded_value_obligation(program, owner.clone(), argument, proof_plan);
            }
        }
        TypeReference::Named { name: _, .. } => {}
        TypeReference::Unit => {}
    }
}

fn collect_bounded_initializer_obligation(
    program: &TypedTrees,
    owner: String,
    type_reference: &TypeReference,
    value: &Expression,
    proof_plan: &mut ProofPlan,
) {
    match type_reference {
        TypeReference::Reference { referee, .. } => {
            collect_bounded_initializer_obligation(program, owner, referee, value, proof_plan);
        }
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));
            proof_plan
                .obligations
                .push(ProofObligation::BoundedInitializer(
                    BoundedInitializerObligation {
                        owner,
                        value: value.clone(),
                        base_type: base_type.as_ref().clone(),
                        constraints,
                    },
                ));
        }
        TypeReference::FixedArray { element_type, .. } => {
            collect_bounded_initializer_obligation(program, owner, element_type, value, proof_plan);
        }
        TypeReference::Slice { element_type } => {
            collect_bounded_initializer_obligation(program, owner, element_type, value, proof_plan);
        }
        TypeReference::Generic { .. } => {
            for argument in program.type_reference_arguments(type_reference) {
                collect_bounded_initializer_obligation(
                    program,
                    owner.clone(),
                    argument,
                    value,
                    proof_plan,
                );
            }
        }
        TypeReference::Named { name: _, .. } => {}
        TypeReference::Unit => {}
    }
}

fn collect_bounded_assignment_obligation(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    assignment: &Assignment,
    proof_plan: &mut ProofPlan,
) {
    let Some(TypeReference::Constrained {
        base_type,
        constraints,
    }) = expression_type_reference(program, machine, state, &assignment.target)
    else {
        return;
    };

    let value_constraints = expression_constraints(program, machine, state, &assignment.value);
    let value_constraints = proof_plan.store_constraints(&value_constraints);
    let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));
    let state_guard = incoming_state_guard(program, machine, state);

    proof_plan
        .obligations
        .push(ProofObligation::BoundedAssignment(
            BoundedAssignmentObligation {
                machine_symbol: machine.symbol,
                machine: machine.name.to_string(),
                state_symbol: state.symbol,
                state: state.name.to_string(),
                state_guard,
                target: assignment.target.clone(),
                value: assignment.value.clone(),
                value_constraints,
                base_type: base_type.as_ref().clone(),
                constraints,
            },
        ));
}

fn collect_bounded_transition_argument_obligations(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    transition: &Transition,
    proof_plan: &mut ProofPlan,
) {
    let Some((target_state, arguments)) =
        transition_target_state_and_arguments(program, state, &transition.target)
    else {
        return;
    };

    for (parameter, argument) in callable_parameters(program, target_state).zip(arguments.iter()) {
        let TypeReference::Constrained {
            base_type,
            constraints,
        } = &parameter.type_reference
        else {
            continue;
        };

        let argument_constraints = expression_constraints(program, machine, state, argument);
        let argument_constraints = proof_plan.store_constraints(&argument_constraints);
        let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));

        proof_plan
            .obligations
            .push(ProofObligation::BoundedTransitionArgument(
                BoundedTransitionArgumentObligation {
                    machine_symbol: machine.symbol,
                    machine: machine.name.to_string(),
                    state_symbol: state.symbol,
                    state: state.name.to_string(),
                    target: transition.target.clone(),
                    parameter: parameter.name.to_string(),
                    argument: argument.clone(),
                    argument_constraints,
                    base_type: base_type.as_ref().clone(),
                    constraints,
                    guard: transition.guard.clone(),
                },
            ));
    }
}

fn collect_bounded_call_argument_obligations(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &Call,
    proof_plan: &mut ProofPlan,
) {
    let Some(parameters) = call_target_parameters(program, call) else {
        return;
    };

    for (parameter, argument) in parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(program.call_arguments(call).iter())
    {
        let TypeReference::Constrained {
            base_type,
            constraints,
        } = &parameter.type_reference
        else {
            continue;
        };

        let argument_constraints = expression_constraints(program, machine, state, argument);
        let argument_constraints = proof_plan.store_constraints(&argument_constraints);
        let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));

        proof_plan
            .obligations
            .push(ProofObligation::BoundedCallArgument(
                BoundedCallArgumentObligation {
                    machine_symbol: machine.symbol,
                    machine: machine.name.to_string(),
                    state_symbol: state.symbol,
                    state: state.name.to_string(),
                    receiver: call.receiver.as_ref().map(display_name_path),
                    target: call.target.to_string(),
                    parameter: parameter.name.to_string(),
                    argument: argument.clone(),
                    argument_constraints,
                    base_type: base_type.as_ref().clone(),
                    constraints,
                },
            ));
    }
}

fn collect_bounded_state_return_obligation(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    return_type: &TypeReference,
    proof_plan: &mut ProofPlan,
) {
    let TypeReference::Constrained {
        base_type,
        constraints,
    } = return_type
    else {
        return;
    };
    let Some(omega_typed_trees::statement::Statement::Expression(value)) =
        program.state_statements(state).last()
    else {
        return;
    };

    let value_constraints = expression_constraints(program, machine, state, value);
    let value_constraints = proof_plan.store_constraints(&value_constraints);
    let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));

    proof_plan
        .obligations
        .push(ProofObligation::BoundedStateReturn(
            BoundedStateReturnObligation {
                machine_symbol: machine.symbol,
                machine: machine.name.to_string(),
                state_symbol: state.symbol,
                state: state.name.to_string(),
                value: value.clone(),
                value_constraints,
                base_type: base_type.as_ref().clone(),
                constraints,
            },
        ));
}

fn call_target_parameters<'program>(
    program: &'program TypedTrees,
    call: &Call,
) -> Option<&'program [StateParameter]> {
    state_by_symbol(program, call.target_symbol).map(|state| program.state_parameters(state))
}

fn display_name_path(path: &omega_typed_trees::expression::NamePath) -> String {
    path.members()
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn transition_target_state_and_arguments<'program>(
    program: &'program TypedTrees,
    state: &'program State,
    target: &'program TransitionTarget,
) -> Option<(&'program State, &'program [Expression])> {
    let TransitionTarget::Named { path, .. } = target else {
        return None;
    };

    state_by_symbol(program, path.symbol())
        .or_else(|| (path.members() == ["self"]).then_some(state))
        .map(|target_state| (target_state, program.transition_target_arguments(target)))
}

fn state_by_symbol(program: &TypedTrees, symbol: SymbolHandle) -> Option<&State> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .find(|state| state.symbol == symbol)
}

fn incoming_state_guard(
    program: &TypedTrees,
    machine: &Machine,
    target_state: &State,
) -> Option<TransitionGuard> {
    let mut guard: Option<TransitionGuard> = None;

    for source_state in program.machine_states(machine) {
        for statement in program.state_statements(source_state) {
            let omega_typed_trees::statement::Statement::Transition(transition) = statement else {
                continue;
            };

            let Some((resolved_target, _)) =
                transition_target_state_and_arguments(program, source_state, &transition.target)
            else {
                continue;
            };

            if resolved_target.symbol != target_state.symbol {
                continue;
            }

            let TransitionGuard::When(_) = &transition.guard else {
                return None;
            };

            match &guard {
                Some(existing)
                    if !guards_equivalent_for_precondition(existing, &transition.guard) =>
                {
                    return None;
                }
                Some(_) => {}
                None => guard = Some(transition.guard.clone()),
            }
        }
    }

    guard
}

fn guards_equivalent_for_precondition(left: &TransitionGuard, right: &TransitionGuard) -> bool {
    match (left, right) {
        (TransitionGuard::Always, TransitionGuard::Always) => true,
        (TransitionGuard::When(left), TransitionGuard::When(right)) => {
            expressions_equivalent_for_precondition(left, right)
        }
        _ => false,
    }
}

fn expressions_equivalent_for_precondition(left: &Expression, right: &Expression) -> bool {
    if left == right {
        return true;
    }

    match (left, right) {
        (Expression::Mutable(left), _) => expressions_equivalent_for_precondition(left, right),
        (_, Expression::Mutable(right)) => expressions_equivalent_for_precondition(left, right),
        (Expression::Name(left), Expression::Name(right)) => left.members() == right.members(),
        (Expression::Call(left), Expression::Call(right)) => {
            left.target == right.target
                && left.arguments.len() == right.arguments.len()
                && match (left.receiver.as_deref(), right.receiver.as_deref()) {
                    (Some(left_receiver), Some(right_receiver)) => {
                        expressions_equivalent_for_precondition(left_receiver, right_receiver)
                    }
                    (None, None) => true,
                    _ => false,
                }
                && left.arguments.iter().zip(&right.arguments).all(
                    |(left_argument, right_argument)| {
                        expressions_equivalent_for_precondition(left_argument, right_argument)
                    },
                )
        }
        (Expression::Member(left), Expression::Member(right)) => {
            left.member == right.member
                && expressions_equivalent_for_precondition(&left.receiver, &right.receiver)
        }
        (Expression::Binary(left), Expression::Binary(right)) => {
            left.operator == right.operator
                && expressions_equivalent_for_precondition(&left.left, &right.left)
                && expressions_equivalent_for_precondition(&left.right, &right.right)
        }
        _ => false,
    }
}

fn callable_parameters<'program>(
    program: &'program TypedTrees,
    state: &'program State,
) -> impl Iterator<Item = &'program StateParameter> {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
}

fn expression_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: &Expression,
) -> Vec<TypeConstraint> {
    match expression {
        Expression::Binary(binary) => {
            let left = expression_constraints(program, machine, state, &binary.left);
            let right = expression_constraints(program, machine, state, &binary.right);
            derived_binary_constraints(binary.operator, &left, &right)
        }
        Expression::Call(call) => {
            if let Some(constraints) =
                derived_builtin_call_constraints(program, machine, state, call)
            {
                return constraints;
            }

            if is_real_from_call(call.receiver.as_deref(), &call.target)
                && let [argument] = call.arguments.as_slice()
            {
                let mut constraints = call_expression_return_type(program, machine, state, call)
                    .map(|return_type| {
                        collect_constraints_in_state(program, machine, state, return_type)
                    })
                    .unwrap_or_default();
                let argument_constraints =
                    expression_constraints(program, machine, state, argument);
                constraints.extend(derived_real_from_constraints(&argument_constraints));

                if !constraints.is_empty() {
                    return constraints;
                }
            }

            if let Some(return_type) = call_expression_return_type(program, machine, state, call) {
                return collect_constraints_in_state(program, machine, state, return_type);
            }

            Vec::new()
        }
        Expression::Cast(cast) => expression_constraints(program, machine, state, &cast.value),
        Expression::Float(value) => float_literal_constraints(*value),
        Expression::Integer(value) => integer_literal_constraints(*value),
        Expression::Name(path) if path.members() == ["u32", "MAX"] => {
            integer_literal_constraints(u32::MAX as i64)
        }
        Expression::Member(_) | Expression::Mutable(_) | Expression::Name(_) => {
            expression_type_reference(program, machine, state, expression)
                .map(|type_reference| {
                    collect_constraints_in_state(program, machine, state, type_reference)
                })
                .unwrap_or_default()
        }
        Expression::ArrayLiteral(_)
        | Expression::Boolean(_)
        | Expression::Indexed(_)
        | Expression::String(_)
        | Expression::StructLiteral(_) => Vec::new(),
    }
}

fn expression_type_reference<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    state: &'program State,
    expression: &Expression,
) -> Option<&'program TypeReference> {
    match expression {
        Expression::Mutable(inner) => expression_type_reference(program, machine, state, inner),
        Expression::Name(path) => {
            if path.symbol().is_valid() {
                return type_reference_for_symbol(program, machine, state, path.symbol());
            }

            let name = match path.members() {
                [name] => name,
                [receiver, name] if receiver == "self" => name,
                _ => return None,
            };

            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.name == *name)
                .map(|parameter| &parameter.type_reference)
                .or_else(|| {
                    program.state_statements(state).iter().find_map(|statement| {
                        let omega_typed_trees::statement::Statement::LocalData(local_data) =
                            statement
                        else {
                            return None;
                        };

                        (local_data.name == *name).then_some(&local_data.type_reference)
                    })
                })
                .or_else(|| {
                    program
                        .machine_owned_data(machine)
                        .iter()
                        .find(|owned_data| owned_data.name == *name)
                        .map(|owned_data| &owned_data.type_reference)
                })
        }
        Expression::Member(member) => {
            expression_type_reference(program, machine, state, &member.receiver)
                .and_then(|receiver_type| {
                    data_field_type_reference(
                        program,
                        receiver_type,
                        member.member_symbol,
                        &member.member,
                    )
                })
                .or_else(|| {
                    type_reference_for_symbol(program, machine, state, member.member_symbol)
                })
        }
        _ => None,
    }
}

fn collect_constraints(program: &TypedTrees, type_reference: &TypeReference) -> Vec<TypeConstraint> {
    match type_reference {
        TypeReference::Reference { referee, .. } => collect_constraints(program, referee),
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            let mut derived = collect_constraints(program, base_type);
            derived.extend(type_constraints(program, *constraints).iter().cloned());
            augment_constraints_with_named_facts(&mut derived);
            derived
        }
        TypeReference::FixedArray { element_type, .. } => {
            collect_constraints(program, element_type)
        }
        TypeReference::Generic { .. } => program
            .type_reference_arguments(type_reference)
            .iter()
            .flat_map(|argument| collect_constraints(program, argument))
            .collect(),
        TypeReference::Slice { element_type } => collect_constraints(program, element_type),
        TypeReference::Named { name, .. } => primitive_constraints(name),
        TypeReference::Unit => Vec::new(),
    }
}

fn collect_constraints_in_state(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    type_reference: &TypeReference,
) -> Vec<TypeConstraint> {
    if let Some(constraints) = index_of_constraints(program, machine, state, type_reference) {
        return constraints;
    }

    collect_constraints(program, type_reference)
}

fn index_of_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    type_reference: &TypeReference,
) -> Option<Vec<TypeConstraint>> {
    let TypeReference::Generic { base_name, .. } = type_reference
    else {
        return None;
    };

    if base_name != "IndexOf" {
        return None;
    }

    let [collection] = program.type_reference_arguments(type_reference) else {
        return None;
    };

    let collection_name = match collection {
        TypeReference::Named { name, .. } => name,
        _ => return None,
    };

    let length = collection_length_for_binding(program, machine, state, collection_name)?;
    if length == 0 {
        return None;
    }

    let mut constraints = vec![TypeConstraint::Range {
        minimum: Expression::Integer(0),
        maximum: Expression::Integer((length - 1) as i64),
    }];
    augment_constraints_with_named_facts(&mut constraints);
    Some(constraints)
}

fn collection_length_for_binding(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    name: &ProgramName,
) -> Option<usize> {
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name == *name)
        .and_then(|parameter| {
            collection_length_from_type_reference(
                program,
                machine,
                state,
                &parameter.type_reference,
            )
        })
        .or_else(|| {
            program.state_statements(state).iter().find_map(|statement| {
                let omega_typed_trees::statement::Statement::LocalData(local_data) = statement
                else {
                    return None;
                };

                if local_data.name != *name {
                    return None;
                }

                local_data
                    .initial_value
                    .as_ref()
                    .and_then(|value| {
                        collection_length_from_expression(program, machine, state, value)
                    })
                    .or_else(|| {
                        collection_length_from_type_reference(
                            program,
                            machine,
                            state,
                            &local_data.type_reference,
                        )
                    })
            })
        })
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find(|owned_data| owned_data.name == *name)
                .and_then(|owned_data| {
                    collection_length_from_type_reference(
                        program,
                        machine,
                        state,
                        &owned_data.type_reference,
                    )
                })
        })
}

fn collection_length_from_expression(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: &Expression,
) -> Option<usize> {
    match expression {
        Expression::Mutable(inner) => {
            collection_length_from_expression(program, machine, state, inner)
        }
        Expression::Call(call) if matches!(call.target.as_str(), "as_slice" | "as_mut_slice") => {
            let receiver = call.receiver.as_deref()?;
            collection_length_from_expression(program, machine, state, receiver).or_else(|| {
                expression_type_reference(program, machine, state, receiver).and_then(
                    |type_reference| {
                        collection_length_from_type_reference(
                            program,
                            machine,
                            state,
                            type_reference,
                        )
                    },
                )
            })
        }
        _ => expression_type_reference(program, machine, state, expression).and_then(
            |type_reference| {
                collection_length_from_type_reference(program, machine, state, type_reference)
            },
        ),
    }
}

fn collection_length_from_type_reference(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    type_reference: &TypeReference,
) -> Option<usize> {
    match type_reference {
        TypeReference::Reference { referee, .. } => {
            collection_length_from_type_reference(program, machine, state, referee)
        }
        TypeReference::Constrained { base_type, .. } => {
            collection_length_from_type_reference(program, machine, state, base_type)
        }
        TypeReference::FixedArray { length, .. } => Some(*length),
        TypeReference::Slice { .. } => None,
        TypeReference::Generic { .. } | TypeReference::Named { .. } | TypeReference::Unit => None,
    }
}

fn primitive_constraints(name: &ProgramName) -> Vec<TypeConstraint> {
    let mut constraints = match name.as_str() {
        "u32" => vec![TypeConstraint::Range {
            minimum: Expression::Integer(0),
            maximum: Expression::Integer(u32::MAX as i64),
        }],
        "usize" => vec![TypeConstraint::Range {
            minimum: Expression::Integer(0),
            maximum: Expression::Integer(i64::MAX),
        }],
        _ => Vec::new(),
    };
    augment_constraints_with_named_facts(&mut constraints);
    constraints
}

fn type_reference_for_symbol<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    state: &'program State,
    symbol: SymbolHandle,
) -> Option<&'program TypeReference> {
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .map(|parameter| &parameter.type_reference)
        .or_else(|| {
            program.state_statements(state).iter().find_map(|statement| {
                let omega_typed_trees::statement::Statement::LocalData(local_data) = statement
                else {
                    return None;
                };

                (local_data.symbol == symbol).then_some(&local_data.type_reference)
            })
        })
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find(|owned_data| owned_data.symbol == symbol)
                .map(|owned_data| &owned_data.type_reference)
        })
        .or_else(|| {
            program.data_definitions().iter().find_map(|data_definition| {
                program.data_members(data_definition).iter().find_map(|member| {
                    let omega_typed_trees::data::DataMember::Field(field) = member else {
                        return None;
                    };

                    (field.symbol == symbol).then_some(&field.type_reference)
                })
            })
        })
}

fn data_field_type_reference<'program>(
    program: &'program TypedTrees,
    type_reference: &'program TypeReference,
    member_symbol: SymbolHandle,
    member_name: &ProgramName,
) -> Option<&'program TypeReference> {
    match type_reference {
        TypeReference::Reference { referee, .. } => {
            data_field_type_reference(program, referee, member_symbol, member_name)
        }
        TypeReference::Constrained { base_type, .. } => {
            data_field_type_reference(program, base_type, member_symbol, member_name)
        }
        TypeReference::Generic {
            base_symbol,
            base_name,
            ..
        } => data_definition_by_symbol_or_name(program, *base_symbol, base_name).and_then(
            |data_definition| {
                data_field_in_definition(program, data_definition, member_symbol, member_name)
            },
        ),
        TypeReference::Named { symbol, name } => {
            data_definition_by_symbol_or_name(program, *symbol, name).and_then(|data_definition| {
                data_field_in_definition(program, data_definition, member_symbol, member_name)
            })
        }
        TypeReference::FixedArray { .. } | TypeReference::Slice { .. } | TypeReference::Unit => {
            None
        }
    }
}

fn data_definition_by_symbol_or_name<'program>(
    program: &'program TypedTrees,
    symbol: SymbolHandle,
    name: &ProgramName,
) -> Option<&'program omega_typed_trees::data::DataDefinition> {
    program.data_definitions().iter().find(|data_definition| {
        (symbol.is_valid() && data_definition.symbol == symbol) || data_definition.name == *name
    })
}

fn data_field_in_definition<'program>(
    program: &'program TypedTrees,
    data_definition: &'program omega_typed_trees::data::DataDefinition,
    member_symbol: SymbolHandle,
    member_name: &ProgramName,
) -> Option<&'program TypeReference> {
    program.data_members(data_definition).iter().find_map(|member| {
        let omega_typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };

        ((member_symbol.is_valid() && field.symbol == member_symbol) || field.name == *member_name)
            .then_some(&field.type_reference)
    })
}

fn integer_literal_constraints(value: i64) -> Vec<TypeConstraint> {
    let mut constraints = vec![
        TypeConstraint::Named(ProgramName::generated("exact")),
        TypeConstraint::Range {
            minimum: Expression::Integer(value),
            maximum: Expression::Integer(value),
        },
    ];

    if value >= 0 {
        constraints.push(TypeConstraint::Named(ProgramName::generated(
            "non_negative",
        )));
    }

    if value > 0 {
        constraints.push(TypeConstraint::Named(ProgramName::generated("positive")));
    }

    constraints
}

fn float_literal_constraints(value: FloatLiteral) -> Vec<TypeConstraint> {
    let value = value.value();
    if !value.is_finite() {
        return Vec::new();
    }

    vec![
        TypeConstraint::Named(ProgramName::generated("finite")),
        TypeConstraint::Range {
            minimum: Expression::Float(FloatLiteral::new(value)),
            maximum: Expression::Float(FloatLiteral::new(value)),
        },
    ]
}

fn derived_binary_constraints(
    operator: BinaryOperator,
    left_constraints: &[TypeConstraint],
    right_constraints: &[TypeConstraint],
) -> Vec<TypeConstraint> {
    let mut constraints = Vec::new();

    if integer_constraints_are_exact(left_constraints)
        && integer_constraints_are_exact(right_constraints)
        && matches!(
            operator,
            BinaryOperator::Add
                | BinaryOperator::Modulo
                | BinaryOperator::Multiply
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::Subtract
        )
    {
        constraints.push(TypeConstraint::Named(ProgramName::generated("exact")));
    }

    if integer_constraints_are_wrapping(left_constraints)
        && matches!(
            operator,
            BinaryOperator::Add
                | BinaryOperator::Modulo
                | BinaryOperator::Multiply
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::Subtract
        )
    {
        constraints.push(TypeConstraint::Named(ProgramName::generated("wrapping")));
    }

    if let (Some(left_range), Some(right_range)) = (
        integer_range_from_constraints(left_constraints),
        integer_range_from_constraints(right_constraints),
    ) && let Some(range) = integer_binary_range(operator, left_range, right_range)
    {
        constraints.push(TypeConstraint::Range {
            minimum: Expression::Integer(range.minimum),
            maximum: Expression::Integer(range.maximum),
        });
        if range.minimum >= 0 {
            constraints.push(TypeConstraint::Named(ProgramName::generated(
                "non_negative",
            )));
        }
        if range.minimum > 0 {
            constraints.push(TypeConstraint::Named(ProgramName::generated("positive")));
        }
    }

    if let (Some(left_range), Some(right_range)) = (
        float_range_from_constraints(left_constraints),
        float_range_from_constraints(right_constraints),
    ) && let Some(range) = float_binary_range(operator, left_range, right_range)
    {
        constraints.push(TypeConstraint::Named(ProgramName::generated("finite")));
        constraints.push(TypeConstraint::Range {
            minimum: Expression::Float(FloatLiteral::new(range.minimum)),
            maximum: Expression::Float(FloatLiteral::new(range.maximum)),
        });
    }

    constraints
}

fn derived_real_from_constraints(argument_constraints: &[TypeConstraint]) -> Vec<TypeConstraint> {
    let Some(range) = integer_range_from_constraints(argument_constraints) else {
        return Vec::new();
    };

    vec![
        TypeConstraint::Named(ProgramName::generated("finite")),
        TypeConstraint::Range {
            minimum: Expression::Float(FloatLiteral::new(range.minimum as f64)),
            maximum: Expression::Float(FloatLiteral::new(range.maximum as f64)),
        },
    ]
}

fn derived_builtin_call_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &omega_typed_trees::expression::CallExpression,
) -> Option<Vec<TypeConstraint>> {
    match call.target.as_str() {
        "max" => derived_extrema_call_constraints(program, machine, state, call, true),
        "min" => derived_extrema_call_constraints(program, machine, state, call, false),
        "range" => derived_range_call_constraints(program, machine, state, call),
        _ => None,
    }
}

fn derived_extrema_call_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &omega_typed_trees::expression::CallExpression,
    is_max: bool,
) -> Option<Vec<TypeConstraint>> {
    let [left, right] = call.arguments.as_slice() else {
        return None;
    };

    let left_constraints = expression_constraints(program, machine, state, left);
    let right_constraints = expression_constraints(program, machine, state, right);
    let mut constraints = Vec::new();

    if integer_constraints_are_exact(&left_constraints)
        && integer_constraints_are_exact(&right_constraints)
    {
        constraints.push(TypeConstraint::Named(ProgramName::generated("exact")));
    }

    if let (Some(left_range), Some(right_range)) = (
        integer_range_from_constraints(&left_constraints),
        integer_range_from_constraints(&right_constraints),
    ) {
        let range = if is_max {
            IntegerRange {
                minimum: left_range.minimum.max(right_range.minimum),
                maximum: left_range.maximum.max(right_range.maximum),
            }
        } else {
            IntegerRange {
                minimum: left_range.minimum.min(right_range.minimum),
                maximum: left_range.maximum.min(right_range.maximum),
            }
        };

        constraints.push(TypeConstraint::Range {
            minimum: Expression::Integer(range.minimum),
            maximum: Expression::Integer(range.maximum),
        });
    }

    if constraints.is_empty() {
        return None;
    }

    augment_constraints_with_named_facts(&mut constraints);
    Some(constraints)
}

fn derived_range_call_constraints(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &omega_typed_trees::expression::CallExpression,
) -> Option<Vec<TypeConstraint>> {
    let [_, exclusive_max] = call.arguments.as_slice() else {
        return None;
    };

    let upper_constraints = expression_constraints(program, machine, state, exclusive_max);
    let mut constraints = vec![TypeConstraint::Named(ProgramName::generated("exact"))];

    if let Some(upper_range) = integer_range_from_constraints(&upper_constraints) {
        constraints.push(TypeConstraint::Range {
            minimum: Expression::Integer(0),
            maximum: Expression::Integer(upper_range.maximum),
        });
    }

    augment_constraints_with_named_facts(&mut constraints);
    Some(constraints)
}

fn call_expression_return_type<'program>(
    program: &'program TypedTrees,
    _machine: &'program Machine,
    _state: &'program State,
    call: &'program omega_typed_trees::expression::CallExpression,
) -> Option<&'program TypeReference> {
    callable_return_type_by_symbol(program, call.target_symbol)
}

fn callable_return_type_by_symbol(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
) -> Option<&TypeReference> {
    if !target_symbol.is_valid() {
        return None;
    }

    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .find(|candidate| candidate.symbol == target_symbol)
        .and_then(|candidate| candidate.return_type.as_ref())
        .or_else(|| {
            program
                .platforms()
                .iter()
                .flat_map(|platform| program.platform_state_signatures(platform).iter())
                .find(|candidate| candidate.symbol == target_symbol)
                .and_then(|candidate| candidate.return_type.as_ref())
        })
}

fn is_real_from_call(receiver: Option<&Expression>, target: &ProgramName) -> bool {
    target == "from"
        && matches!(
            receiver,
            Some(Expression::Name(path)) if path.members() == ["Real"]
        )
}

fn augment_constraints_with_named_facts(constraints: &mut Vec<TypeConstraint>) {
    if constraints.iter().any(|constraint| {
        matches!(constraint, TypeConstraint::Range { minimum, maximum } if minimum == maximum)
    }) && !has_named_constraint(constraints, "exact")
    {
        constraints.push(TypeConstraint::Named(ProgramName::generated("exact")));
    }

    if let Some(range) = integer_range_from_constraints(constraints) {
        if range.minimum >= 0 && !has_named_constraint(constraints, "non_negative") {
            constraints.push(TypeConstraint::Named(ProgramName::generated(
                "non_negative",
            )));
        }
        if range.minimum > 0 && !has_named_constraint(constraints, "positive") {
            constraints.push(TypeConstraint::Named(ProgramName::generated("positive")));
        }
    }

    if float_range_from_constraints(constraints).is_some()
        && !has_named_constraint(constraints, "finite")
    {
        constraints.push(TypeConstraint::Named(ProgramName::generated("finite")));
    }
}

fn integer_constraints_are_exact(constraints: &[TypeConstraint]) -> bool {
    has_named_constraint(constraints, "exact")
        || integer_range_from_constraints(constraints).is_some()
}

fn integer_constraints_are_wrapping(constraints: &[TypeConstraint]) -> bool {
    has_named_constraint(constraints, "wrapping")
}

fn has_named_constraint(constraints: &[TypeConstraint], name: &str) -> bool {
    constraints.iter().any(|constraint| {
        matches!(constraint, TypeConstraint::Named(constraint_name) if constraint_name == name)
    })
}

fn integer_range_from_constraints(constraints: &[TypeConstraint]) -> Option<IntegerRange> {
    let mut range: Option<IntegerRange> = None;

    for constraint in constraints {
        let TypeConstraint::Range { minimum, maximum } = constraint else {
            continue;
        };

        let Some(candidate) = (|| {
            Some(IntegerRange {
                minimum: integer_constant_value(minimum)?,
                maximum: integer_constant_value(maximum)?,
            })
        })() else {
            continue;
        };

        range = Some(match range {
            Some(existing) => IntegerRange {
                minimum: existing.minimum.max(candidate.minimum),
                maximum: existing.maximum.min(candidate.maximum),
            },
            None => candidate,
        });
    }

    for constraint in constraints {
        let TypeConstraint::Named(name) = constraint else {
            continue;
        };

        let implied = match name.as_str() {
            "non_negative" => Some(IntegerRange {
                minimum: 0,
                maximum: i64::MAX,
            }),
            "positive" => Some(IntegerRange {
                minimum: 1,
                maximum: i64::MAX,
            }),
            _ => None,
        };

        let Some(implied) = implied else {
            continue;
        };

        range = Some(match range {
            Some(existing) => IntegerRange {
                minimum: existing.minimum.max(implied.minimum),
                maximum: existing.maximum.min(implied.maximum),
            },
            None => implied,
        });
    }

    range
}

fn float_range_from_constraints(constraints: &[TypeConstraint]) -> Option<FloatRange> {
    let mut range: Option<FloatRange> = None;

    for constraint in constraints {
        let TypeConstraint::Range { minimum, maximum } = constraint else {
            continue;
        };

        let Some(candidate) = (|| {
            Some(FloatRange {
                minimum: float_constant_value(minimum)?,
                maximum: float_constant_value(maximum)?,
            })
        })() else {
            continue;
        };

        range = Some(match range {
            Some(existing) => FloatRange {
                minimum: existing.minimum.max(candidate.minimum),
                maximum: existing.maximum.min(candidate.maximum),
            },
            None => candidate,
        });
    }

    range
}

fn integer_binary_range(
    operator: BinaryOperator,
    left: IntegerRange,
    right: IntegerRange,
) -> Option<IntegerRange> {
    match operator {
        BinaryOperator::Add => Some(IntegerRange {
            minimum: left.minimum.saturating_add(right.minimum),
            maximum: left.maximum.saturating_add(right.maximum),
        }),
        BinaryOperator::Subtract => Some(IntegerRange {
            minimum: left.minimum.saturating_sub(right.maximum),
            maximum: left.maximum.saturating_sub(right.minimum),
        }),
        BinaryOperator::Multiply => {
            let products = [
                left.minimum.saturating_mul(right.minimum),
                left.minimum.saturating_mul(right.maximum),
                left.maximum.saturating_mul(right.minimum),
                left.maximum.saturating_mul(right.maximum),
            ];
            Some(IntegerRange {
                minimum: *products.iter().min()?,
                maximum: *products.iter().max()?,
            })
        }
        BinaryOperator::Modulo => {
            if right.minimum <= 0 {
                return None;
            }

            Some(IntegerRange {
                minimum: 0,
                maximum: right.maximum.saturating_sub(1),
            })
        }
        BinaryOperator::ShiftRight => {
            if right.minimum < 0 {
                return None;
            }

            Some(IntegerRange {
                minimum: 0.max(left.minimum),
                maximum: left.maximum.max(0),
            })
        }
        BinaryOperator::And
        | BinaryOperator::Divide
        | BinaryOperator::Equal
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft => None,
    }
}

fn float_binary_range(
    operator: BinaryOperator,
    left: FloatRange,
    right: FloatRange,
) -> Option<FloatRange> {
    match operator {
        BinaryOperator::Add => Some(FloatRange {
            minimum: left.minimum + right.minimum,
            maximum: left.maximum + right.maximum,
        }),
        BinaryOperator::Subtract => Some(FloatRange {
            minimum: left.minimum - right.maximum,
            maximum: left.maximum - right.minimum,
        }),
        BinaryOperator::Multiply => {
            let products = [
                left.minimum * right.minimum,
                left.minimum * right.maximum,
                left.maximum * right.minimum,
                left.maximum * right.maximum,
            ];
            Some(FloatRange {
                minimum: products.iter().copied().fold(f64::INFINITY, f64::min),
                maximum: products.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            })
        }
        BinaryOperator::Divide => {
            if right.minimum <= 0.0 && right.maximum >= 0.0 {
                return None;
            }

            let quotients = [
                left.minimum / right.minimum,
                left.minimum / right.maximum,
                left.maximum / right.minimum,
                left.maximum / right.maximum,
            ];
            Some(FloatRange {
                minimum: quotients.iter().copied().fold(f64::INFINITY, f64::min),
                maximum: quotients.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            })
        }
        BinaryOperator::And
        | BinaryOperator::Equal
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Modulo
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => None,
    }
}

fn integer_constant_value(expression: &Expression) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        Expression::Name(path) if path.members() == ["u32", "MAX"] => Some(u32::MAX as i64),
        _ => None,
    }
}

fn float_constant_value(expression: &Expression) -> Option<f64> {
    match expression {
        Expression::Float(value) => Some(value.value()),
        Expression::Integer(value) => Some(*value as f64),
        Expression::Name(path) if path.members() == ["u32", "MAX"] => Some(u32::MAX as f64),
        _ => None,
    }
}

fn type_constraints(
    program: &TypedTrees,
    constraints: omega_core::arena::HandleSpan<TypeConstraint>,
) -> &[TypeConstraint] {
    program.type_constraints.span(constraints).unwrap_or(&[])
}
