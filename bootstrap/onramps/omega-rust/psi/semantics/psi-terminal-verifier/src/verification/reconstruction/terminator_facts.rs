//! Ordered successor and return-fact reconstruction for one terminator.

use std::collections::BTreeMap;

use psi_core::{BlockId, MachineId, Proposition, ScalarTerm, ValueId};
use psi_proof_kernel::{Obligation, ObligationClass};
use psi_terminal::{Block, TerminalMachine, Terminator};

use super::super::substitution::substitute_proposition_places;
use super::{ReconstructedOperationObligation, path_facts};

pub(super) fn append_terminator(
    terminator: &Terminator,
    machine: &TerminalMachine,
    blocks: &BTreeMap<BlockId, &Block>,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    reconstruct_path_facts: bool,
    mut axioms: Vec<Proposition>,
    incoming: &mut BTreeMap<BlockId, Vec<Vec<Proposition>>>,
    exits: &mut Vec<Vec<Proposition>>,
    operation_obligations: &mut Vec<ReconstructedOperationObligation>,
) {
    match terminator {
        Terminator::Jump {
            target, arguments, ..
        } => {
            let target_block = blocks.get(target).expect("validator requires jump target");
            path_facts::bind_successor_axioms(
                &mut axioms,
                target_block,
                arguments,
                value_term,
                reconstruct_path_facts,
            );
            incoming.entry(*target).or_default().push(axioms);
        }
        Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            let true_fact = path_facts::true_condition_fact(*condition, &axioms, value_term);
            for (successor, condition_fact) in [(when_true, true_fact.as_ref()), (when_false, None)]
            {
                let target_block = blocks
                    .get(&successor.target)
                    .expect("validator requires conditional target");
                let mut arm_axioms = axioms.clone();
                path_facts::bind_successor_axioms(
                    &mut arm_axioms,
                    target_block,
                    &successor.arguments,
                    value_term,
                    reconstruct_path_facts,
                );
                if reconstruct_path_facts && let Some(condition_fact) = condition_fact {
                    path_facts::append_successor_fact(
                        &mut arm_axioms,
                        condition_fact,
                        target_block,
                        &successor.arguments,
                        value_term,
                    );
                }
                incoming
                    .entry(successor.target)
                    .or_default()
                    .push(arm_axioms);
            }
        }
        Terminator::Return {
            value,
            cleanup_actions,
            ..
        } => {
            let result = machine
                .result
                .scalar()
                .expect("validated scalar return has a scalar machine result");
            axioms.push(Proposition::Equal(
                value_term(result.id),
                value_term(*value),
            ));
            for cleanup in cleanup_actions.iter().filter_map(|action| match action {
                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => Some(cleanup),
                psi_terminal::TerminalAffineCleanupAction::DiscardRoot(_)
                | psi_terminal::TerminalAffineCleanupAction::DiscardResidual(_) => None,
            }) {
                append_nominal_cleanup_obligations(
                    cleanup,
                    machines,
                    &axioms,
                    operation_obligations,
                );
            }
            exits.push(axioms);
        }
        Terminator::ReturnUnitNominalAffine { cleanups, .. } => {
            for cleanup in cleanups {
                append_nominal_cleanup_obligations(
                    cleanup,
                    machines,
                    &axioms,
                    operation_obligations,
                );
            }
            exits.push(axioms);
        }
        Terminator::ReturnUnit { .. } | Terminator::ReturnUnitPartialAffine { .. } => {
            exits.push(axioms)
        }
        Terminator::ReturnStructural {
            returned_claims, ..
        } => {
            axioms.extend(
                machine
                    .content_identity_reshuffles
                    .iter()
                    .filter(|reshuffle| returned_claims.contains(&reshuffle.claim))
                    .flat_map(|reshuffle| reshuffle.inferred_propositions()),
            );
            exits.push(axioms);
        }
        // A crash establishes no normal-return guarantee. Its explicit
        // frontier record is validated structurally before proof replay.
        Terminator::Crash { .. } => {}
    }
}

fn append_nominal_cleanup_obligations(
    cleanup: &psi_terminal::NominalAffineCleanup,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    axioms: &[Proposition],
    operation_obligations: &mut Vec<ReconstructedOperationObligation>,
) {
    let target = machines
        .get(&cleanup.cleanup_machine)
        .copied()
        .expect("validated nominal cleanup target exists");
    let receiver = cleanup
        .cleanup_receiver
        .map(|receiver| BTreeMap::from([(receiver, cleanup.place)]))
        .unwrap_or_default();
    for (required, obligation) in target
        .contract
        .requires
        .iter()
        .zip(&cleanup.requirement_obligations)
    {
        operation_obligations.push(ReconstructedOperationObligation {
            obligation: Obligation {
                id: *obligation,
                proposition: substitute_proposition_places(required, &receiver),
                class: ObligationClass::Derivable,
            },
            semantic_axioms: axioms.to_vec(),
            canonical_certificate: false,
        });
    }
}
