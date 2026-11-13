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
) -> Checked<()> {
    let mut witness = elaborate_callable_witness(typed, required, supplied)?;
    let discharged = crate::discharge(&mut witness)?;
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
    if let Some(excess) = crate::ceilings::judge(&witness.module, &witness.tables)?
        .into_iter()
        .next()
    {
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
            },
        ));
    }
    Ok(())
}
