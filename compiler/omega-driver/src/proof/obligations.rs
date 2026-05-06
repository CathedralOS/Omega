use crate::ir::Program;
use crate::ir::statement::{TransitionGuard, TransitionTarget};
use crate::ir::types::{TypeConstraint, TypeReference};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofPlan {
    pub obligations: Vec<ProofObligation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofObligation {
    BoundedValue(BoundedValueObligation),
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
