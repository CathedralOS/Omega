use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::Program;
use omega_typed_program::expression::Expression;
use omega_typed_program::machine::Machine;
use omega_typed_program::signature::StateParameter;
use omega_typed_program::state::State;
use omega_typed_program::statement::{
    Assignment, Call, Transition, TransitionGuard, TransitionTarget,
};
use omega_typed_program::types::{TypeConstraint, TypeReference};

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
    pub value_constraints: HandleSpan<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedCallArgumentObligation {
    pub machine: String,
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
    pub machine: String,
    pub state: String,
    pub value: Expression,
    pub value_constraints: HandleSpan<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedTransitionArgumentObligation {
    pub machine: String,
    pub state: String,
    pub target: TransitionTarget,
    pub parameter: String,
    pub argument: Expression,
    pub argument_constraints: HandleSpan<TypeConstraint>,
    pub base_type: TypeReference,
    pub constraints: HandleSpan<TypeConstraint>,
    pub guard: TransitionGuard,
}

pub fn build_proof_plan(program: &Program) -> ProofPlan {
    let mut proof_plan = ProofPlan::default();

    for machine in &program.machines {
        for owned_data in &machine.owned_data {
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

        for state in &machine.states {
            for parameter in &state.parameters {
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

            for statement in &state.statements {
                let transition = match statement {
                    omega_typed_program::statement::Statement::Assignment(assignment) => {
                        collect_bounded_assignment_obligation(
                            program,
                            machine,
                            state,
                            assignment,
                            &mut proof_plan,
                        );
                        continue;
                    }
                    omega_typed_program::statement::Statement::Call(call) => {
                        collect_bounded_call_argument_obligations(
                            program,
                            machine,
                            state,
                            call,
                            &mut proof_plan,
                        );
                        continue;
                    }
                    omega_typed_program::statement::Statement::Transition(transition) => transition,
                    _ => continue,
                };

                if let TransitionGuard::When(_) = &transition.guard {
                    proof_plan
                        .obligations
                        .push(ProofObligation::GuardedTransition(
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
                    &mut proof_plan,
                );
            }
        }
    }

    proof_plan
}

fn collect_bounded_value_obligation(
    program: &Program,
    owner: String,
    type_reference: &TypeReference,
    proof_plan: &mut ProofPlan,
) {
    match type_reference {
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
        TypeReference::Generic { arguments, .. } => {
            for argument in arguments {
                collect_bounded_value_obligation(program, owner.clone(), argument, proof_plan);
            }
        }
        TypeReference::Named(_) => {}
        TypeReference::Unit => {}
    }
}

fn collect_bounded_initializer_obligation(
    program: &Program,
    owner: String,
    type_reference: &TypeReference,
    value: &Expression,
    proof_plan: &mut ProofPlan,
) {
    match type_reference {
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
        TypeReference::Generic { arguments, .. } => {
            for argument in arguments {
                collect_bounded_initializer_obligation(
                    program,
                    owner.clone(),
                    argument,
                    value,
                    proof_plan,
                );
            }
        }
        TypeReference::Named(_) => {}
        TypeReference::Unit => {}
    }
}

fn collect_bounded_assignment_obligation(
    program: &Program,
    machine: &Machine,
    state: &State,
    assignment: &Assignment,
    proof_plan: &mut ProofPlan,
) {
    let Some(TypeReference::Constrained {
        base_type,
        constraints,
    }) = expression_type_reference(machine, state, &assignment.target)
    else {
        return;
    };

    let value_constraints = expression_constraints(program, machine, state, &assignment.value);
    let value_constraints = proof_plan.store_constraints(&value_constraints);
    let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));

    proof_plan
        .obligations
        .push(ProofObligation::BoundedAssignment(
            BoundedAssignmentObligation {
                machine: machine.name.clone(),
                state: state.name.clone(),
                target: assignment.target.clone(),
                value: assignment.value.clone(),
                value_constraints,
                base_type: base_type.as_ref().clone(),
                constraints,
            },
        ));
}

fn collect_bounded_transition_argument_obligations(
    program: &Program,
    machine: &Machine,
    state: &State,
    transition: &Transition,
    proof_plan: &mut ProofPlan,
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

        let argument_constraints = expression_constraints(program, machine, state, argument);
        let argument_constraints = proof_plan.store_constraints(&argument_constraints);
        let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));

        proof_plan
            .obligations
            .push(ProofObligation::BoundedTransitionArgument(
                BoundedTransitionArgumentObligation {
                    machine: machine.name.clone(),
                    state: state.name.clone(),
                    target: transition.target.clone(),
                    parameter: parameter.name.clone(),
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
    program: &Program,
    machine: &Machine,
    state: &State,
    call: &Call,
    proof_plan: &mut ProofPlan,
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

        let argument_constraints = expression_constraints(program, machine, state, argument);
        let argument_constraints = proof_plan.store_constraints(&argument_constraints);
        let constraints = proof_plan.store_constraints(type_constraints(program, *constraints));

        proof_plan
            .obligations
            .push(ProofObligation::BoundedCallArgument(
                BoundedCallArgumentObligation {
                    machine: machine.name.clone(),
                    state: state.name.clone(),
                    receiver: call.receiver.clone(),
                    target: call.target.clone(),
                    parameter: parameter.name.clone(),
                    argument: argument.clone(),
                    argument_constraints,
                    base_type: base_type.as_ref().clone(),
                    constraints,
                },
            ));
    }
}

fn collect_bounded_state_return_obligation(
    program: &Program,
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
    let Some(omega_typed_program::statement::Statement::Expression(value)) =
        state.statements.last()
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
                machine: machine.name.clone(),
                state: state.name.clone(),
                value: value.clone(),
                value_constraints,
                base_type: base_type.as_ref().clone(),
                constraints,
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
    program: &Program,
    machine: &Machine,
    state: &State,
    expression: &Expression,
) -> Vec<TypeConstraint> {
    expression_type_reference(machine, state, expression)
        .map(|type_reference| collect_constraints(program, type_reference))
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
                let omega_typed_program::statement::Statement::LocalData(local_data) = statement
                else {
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

fn collect_constraints(program: &Program, type_reference: &TypeReference) -> Vec<TypeConstraint> {
    match type_reference {
        TypeReference::Constrained { constraints, .. } => {
            type_constraints(program, *constraints).to_vec()
        }
        TypeReference::FixedArray { element_type, .. } => {
            collect_constraints(program, element_type)
        }
        TypeReference::Generic { arguments, .. } => arguments
            .iter()
            .flat_map(|argument| collect_constraints(program, argument))
            .collect(),
        TypeReference::Named(_) => Vec::new(),
        TypeReference::Unit => Vec::new(),
    }
}

fn type_constraints(
    program: &Program,
    constraints: omega_core::arena::HandleSpan<TypeConstraint>,
) -> &[TypeConstraint] {
    program.type_constraints.span(constraints).unwrap_or(&[])
}
