use crate::ir::Program;
use crate::ir::expression::Expression;
use crate::ir::machine::Machine;
use crate::ir::signature::StateParameter;
use crate::ir::state::State;
use crate::ir::statement::{Assignment, Call, Transition, TransitionGuard, TransitionTarget};
use crate::ir::types::{TypeConstraint, TypeReference};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofPlan {
    pub obligations: Vec<ProofObligation>,
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
    pub constraints: Vec<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardedTransitionObligation {
    pub machine: String,
    pub state: String,
    pub target: TransitionTarget,
    pub guard: TransitionGuard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedAssignmentObligation {
    pub machine: String,
    pub state: String,
    pub target: Expression,
    pub value: Expression,
    pub value_constraints: Vec<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: Vec<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedCallArgumentObligation {
    pub machine: String,
    pub state: String,
    pub receiver: Option<String>,
    pub target: String,
    pub parameter: String,
    pub argument: Expression,
    pub argument_constraints: Vec<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: Vec<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedInitializerObligation {
    pub owner: String,
    pub value: Expression,
    pub base_type: TypeReference,
    pub constraints: Vec<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedStateReturnObligation {
    pub machine: String,
    pub state: String,
    pub value: Expression,
    pub value_constraints: Vec<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: Vec<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedTransitionArgumentObligation {
    pub machine: String,
    pub state: String,
    pub target: TransitionTarget,
    pub parameter: String,
    pub argument: Expression,
    pub argument_constraints: Vec<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: Vec<TypeConstraint>,
    pub guard: TransitionGuard,
}

pub fn build_proof_plan(program: &Program) -> ProofPlan {
    let mut obligations = Vec::new();

    for machine in &program.machines {
        for owned_data in &machine.owned_data {
            collect_bounded_value_obligation(
                format!(
                    "machine `{}` owned data `{}`",
                    machine.name, owned_data.name
                ),
                &owned_data.type_reference,
                &mut obligations,
            );
            if let Some(initial_value) = &owned_data.initial_value {
                collect_bounded_initializer_obligation(
                    format!(
                        "machine `{}` owned data `{}`",
                        machine.name, owned_data.name
                    ),
                    &owned_data.type_reference,
                    initial_value,
                    &mut obligations,
                );
            }
        }

        for state in &machine.states {
            for parameter in &state.parameters {
                collect_bounded_value_obligation(
                    format!(
                        "machine `{}` state `{}` parameter `{}`",
                        machine.name, state.name, parameter.name
                    ),
                    &parameter.type_reference,
                    &mut obligations,
                );
            }

            if let Some(return_type) = &state.return_type {
                collect_bounded_value_obligation(
                    format!(
                        "machine `{}` state `{}` return value",
                        machine.name, state.name
                    ),
                    return_type,
                    &mut obligations,
                );
                collect_bounded_state_return_obligation(
                    machine,
                    state,
                    return_type,
                    &mut obligations,
                );
            }

            for statement in &state.statements {
                let transition = match statement {
                    crate::ir::statement::Statement::Assignment(assignment) => {
                        collect_bounded_assignment_obligation(
                            machine,
                            state,
                            assignment,
                            &mut obligations,
                        );
                        continue;
                    }
                    crate::ir::statement::Statement::Call(call) => {
                        collect_bounded_call_argument_obligations(
                            program,
                            machine,
                            state,
                            call,
                            &mut obligations,
                        );
                        continue;
                    }
                    crate::ir::statement::Statement::Transition(transition) => transition,
                    _ => continue,
                };

                if let TransitionGuard::When(_) = &transition.guard {
                    obligations.push(ProofObligation::GuardedTransition(
                        GuardedTransitionObligation {
                            machine: machine.name.clone(),
                            state: state.name.clone(),
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
                    &mut obligations,
                );
            }
        }
    }

    ProofPlan { obligations }
}

fn collect_bounded_value_obligation(
    owner: String,
    type_reference: &TypeReference,
    obligations: &mut Vec<ProofObligation>,
) {
    match type_reference {
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            obligations.push(ProofObligation::BoundedValue(BoundedValueObligation {
                owner,
                base_type: base_type.as_ref().clone(),
                constraints: constraints.clone(),
            }));
        }
        TypeReference::FixedArray { element_type, .. } => {
            collect_bounded_value_obligation(owner, element_type, obligations);
        }
        TypeReference::Named(_) => {}
    }
}

fn collect_bounded_initializer_obligation(
    owner: String,
    type_reference: &TypeReference,
    value: &Expression,
    obligations: &mut Vec<ProofObligation>,
) {
    match type_reference {
        TypeReference::Constrained {
            base_type,
            constraints,
        } => {
            obligations.push(ProofObligation::BoundedInitializer(
                BoundedInitializerObligation {
                    owner,
                    value: value.clone(),
                    base_type: base_type.as_ref().clone(),
                    constraints: constraints.clone(),
                },
            ));
        }
        TypeReference::FixedArray { element_type, .. } => {
            collect_bounded_initializer_obligation(owner, element_type, value, obligations);
        }
        TypeReference::Named(_) => {}
    }
}

fn collect_bounded_assignment_obligation(
    machine: &Machine,
    state: &State,
    assignment: &Assignment,
    obligations: &mut Vec<ProofObligation>,
) {
    let Some(TypeReference::Constrained {
        base_type,
        constraints,
    }) = expression_type_reference(machine, state, &assignment.target)
    else {
        return;
    };

    obligations.push(ProofObligation::BoundedAssignment(
        BoundedAssignmentObligation {
            machine: machine.name.clone(),
            state: state.name.clone(),
            target: assignment.target.clone(),
            value: assignment.value.clone(),
            value_constraints: expression_constraints(machine, state, &assignment.value),
            base_type: base_type.as_ref().clone(),
            constraints: constraints.clone(),
        },
    ));
}

fn collect_bounded_transition_argument_obligations(
    program: &Program,
    machine: &Machine,
    state: &State,
    transition: &Transition,
    obligations: &mut Vec<ProofObligation>,
) {
    let Some((target_state, arguments)) =
        transition_target_state_and_arguments(program, machine, state, &transition.target)
    else {
        return;
    };

    for (parameter, argument) in callable_parameters(target_state).zip(arguments.iter()) {
        let TypeReference::Constrained {
            base_type,
            constraints,
        } = &parameter.type_reference
        else {
            continue;
        };

        obligations.push(ProofObligation::BoundedTransitionArgument(
            BoundedTransitionArgumentObligation {
                machine: machine.name.clone(),
                state: state.name.clone(),
                target: transition.target.clone(),
                parameter: parameter.name.clone(),
                argument: argument.clone(),
                argument_constraints: expression_constraints(machine, state, argument),
                base_type: base_type.as_ref().clone(),
                constraints: constraints.clone(),
                guard: transition.guard.clone(),
            },
        ));
    }
}

fn collect_bounded_call_argument_obligations(
    program: &Program,
    machine: &Machine,
    state: &State,
    call: &Call,
    obligations: &mut Vec<ProofObligation>,
) {
    let Some(parameters) = call_target_parameters(program, machine, call) else {
        return;
    };

    for (parameter, argument) in parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(call.arguments.iter())
    {
        let TypeReference::Constrained {
            base_type,
            constraints,
        } = &parameter.type_reference
        else {
            continue;
        };

        obligations.push(ProofObligation::BoundedCallArgument(
            BoundedCallArgumentObligation {
                machine: machine.name.clone(),
                state: state.name.clone(),
                receiver: call.receiver.clone(),
                target: call.target.clone(),
                parameter: parameter.name.clone(),
                argument: argument.clone(),
                argument_constraints: expression_constraints(machine, state, argument),
                base_type: base_type.as_ref().clone(),
                constraints: constraints.clone(),
            },
        ));
    }
}

fn collect_bounded_state_return_obligation(
    machine: &Machine,
    state: &State,
    return_type: &TypeReference,
    obligations: &mut Vec<ProofObligation>,
) {
    let TypeReference::Constrained {
        base_type,
        constraints,
    } = return_type
    else {
        return;
    };
    let Some(crate::ir::statement::Statement::Expression(value)) = state.statements.last() else {
        return;
    };

    obligations.push(ProofObligation::BoundedStateReturn(
        BoundedStateReturnObligation {
            machine: machine.name.clone(),
            state: state.name.clone(),
            value: value.clone(),
            value_constraints: expression_constraints(machine, state, value),
            base_type: base_type.as_ref().clone(),
            constraints: constraints.clone(),
        },
    ));
}

fn call_target_parameters<'program>(
    program: &'program Program,
    machine: &'program Machine,
    call: &Call,
) -> Option<&'program [StateParameter]> {
    let Some(receiver) = call.receiver.as_deref() else {
        return machine
            .states
            .iter()
            .find(|state| state.name == call.target)
            .map(|state| state.parameters.as_slice());
    };

    if receiver == "self" {
        return machine
            .states
            .iter()
            .find(|state| state.name == call.target)
            .map(|state| state.parameters.as_slice());
    }

    let receiver_type = machine
        .contains
        .iter()
        .find(|contained| contained.name == receiver)
        .map(|contained| contained.type_name.as_str());

    if let Some(parameters) = receiver_type
        .and_then(|type_name| platform_state_parameters(program, type_name, &call.target))
    {
        return Some(parameters);
    }

    receiver_type
        .and_then(|type_name| machine_state_parameters(program, type_name, &call.target))
        .or_else(|| machine_state_parameters(program, receiver, &call.target))
}

fn platform_state_parameters<'program>(
    program: &'program Program,
    platform_name: &str,
    state_name: &str,
) -> Option<&'program [StateParameter]> {
    program
        .platforms
        .iter()
        .find(|platform| platform.name == platform_name)?
        .states
        .iter()
        .find(|state| state.name == state_name)
        .map(|state| state.parameters.as_slice())
}

fn machine_state_parameters<'program>(
    program: &'program Program,
    machine_name: &str,
    state_name: &str,
) -> Option<&'program [StateParameter]> {
    program
        .machines
        .iter()
        .find(|machine| machine.name == machine_name)?
        .states
        .iter()
        .find(|state| state.name == state_name)
        .map(|state| state.parameters.as_slice())
}

fn transition_target_state_and_arguments<'program>(
    program: &'program Program,
    machine: &'program Machine,
    state: &'program State,
    target: &'program TransitionTarget,
) -> Option<(&'program State, &'program [Expression])> {
    let TransitionTarget::Named { path, arguments } = target else {
        return None;
    };

    let target_state = match path.as_slice() {
        [state_name] => machine
            .states
            .iter()
            .find(|candidate| candidate.name == *state_name),
        [receiver, state_name] if receiver == "self" => machine
            .states
            .iter()
            .find(|candidate| candidate.name == *state_name),
        [receiver, state_name] => {
            contained_machine(program, machine, receiver).and_then(|target_machine| {
                target_machine
                    .states
                    .iter()
                    .find(|candidate| candidate.name == *state_name)
            })
        }
        _ => None,
    };

    target_state
        .or_else(|| {
            if path.as_slice() == ["self"] {
                Some(state)
            } else {
                None
            }
        })
        .map(|target_state| (target_state, arguments.as_slice()))
}

fn contained_machine<'program>(
    program: &'program Program,
    machine: &Machine,
    receiver: &str,
) -> Option<&'program Machine> {
    let contained = machine
        .contains
        .iter()
        .find(|contained| contained.name == receiver)?;

    program
        .machines
        .iter()
        .find(|machine| machine.name == contained.type_name)
}

fn callable_parameters(state: &State) -> impl Iterator<Item = &StateParameter> {
    state
        .parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
}

fn expression_constraints(
    machine: &Machine,
    state: &State,
    expression: &Expression,
) -> Vec<TypeConstraint> {
    expression_type_reference(machine, state, expression)
        .map(collect_constraints)
        .unwrap_or_default()
}

fn expression_type_reference<'program>(
    machine: &'program Machine,
    state: &'program State,
    expression: &Expression,
) -> Option<&'program TypeReference> {
    let Expression::Name(path) = expression else {
        return None;
    };
    let name = match path.as_slice() {
        [name] => name,
        [receiver, name] if receiver == "self" => name,
        _ => return None,
    };

    state
        .parameters
        .iter()
        .find(|parameter| parameter.name == *name)
        .map(|parameter| &parameter.type_reference)
        .or_else(|| {
            state.statements.iter().find_map(|statement| {
                let crate::ir::statement::Statement::LocalData(local_data) = statement else {
                    return None;
                };

                (local_data.name == *name).then_some(&local_data.type_reference)
            })
        })
        .or_else(|| {
            machine
                .owned_data
                .iter()
                .find(|owned_data| owned_data.name == *name)
                .map(|owned_data| &owned_data.type_reference)
        })
}

fn collect_constraints(type_reference: &TypeReference) -> Vec<TypeConstraint> {
    match type_reference {
        TypeReference::Constrained { constraints, .. } => constraints.clone(),
        TypeReference::FixedArray { element_type, .. } => collect_constraints(element_type),
        TypeReference::Named(_) => Vec::new(),
    }
}
