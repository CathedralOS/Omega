//! Ordered successor and return-fact reconstruction for one terminator.

use std::collections::BTreeMap;

use proof_admission::{Obligation, ObligationClass};
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, EdgeId, MachineId, Proposition, PropositionContext,
    ScalarTerm, ScalarType, StructuralCaseSubject, ValueId,
};
use terminal_psi::OutcomeSpecificGuard;
use terminal_psi::{Block, TerminalMachine, Terminator};

use super::super::substitution::{SharedRoster, substitute_proposition_places};
use super::{
    ReconstructedCrashSiteFacts, ReconstructedOperationObligation,
    ReconstructedTerminalObligationOwner, operation_facts, path_facts,
};

pub(super) fn append_terminator(
    module: &terminal_psi::TerminalModule,
    terminator: &Terminator,
    block: BlockId,
    machine: &TerminalMachine,
    blocks: &BTreeMap<BlockId, &Block>,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    proposition_context: &PropositionContext,
    reconstruct_path_facts: bool,
    crash_facts: bool,
    mut axioms: Vec<Proposition>,
    incoming: &mut BTreeMap<BlockId, Vec<SharedRoster>>,
    exits: &mut Vec<SharedRoster>,
    outcome_guard: Option<OutcomeSpecificGuard>,
    outcome_exits: &mut BTreeMap<OutcomeSpecificGuard, Vec<SharedRoster>>,
    operation_obligations: &mut Vec<ReconstructedOperationObligation>,
    crash_sites: &mut Vec<ReconstructedCrashSiteFacts>,
    ignored_backedges: &std::collections::BTreeSet<EdgeId>,
) {
    match terminator {
        Terminator::Jump {
            edge,
            target,
            arguments,
            structural_arguments,
            ..
        } => {
            if ignored_backedges.contains(edge) {
                return;
            }
            let target_block = blocks.get(target).expect("validator requires jump target");
            let mut roster = SharedRoster::whole(axioms);
            path_facts::bind_successor_axioms(
                &mut roster,
                target_block,
                arguments,
                value_term,
                proposition_context,
                reconstruct_path_facts,
            );
            path_facts::bind_successor_structural_axioms(
                &mut roster,
                target_block,
                structural_arguments,
                reconstruct_path_facts,
            );
            incoming.entry(*target).or_default().push(roster);
        }
        Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            let true_fact = path_facts::condition_fact(
                *condition,
                true,
                &axioms,
                value_term,
                proposition_context,
            );
            let false_fact = path_facts::condition_fact(
                *condition,
                false,
                &axioms,
                value_term,
                proposition_context,
            );
            let shared = SharedRoster::whole(axioms);
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
                let mut arm_axioms = shared.successor();
                path_facts::bind_successor_axioms(
                    &mut arm_axioms,
                    target_block,
                    &successor.arguments,
                    value_term,
                    proposition_context,
                    reconstruct_path_facts,
                );
                path_facts::bind_successor_structural_axioms(
                    &mut arm_axioms,
                    target_block,
                    &successor.structural_arguments,
                    reconstruct_path_facts,
                );
                if reconstruct_path_facts && let Some(condition_fact) = condition_fact {
                    path_facts::append_successor_fact(
                        &mut arm_axioms,
                        &condition_fact.proposition,
                        target_block,
                        &successor.arguments,
                        value_term,
                        proposition_context,
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
                        proposition_context,
                    );
                }
                incoming
                    .entry(successor.target)
                    .or_default()
                    .push(arm_axioms);
            }
        }
        Terminator::StructuralCase { source, cases } => {
            let shared = SharedRoster::whole(axioms);
            for successor in cases {
                if ignored_backedges.contains(&successor.edge) {
                    continue;
                }
                let mut arm_axioms = shared.successor();
                arm_axioms.push(Proposition::StructuralCaseMembership {
                    subject: StructuralCaseSubject::new(*source, Vec::new()),
                    case: successor.case,
                });
                let target = blocks
                    .get(&successor.target)
                    .expect("validated structural case target");
                for (field, parameter) in successor.payload_fields.iter().zip(&target.parameters) {
                    let path = vec![
                        CanonicalStructuralPathSegment::Case(successor.case),
                        CanonicalStructuralPathSegment::Field(*field),
                    ];
                    if let Some(field_type) = selected_payload_refinement(
                        module,
                        machine,
                        *source,
                        successor.case,
                        *field,
                    ) {
                        // The copied SSA payload keeps its declared predicate even
                        // after its former storage root is mutated or consumed.
                        for fact in operation_facts::declared_scalar_field_facts(
                            proposition_context,
                            field_type,
                            parameter.id,
                        ) {
                            if fact.certified {
                                arm_axioms.push(fact.proposition);
                            }
                        }
                    }
                    // These are current-storage observations. The ordinary root
                    // mutation invalidation and iteration cuts govern their lifetime.
                    let field_term = match parameter.scalar_type {
                        ScalarType::Boolean => ScalarTerm::boolean_field_path(*source, path),
                        ScalarType::Integer(integer) => {
                            ScalarTerm::integer_field_path(*source, path, integer)
                        }
                        // The proposition vocabulary has no IEEE field term.
                        ScalarType::IeeeFloat(_) => continue,
                    };
                    arm_axioms.push(Proposition::Equal(value_term(parameter.id), field_term));
                }
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
            crate::verification::substitution::push_unique_fact(
                &mut axioms,
                Proposition::Equal(value_term(result.id), value_term(*value)),
            );
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
            exits.push(SharedRoster::whole(axioms));
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
            exits.push(SharedRoster::whole(axioms));
        }
        Terminator::ReturnUnit { .. } | Terminator::ReturnUnitPartialAffine { .. } => {
            exits.push(SharedRoster::whole(axioms))
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
            let established_count = axioms.len();
            let mut fact_index = crate::verification::substitution::FactIndex::new(&axioms);
            for index in 0..established_count {
                let proposition = substitute_proposition_places(&axioms[index], &substitutions);
                if fact_index.position_of(&axioms, &proposition).is_none() {
                    fact_index.record(axioms.len(), &proposition);
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
            let exit = SharedRoster::whole(axioms);
            if let Some(guard) = outcome_guard {
                outcome_exits.entry(guard).or_default().push(exit.clone());
            }
            exits.push(exit);
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
                    super::crash_field_origins::retains_entry_meaning(proposition, module, machine)
                });
            }
            crash_sites.push(ReconstructedCrashSiteFacts {
                owner: terminal_psi::CrashObligationOwner::Site {
                    machine: machine.id,
                    block,
                    edge: *edge,
                },
                semantic_axioms: axioms,
            });
        }
    }
}

fn selected_payload_refinement<'module>(
    module: &'module terminal_psi::TerminalModule,
    machine: &TerminalMachine,
    source: semantic_vocabulary::PlaceId,
    selected_case: semantic_vocabulary::StructuralCaseId,
    selected_field: semantic_vocabulary::StructuralFieldId,
) -> Option<&'module terminal_psi::StructuralFieldType> {
    // Formation and full-graph dominance have already validated this exact
    // source. Contract field paths deliberately have a narrower parameter scope.
    let structural_type = terminal_semantics::arrival_parameter(machine, source)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| operation.result.structural())
                .find(|result| result.place == source)
                .map(|result| result.structural_type)
        })?;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)?;
    let (terminal_psi::StructuralTypeShape::Sum { cases }
    | terminal_psi::StructuralTypeShape::Mixed { cases, .. }) = &declaration.shape
    else {
        return None;
    };
    let field = cases
        .iter()
        .find(|case| case.id == selected_case)?
        .fields
        .iter()
        .find(|field| field.id == selected_field)?;
    terminal_semantics::scalar_field_requires_evidence(&field.field_type)
        .then_some(&field.field_type)
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
