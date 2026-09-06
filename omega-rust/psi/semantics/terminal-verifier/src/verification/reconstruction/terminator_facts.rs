//! Ordered successor and return-fact reconstruction for one terminator.

use std::collections::BTreeMap;

use proof_admission::{Obligation, ObligationClass};
use semantic_vocabulary::{
    BlockId, EdgeId, MachineId, Proposition, ScalarTerm, StructuralCaseSubject, ValueId,
};
use terminal_psi::OutcomeSpecificGuard;
use terminal_psi::{Block, TerminalMachine, Terminator};

use super::super::substitution::substitute_proposition_places;
use super::{
    ReconstructedCrashSiteFacts, ReconstructedOperationObligation,
    ReconstructedTerminalObligationOwner, path_facts,
};

pub(super) fn append_terminator(
    terminator: &Terminator,
    block: BlockId,
    machine: &TerminalMachine,
    blocks: &BTreeMap<BlockId, &Block>,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    reconstruct_path_facts: bool,
    crash_facts: bool,
    mut axioms: Vec<Proposition>,
    incoming: &mut BTreeMap<BlockId, Vec<Vec<Proposition>>>,
    exits: &mut Vec<Vec<Proposition>>,
    outcome_guard: Option<OutcomeSpecificGuard>,
    outcome_exits: &mut BTreeMap<OutcomeSpecificGuard, Vec<Vec<Proposition>>>,
    operation_obligations: &mut Vec<ReconstructedOperationObligation>,
    crash_sites: &mut Vec<ReconstructedCrashSiteFacts>,
    ignored_backedges: &std::collections::BTreeSet<EdgeId>,
) {
    match terminator {
        Terminator::Jump {
            edge,
            target,
            arguments,
            ..
        } => {
            if ignored_backedges.contains(edge) {
                return;
            }
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
            let true_fact = path_facts::condition_fact(*condition, true, &axioms, value_term);
            let false_fact = path_facts::condition_fact(*condition, false, &axioms, value_term);
            for (successor, condition_fact, positive) in [
                (when_true, true_fact.as_ref(), true),
                (when_false, false_fact.as_ref(), false),
            ] {
                if ignored_backedges.contains(&successor.edge) {
                    continue;
                }
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
                if crash_facts {
                    // Current observations still have exact SSA truth on the
                    // selected edge even when their entry-field origin is
                    // unavailable. Keep ordinary obligation axiom indexes
                    // unchanged by retaining this only in the private mode.
                    path_facts::append_successor_fact(
                        &mut arm_axioms,
                        &Proposition::Equal(value_term(*condition), ScalarTerm::Boolean(positive)),
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
        Terminator::StructuralCase { source, cases } => {
            for successor in cases {
                if ignored_backedges.contains(&successor.edge) {
                    continue;
                }
                let mut arm_axioms = axioms.clone();
                arm_axioms.push(Proposition::StructuralCaseMembership {
                    subject: StructuralCaseSubject::new(*source, Vec::new()),
                    case: successor.case,
                });
                incoming
                    .entry(successor.target)
                    .or_default()
                    .push(arm_axioms);
            }
        }
        Terminator::Return {
            edge,
            value,
            cleanup_actions,
        } => {
            let result = machine
                .result
                .scalar()
                .expect("validated scalar return has a scalar machine result");
            axioms.push(Proposition::Equal(
                value_term(result.id),
                value_term(*value),
            ));
            for (cleanup_position, cleanup) in
                cleanup_actions
                    .iter()
                    .enumerate()
                    .filter_map(|(position, action)| match action {
                        terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                            Some((position, cleanup))
                        }
                        terminal_psi::TerminalAffineCleanupAction::DiscardRoot(_)
                        | terminal_psi::TerminalAffineCleanupAction::DiscardResidual(_) => None,
                    })
            {
                append_nominal_cleanup_obligations(
                    cleanup,
                    machine.id,
                    *edge,
                    cleanup_position,
                    machines,
                    &axioms,
                    operation_obligations,
                );
            }
            exits.push(axioms);
        }
        Terminator::ReturnUnitNominalAffine { edge, cleanups } => {
            for (cleanup_position, cleanup) in cleanups.iter().enumerate() {
                append_nominal_cleanup_obligations(
                    cleanup,
                    machine.id,
                    *edge,
                    cleanup_position,
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
            source,
            returned_claims,
            ..
        } => {
            let result = machine
                .result
                .structural()
                .expect("validated structural return has a structural machine result");
            let substitutions = BTreeMap::from([(*source, result.place)]);
            for proposition in axioms
                .clone()
                .into_iter()
                .map(|proposition| substitute_proposition_places(&proposition, &substitutions))
            {
                if !axioms.contains(&proposition) {
                    axioms.push(proposition);
                }
            }
            axioms.extend(
                machine
                    .content_identity_reshuffles
                    .iter()
                    .filter(|reshuffle| returned_claims.contains(&reshuffle.claim))
                    .flat_map(|reshuffle| reshuffle.inferred_propositions()),
            );
            if let Some(guard) = outcome_guard {
                outcome_exits.entry(guard).or_default().push(axioms.clone());
            }
            exits.push(axioms);
        }
        // A crash establishes no normal-return guarantee. Retain only facts
        // reconstructed before it, not the producer's asserted site guards.
        Terminator::Crash { edge, .. } => {
            // Ranked reconstruction omits backedges and therefore does not
            // establish all-path invariants at these sites. Until invariant
            // custody is available, only independent entry requirements may
            // prove a ranked machine's crash guards.
            if machine.ranked_scc.is_some() {
                axioms.clear();
            } else if crash_facts {
                axioms.retain(|proposition| {
                    super::crash_field_origins::retains_entry_meaning(proposition, machine)
                });
            }
            crash_sites.push(ReconstructedCrashSiteFacts {
                machine: machine.id,
                block,
                edge: *edge,
                semantic_axioms: axioms,
            });
        }
    }
}

fn append_nominal_cleanup_obligations(
    cleanup: &terminal_psi::NominalAffineCleanup,
    machine: MachineId,
    edge: semantic_vocabulary::EdgeId,
    cleanup_position: usize,
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
    for (requirement_position, (required, obligation)) in target
        .contract
        .requires
        .iter()
        .zip(&cleanup.requirement_obligations)
        .enumerate()
    {
        operation_obligations.push(ReconstructedOperationObligation {
            owner: ReconstructedTerminalObligationOwner::NominalCleanupRequires {
                machine,
                edge,
                cleanup_position: u32::try_from(cleanup_position)
                    .expect("validated cleanup position fits u32"),
                requirement_position: u32::try_from(requirement_position)
                    .expect("validated cleanup requirement position fits u32"),
            },
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
