/*
A callable refinement is a proof under a published contract, not an executable
closure. The witness owns the required entry assumptions and calls an abstract
supplied contract. Ordinary discharge proves that call's requirements before
using its guarantees and proves every normal exit of the witness. The supplied
body is checked independently; reading it here would strengthen its public
contract with private implementation facts.

Whole-program `judge` deliberately refuses abstract requirement rows. This
operation instead returns only a proof verdict for the witness, with no module,
publication or executable acceptance. It leaves the abstract rows and their
Unsupported verdicts intact.
*/

use structure_elaboration::{CallableDeclaration, elaborate_callable_witness};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::{Checked, Construct, RefusalReason};
use typed_trees::TypedTrees;
use typed_trees::signature::StateSignature;

/// Prove the required callable contract from the supplied published contract.
/// Formation, lifetime, progress and effect refinement remain separately owed
/// by the caller. An unsupported logical form keeps its precise refusal.
pub fn prove_callable_refinement<'a>(
    typed: &'a TypedTrees,
    required: &'a StateSignature,
    supplied: CallableDeclaration<'a>,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<()> {
    let mut witness = elaborate_callable_witness(typed, required, supplied)?;
    let discharged = crate::discharge(&mut witness, activity)?;
    for (machine, result) in discharged.machines.iter() {
        if witness.requirements.contains(&machine) {
            continue;
        }
        // Every concrete body is owed, including all exits, loan judgments,
        // storage reads and consumption points of open invariant windows.
        // Abstract guarantees are premises only after their symbolic call;
        // they are never treated as proved implementation bodies here.
        let tables = terminal_check_tables::row(
            &witness.tables.machines,
            machine,
            machine,
            Construct::Machine,
        )?;
        for (obligation, verdict) in result.verdicts.iter() {
            let Err(refused) = verdict else { continue };
            let row = terminal_check_tables::row(
                &tables.obligations,
                obligation,
                machine,
                Construct::Obligation(obligation),
            )?;
            // A published clause restates its failing return. Report the
            // return's actual cause, especially NoRuleForGoalForm, instead
            // of converting an incomplete proof into ReturnDoesNotEstablish.
            // The return remains in this same obligation inventory and must
            // refuse below; no failing clause can disappear from the proof.
            if matches!(row.kind, ObligationKind::PublishedGuarantee { .. })
                && tables.obligations.iter().any(|(exit, row)| {
                    matches!(row.kind, ObligationKind::NormalGuarantee { published, .. } if published == obligation)
                        && matches!(result.verdicts.get(exit), Some(Err(refused)) if matches!(refused.reason, RefusalReason::Unproved(_)))
                })
            {
                continue;
            }
            return Err(refused.clone());
        }
        for used in &result.uses {
            used.verdict.as_ref().map_err(Clone::clone)?;
        }
        for consumed in &result.windows {
            consumed.verdict.as_ref().map_err(Clone::clone)?;
        }
    }
    // effects.md: "provider reach subset-of requirement reach ceiling, when
    // one is declared". The witness stands for the required contract and
    // calls the supplied one, so the per-call containment is that rule. A
    // supplied callable outside the ceiling is refused here, not reported
    // as a rejection: this operation has no rejection of its own. The same
    // containment on the entries axis is "Bound the entries of every
    // admitted implementation".
    if let Some(excess) = crate::ceilings::judge(&witness)?.into_iter().next() {
        return Err(crate::refusal(
            RefusalReason::Unsupported,
            excess.machine,
            excess.at,
            match excess.axis {
                crate::ceilings::Axis::Reach => {
                    "supplied callable reaches a service outside the required ceiling"
                }
                crate::ceilings::Axis::Entries => {
                    "supplied callable enters a callback outside the required invokes ceiling"
                }
                crate::ceilings::Axis::Suspension => {
                    "supplied callable may suspend outside the required ceiling"
                }
                crate::ceilings::Axis::Blocking => {
                    "supplied callable may block outside the required ceiling"
                }
                crate::ceilings::Axis::Termination => {
                    "supplied callable lacks the required termination guarantee"
                }
            },
        ));
    }
    Ok(())
}

/// Complete the catalog's retained proof questions without any source-checker
/// hypotheses. Existing certificates are replayed rather than replaced, so a
/// malformed imported proof never becomes a freshly trusted construction.
pub fn finalize_callable_refinements(
    module: &mut terminal_psi::TerminalModule,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<()> {
    use terminal_semantics::calls::callable_refinement::{
        callable_refinement_context, descriptor_refinement_goals,
        validate_callable_evidence_contracts_in, validate_descriptor_refinement_evidence,
    };
    // A real construction owes its operational contract before acceptance,
    // even if its logical refinement already has retained certificates.
    // Abstract contract witnesses have no descriptor or nominal publication.
    if let Some(descriptor) = module.callables.descriptors.first() {
        terminal_semantics::callable_envelopes::validate_callable_operational_refinements(module)
            .map_err(|detail| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                descriptor.machine,
                Construct::Machine,
                detail,
            )
        })?;
    }
    let mut prepared = Vec::with_capacity(module.callables.descriptors.len());
    for descriptor in &module.callables.descriptors {
        let refuse = |reason, detail: &str| {
            crate::refusal(reason, descriptor.machine, Construct::Machine, detail)
        };
        let goals = descriptor_refinement_goals(module, descriptor.id)
            .map_err(|detail| refuse(RefusalReason::ForeignIdentity, detail))?;
        if !descriptor.refinement_evidence.is_empty() {
            // Receiving admission below authenticates the exact retained
            // questions and certificates, including their identity and count.
            continue;
        }
        let context = callable_refinement_context(module, descriptor.contract)
            .map_err(|detail| refuse(RefusalReason::ForeignIdentity, detail))?;
        let contract = module
            .callables
            .contract(descriptor.contract)
            .ok_or_else(|| {
                refuse(
                    RefusalReason::ForeignIdentity,
                    "descriptor has no callable contract",
                )
            })?;
        let declarations = contract
            .scalar_parameters
            .iter()
            .chain(&contract.proof.erased_scalar_formals)
            .chain(contract.proof_result.scalar_ref())
            .map(|formal| (formal.id, formal.scalar_type))
            .collect::<Vec<_>>();
        let certificates = goals
            .into_iter()
            .map(|(id, goal)| {
                crate::derive_contract_obligation(
                    &context,
                    id,
                    &goal.proposition,
                    &declarations,
                    activity,
                )
                .map_err(|detail| {
                    refuse(
                        RefusalReason::Unproved(terminal_check_tables::UnprovedCause::NoDerivation),
                        detail,
                    )
                })
            })
            .collect::<Checked<Vec<_>>>()?;
        prepared.push((descriptor.id, certificates));
    }
    if let Some(descriptor) = module.callables.descriptors.first() {
        validate_callable_evidence_contracts_in(
            &module.callables,
            &module.proposition_declarations,
            &module.proposition_applications,
            &module.evidence_terms,
            &module.evidence_contract_lanes,
        )
        .map_err(|detail| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                descriptor.machine,
                Construct::Machine,
                detail,
            )
        })?;
    }
    for (id, certificates) in prepared {
        let descriptor = module
            .callables
            .descriptors
            .iter_mut()
            .find(|row| row.id == id)
            .expect("prepared descriptors remain in the borrowed catalog");
        descriptor.refinement_evidence = certificates;
    }
    if let Some(descriptor) = module.callables.descriptors.first() {
        validate_descriptor_refinement_evidence(module).map_err(|detail| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                descriptor.machine,
                Construct::Machine,
                detail,
            )
        })?;
    }
    Ok(())
}
