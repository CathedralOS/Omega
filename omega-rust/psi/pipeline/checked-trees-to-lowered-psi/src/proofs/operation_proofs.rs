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

const PARALLEL_PROOF_THRESHOLD: usize = 16;
const MAX_PROOF_WORKERS: usize = 8;

pub(crate) fn finalize_operation_proofs(lowered: &mut LoweredPsi) -> Result<(), LoweringError> {
    finalize_operation_proofs_inner(
        lowered,
        #[cfg(test)]
        &|_| {},
    )
}

fn finalize_operation_proofs_inner(
    lowered: &mut LoweredPsi,
    #[cfg(test)] prepared_machine: &(impl Fn(MachineId) + Sync),
) -> Result<(), LoweringError> {
    crate::proofs::scalar_block_invariants::retain_provable(lowered)?;
    let validated = terminal_verifier::validate_module(&lowered.semantic_module)
        .map_err(LoweringError::InvalidTerminalModule)?;
    let obligations = terminal_verifier::reconstruct_execution_terminal_obligations(validated)
        .map_err(LoweringError::InvalidTerminalModule)?;
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
    let owners = lowered
        .semantic_module
        .machines
        .iter()
        .map(|machine| (machine.id, (machine, OnceLock::new())))
        .collect::<BTreeMap<_, _>>();
    let produce = |site: &terminal_verifier::ReconstructedTerminalObligation| {
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
                let context = validated.value_context(machine)?;
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
            produce_checked_canonical_integer_proof(
                context,
                &site.obligation.proposition,
                assumptions,
                &site.semantic_axioms,
                machine_parameter_values,
            )
            // Obligations whose operands meet cited facts only through
            // equality/definition chains outgrow the canonical custody
            // envelope; the relaxed search keeps the kernel as final
            // authority without loosening canonical producer contracts.
            .or_else(|| {
                produce_relaxed_integer_proof(
                    context,
                    &site.obligation.proposition,
                    assumptions,
                    &site.semantic_axioms,
                    machine_parameter_values,
                )
            })
        } else {
            proof_from_available_facts(
                &site.obligation.proposition,
                assumptions,
                &site.semantic_axioms,
            )
        };
        let proof = proof.ok_or(LoweringError::OperationProofUnavailable(site.obligation.id))?;
        Ok::<_, LoweringError>(ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(site.obligation.id.get())
                    .expect("terminal obligations have nonzero identities"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        })
    };
    let generated = if pending.len() < PARALLEL_PROOF_THRESHOLD {
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
                .map(|(_, evidence)| evidence)
                .collect::<Result<Vec<_>, _>>()
        })?
    };
    lowered.proof_bundle.evidence.extend(generated);
    lowered
        .proof_bundle
        .evidence
        .sort_by_key(|evidence| evidence.obligation);
    crate::proofs::control_cycle_proofs::finalize(lowered)?;
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
