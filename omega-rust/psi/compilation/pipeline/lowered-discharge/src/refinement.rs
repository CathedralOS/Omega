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
use terminal_check_tables::Checked;
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
        for (_, verdict) in result.verdicts.iter() {
            verdict.as_ref().map_err(Clone::clone)?;
        }
        for used in &result.uses {
            used.verdict.as_ref().map_err(Clone::clone)?;
        }
        for consumed in &result.windows {
            consumed.verdict.as_ref().map_err(Clone::clone)?;
        }
    }
    Ok(())
}
