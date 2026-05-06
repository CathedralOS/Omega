use crate::ir::Program;
use crate::ir::expression::Expression;
use crate::ir::machine::Machine;
use crate::ir::signature::StateParameter;
use crate::ir::state::State;
use crate::ir::statement::{Transition, TransitionGuard, TransitionTarget};
use crate::ir::types::{TypeConstraint, TypeReference};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofPlan {
    pub obligations: Vec<ProofObligation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofObligation {
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
            }

            for statement in &state.statements {
                let crate::ir::statement::Statement::Transition(transition) = statement else {
                    continue;
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
    let Expression::Name(path) = expression else {
        return Vec::new();
    };
    let [name] = path.as_slice() else {
        return Vec::new();
    };

    state
        .parameters
        .iter()
        .find(|parameter| parameter.name == *name)
        .map(|parameter| collect_constraints(&parameter.type_reference))
        .or_else(|| {
            machine
                .owned_data
                .iter()
                .find(|owned_data| owned_data.name == *name)
                .map(|owned_data| collect_constraints(&owned_data.type_reference))
        })
        .unwrap_or_default()
}

fn collect_constraints(type_reference: &TypeReference) -> Vec<TypeConstraint> {
    match type_reference {
        TypeReference::Constrained { constraints, .. } => constraints.clone(),
        TypeReference::FixedArray { element_type, .. } => collect_constraints(element_type),
        TypeReference::Named(_) => Vec::new(),
    }
}
