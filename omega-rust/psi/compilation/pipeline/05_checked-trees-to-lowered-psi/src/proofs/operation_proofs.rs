//! Complete a finished module's operation evidence in reconstructed obligation order.

use super::LoweringError;
use super::nonzero_divisor_certificate::{
    produce_checked_canonical_integer_proof, produce_relaxed_integer_proof,
};
#[cfg(test)]
use crate::machine_lowering::lower_machine;
#[cfg(test)]
use crate::terminal_identities::obligation_id;
use lowered_psi::LoweredPsi;
use proof_admission::{
    CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
};
#[cfg(test)]
use semantic_vocabulary::MachineId;
use semantic_vocabulary::{EvidenceIdentity, Proposition};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use terminal_verifier::ObligationEvidence;

pub(crate) const PARALLEL_PROOF_THRESHOLD: usize = 16;
pub(crate) const MAX_PROOF_WORKERS: usize = 8;

pub(crate) fn finalize_operation_proofs(lowered: &mut LoweredPsi) -> Result<(), LoweringError> {
    finalize_operation_proofs_inner(
        lowered,
        crate::proofs::scalar_block_invariants::MAX_ROSTER_ROUNDS,
        #[cfg(test)]
        &|_| {},
    )
}

fn finalize_operation_proofs_inner(
    lowered: &mut LoweredPsi,
    roster_rounds: usize,
    #[cfg(test)] prepared_machine: &(impl Fn(MachineId) + Sync),
) -> Result<(), LoweringError> {
    let _retain_scope =
        crate::proofs::proof_timing::Scope::enter("finalize_operation_proofs:retain_provable");
    let mut declines = super::ProofSearchDeclines::default();
    super::scalar_result_guarantees::retain_provable(lowered, &mut declines)?;
    declines.extend(
        crate::proofs::scalar_block_invariants::retain_provable_bounded(lowered, roster_rounds)?,
    );
    drop(_retain_scope);
    crate::emission::operation_emission::element_unification::unify_hoisted_element_reads(
        &mut lowered.semantic_module,
    );
    crate::emission::operation_emission::place_declarations::reconcile_leaf_copy_place_declarations(
        &mut lowered.semantic_module,
    );
    let started = std::time::Instant::now();
    let validated = terminal_verifier::validate_module(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    crate::proofs::proof_timing::count_validate(started.elapsed());
    let started = std::time::Instant::now();
    let obligations = terminal_verifier::reconstruct_execution_terminal_obligations(validated)
        .map_err(LoweringError::InvalidTerminalModule)?;
    crate::proofs::proof_timing::count_reconstruct(started.elapsed());
    let existing = lowered
        .proof_bundle
        .evidence
        .iter()
        .map(|evidence| evidence.obligation)
        .collect::<BTreeSet<_>>();
    // Some closure builders have already supplied source-derived evidence for
    // contextual call obligations and closed contracts. Reconstruct every site, but
    // synthesize only obligations that remain undispatched; the final verifier
    // still checks the retained evidence against the exact goal.
    // Nominal cleanup builders leave their obligations pending so certificates
    // cite live field observations at each completed edge, not entry-list indexes.
    let pending = obligations
        .obligations()
        .iter()
        .filter(|site| !existing.contains(&site.obligation.id))
        .cloned()
        .collect::<Vec<_>>();
    if std::env::var("OMEGA_DUMP_OBLIGATIONS").is_ok() {
        for site in &pending {
            eprintln!(
                "[obligation] id={:?} owner={:?} canonical={} proposition={:?} requirements={:?} axioms={:?}",
                site.obligation.id,
                site.owner,
                site.canonical_certificate,
                site.obligation.proposition,
                site.requirements,
                site.semantic_axioms,
            );
        }
    }
    let owners = lowered
        .semantic_module
        .machines
        .iter()
        .map(|machine| (machine.id, (machine, OnceLock::new())))
        .collect::<BTreeMap<_, _>>();
    let produce = |site: &terminal_verifier::ReconstructedTerminalObligation| {
        let _counted = crate::proofs::proof_timing::Counted::enter(
            crate::proofs::proof_timing::count_producer,
        );
        // Each search records into its own ledger so workers never borrow the
        // batch's; declines join in batch order the same way verdicts do.
        let mut search_declines = super::ProofSearchDeclines::default();
        let owner = owners.get(&site.owner.machine());
        let assumptions = site.requirements.as_slice();
        let proof = if let Some((machine, preparation)) = owner
            && (site.canonical_certificate
                || matches!(
                    site.owner,
                    terminal_verifier::ReconstructedTerminalObligationOwner::ContractEnsures { .. }
                        | terminal_verifier::ReconstructedTerminalObligationOwner::CallRequires { .. }
                )) {
            // This invocation borrows an immutable module. Share only machine
            // facts, not site assumptions or proof-search state. Cache failures
            // too, but report them at the original pending-site position.
            let preparation = preparation.get_or_init(|| {
                #[cfg(test)]
                prepared_machine(machine.id);
                let started = std::time::Instant::now();
                let context = validated.value_context(machine)?;
                crate::proofs::proof_timing::count_value_context(started.elapsed());
                let machine_parameter_values = machine
                    .parameters
                    .iter()
                    .map(|parameter| parameter.id)
                    .collect::<BTreeSet<_>>();
                Ok::<_, terminal_verifier::ModuleError>((context, machine_parameter_values))
            });
            let (context, machine_parameter_values) = preparation
                .as_ref()
                .map_err(|error| LoweringError::InvalidTerminalModule(error.clone()))?;
            let started = std::time::Instant::now();
            let mut canonical_elapsed = None;
            let mut relaxed_ran = false;
            let proof = produce_checked_canonical_integer_proof(
                context,
                &site.obligation.proposition,
                assumptions,
                &site.semantic_axioms,
                machine_parameter_values,
                &mut search_declines,
            )
            .inspect(|_| canonical_elapsed = Some(started.elapsed()))
            // Obligations whose operands meet cited facts only through
            // equality/definition chains outgrow the canonical custody
            // envelope; the relaxed search keeps the kernel as final
            // authority without loosening canonical producer contracts.
            .or_else(|| {
                canonical_elapsed = Some(started.elapsed());
                relaxed_ran = true;
                produce_relaxed_integer_proof(
                    context,
                    &site.obligation.proposition,
                    assumptions,
                    &site.semantic_axioms,
                    machine_parameter_values,
                    &mut search_declines,
                )
            })
            // Contract `Atom` cites — a retained float-meaning equality row,
            // for example — are vocabulary-level goals the integer producers
            // cannot shape; they still discharge through the site's retained
            // facts and semantic axioms, the same route non-owner sites take.
            .or_else(|| {
                proof_from_available_facts(
                    &site.obligation.proposition,
                    assumptions,
                    &site.semantic_axioms,
                )
            })
            // A goal over storage the body last wrote, such as an IEEE range
            // row restored by a store, holds once its observations follow
            // the store and value equations to closed literals.
            .or_else(|| {
                proof_by_value_equality_transport(
                    context,
                    &site.obligation.proposition,
                    &site.semantic_axioms,
                )
            });
            if crate::proofs::proof_timing::enabled() {
                let canonical = canonical_elapsed.unwrap_or_default();
                crate::proofs::proof_timing::search(
                    "obligation",
                    site.obligation.id.get(),
                    &site.obligation.proposition,
                    assumptions.len(),
                    site.semantic_axioms.len(),
                    canonical,
                    relaxed_ran.then(|| started.elapsed().saturating_sub(canonical)),
                    proof.is_some(),
                );
            }
            proof
        } else {
            proof_from_available_facts(
                &site.obligation.proposition,
                assumptions,
                &site.semantic_axioms,
            )
        };
        Ok::<_, LoweringError>((proof, search_declines))
    };
    let outcomes = if pending.len() < PARALLEL_PROOF_THRESHOLD {
        pending.iter().map(produce).collect::<Result<Vec<_>, _>>()?
    } else {
        // Certificate searches are independent and read only validated module
        // state. Bound the pool so one large compile can use the host without
        // turning ordinary concurrent test runs into nested fan-out.
        let worker_count = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .min(MAX_PROOF_WORKERS)
            .min(pending.len());
        let next = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            let produce = &produce;
            let pending = pending.as_slice();
            let mut workers = Vec::with_capacity(worker_count);
            for _ in 0..worker_count {
                let next = &next;
                workers.push(scope.spawn(move || {
                    let mut generated = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(site) = pending.get(index) else {
                            break;
                        };
                        generated.push((index, produce(site)));
                    }
                    generated
                }));
            }
            let mut generated = Vec::with_capacity(pending.len());
            for worker in workers {
                generated.extend(
                    worker
                        .join()
                        .expect("operation-proof synthesis worker does not panic"),
                );
            }
            generated.sort_by_key(|(index, _)| *index);
            generated
                .into_iter()
                .map(|(_, outcome)| outcome)
                .collect::<Result<Vec<_>, _>>()
        })?
    };
    let mut generated = Vec::with_capacity(outcomes.len());
    for (site, (proof, local)) in pending.iter().zip(outcomes) {
        // A produced site's own declines merge before its unproved verdict
        // reads the ledger, in batch order. An unproved site is the ordinary
        // rejection only while no named capacity declined: after a decline the
        // refusal is the publication contract's `Incomplete` outcome, never a
        // claim the site is false.
        declines.extend(local);
        let proof = proof.ok_or_else(|| declines.unproved_obligation_error(site.obligation.id))?;
        generated.push(ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(site.obligation.id.get())
                    .expect("terminal obligations have nonzero identities"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        });
    }
    lowered.proof_bundle.evidence.extend(generated);
    lowered
        .proof_bundle
        .evidence
        .sort_by_key(|evidence| evidence.obligation);
    crate::proofs::control_cycle_proofs::finalize(lowered, &mut declines)?;
    crate::proofs::proof_timing::report("finalize_operation_proofs done");
    Ok(())
}

#[cfg(test)]
mod tests;

fn proof_from_available_facts(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    if goal == &Proposition::Truth {
        return Some(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        });
    }
    if matches!(goal, Proposition::Equal(left, right) if left == right) {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
        });
    }
    if let Some(index) = assumptions.iter().position(|assumption| assumption == goal) {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Assumption { index },
        });
    }
    if let Some(index) = semantic_axioms.iter().position(|axiom| axiom == goal) {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::SemanticAxiom { index },
        });
    }
    // An imported guarantee arrives as one conjunction: select the goal from
    // it by conjunction elimination.
    let premises = assumptions
        .iter()
        .enumerate()
        .map(|(index, premise)| (premise, ProofRule::Assumption { index }))
        .chain(
            semantic_axioms
                .iter()
                .enumerate()
                .map(|(index, premise)| (premise, ProofRule::SemanticAxiom { index })),
        );
    for (premise, rule) in premises {
        if let Some(proof) = conjunct_of(
            ProofNode {
                conclusion: premise.clone(),
                rule,
            },
            goal,
            0,
        ) {
            return Some(proof);
        }
    }
    let Proposition::Conjunction(conjuncts) = goal else {
        return None;
    };
    let proofs = conjuncts
        .iter()
        .map(|conjunct| proof_from_available_facts(conjunct, assumptions, semantic_axioms))
        .collect::<Option<Vec<_>>>()?;
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ConjunctionIntroduction(proofs),
    })
}

/// Prove `goal` by transporting it along the site's value and field
/// definition equations until the kernel's predicate denotation decides it
/// `true`: the premise is plain truth, and every cited equation is one of
/// the site's own semantic axioms. The kernel re-runs the same transport.
fn proof_by_value_equality_transport(
    context: &semantic_vocabulary::PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    use semantic_vocabulary::ScalarTerm;
    let equalities = semantic_axioms
        .iter()
        .enumerate()
        .filter(|(_, axiom)| {
            matches!(
                axiom,
                Proposition::Equal(
                    ScalarTerm::Value { .. }
                        | ScalarTerm::IntegerField { .. }
                        | ScalarTerm::BooleanField { .. }
                        | ScalarTerm::IeeeFloatField { .. },
                    _,
                )
            )
        })
        .collect::<Vec<_>>();
    if equalities.is_empty() {
        return None;
    }
    let transported = proof_admission::check_value_equality_denotation(
        context,
        goal,
        equalities.iter().map(|(_, equality)| *equality),
    )
    .ok()?;
    (transported == Proposition::Truth).then(|| ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(ProofNode {
                conclusion: Proposition::Truth,
                rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
            }),
            equalities: equalities
                .into_iter()
                .map(|(index, equality)| ProofNode {
                    conclusion: equality.clone(),
                    rule: ProofRule::SemanticAxiom { index },
                })
                .collect(),
        },
    })
}

/// Project `goal` out of a proved conjunction, descending through nested
/// conjunctions a bounded depth.
fn conjunct_of(proof: ProofNode, goal: &Proposition, depth: usize) -> Option<ProofNode> {
    const MAXIMUM_CONJUNCTION_DEPTH: usize = 8;
    let Proposition::Conjunction(conjuncts) = &proof.conclusion else {
        return None;
    };
    if depth >= MAXIMUM_CONJUNCTION_DEPTH {
        return None;
    }
    let conjuncts = conjuncts.clone();
    for (conjunct, candidate) in conjuncts.iter().enumerate() {
        let projected = ProofNode {
            conclusion: candidate.clone(),
            rule: ProofRule::ConjunctionElimination {
                conjunction: Box::new(proof.clone()),
                conjunct,
            },
        };
        if candidate == goal {
            return Some(projected);
        }
        if let Some(found) = conjunct_of(projected, goal, depth + 1) {
            return Some(found);
        }
    }
    None
}
