use omega_checked_trees::{
    CheckFacts, InvariantFact, InvariantFacts, Program, ProofFactKind, ProofFacts,
    ProofObligationFact,
};

pub fn lower_typed_trees(program: &omega_typed_trees::Program) -> Result<Program, Vec<omega_core::diagnostics::Diagnostic>> {
    omega_validation::validate_program(program)?;

    let proof_plan = omega_proof::obligations::build_proof_plan(program);
    omega_proof::checker::check_proof_plan(&proof_plan)?;

    Ok(Program {
        typed: program.clone(),
        facts: CheckFacts {
            borrow: Default::default(),
            proof: build_proof_facts(&proof_plan),
            invariants: build_invariant_facts(program),
        },
    })
}

pub fn lower_typed_program(
    program: &omega_typed_trees::Program,
) -> Result<Program, Vec<omega_core::diagnostics::Diagnostic>> {
    lower_typed_trees(program)
}

fn build_proof_facts(proof_plan: &omega_proof::obligations::ProofPlan) -> ProofFacts {
    let obligations = proof_plan
        .obligations
        .iter()
        .map(|obligation| match obligation {
            omega_proof::obligations::ProofObligation::BoundedAssignment(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedAssignment,
                    owner: format!("machine `{}` state `{}`", obligation.machine, obligation.state),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedCallArgument(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedCallArgument,
                    owner: format!(
                        "machine `{}` state `{}` call `{}` parameter `{}`",
                        obligation.machine, obligation.state, obligation.target, obligation.parameter
                    ),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedInitializer(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedInitializer,
                    owner: obligation.owner.clone(),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedStateReturn(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedStateReturn,
                    owner: format!("machine `{}` state `{}` return", obligation.machine, obligation.state),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedValue(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedValue,
                    owner: obligation.owner.clone(),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedTransitionArgument(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedTransitionArgument,
                    owner: format!(
                        "machine `{}` state `{}` transition parameter `{}`",
                        obligation.machine, obligation.state, obligation.parameter
                    ),
                }
            }
            omega_proof::obligations::ProofObligation::GuardedTransition(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::GuardedTransition,
                    owner: format!("machine `{}` state `{}` guard", obligation.machine, obligation.state),
                }
            }
        })
        .collect::<Vec<_>>();

    let mut stored = omega_core::arena::Arena::new();
    stored.insert_many(obligations);

    ProofFacts {
        obligations: stored,
    }
}

fn build_invariant_facts(program: &omega_typed_trees::Program) -> InvariantFacts {
    let definitions = program
        .invariant_definitions
        .iter()
        .map(|definition| InvariantFact {
            symbol: definition.symbol,
            name: definition.name.clone(),
            constraint_count: program.type_constraints.span_or_empty(definition.constraints).len(),
        })
        .collect::<Vec<_>>();

    let mut stored = omega_core::arena::Arena::new();
    stored.insert_many(definitions);

    InvariantFacts {
        definitions: stored,
    }
}
