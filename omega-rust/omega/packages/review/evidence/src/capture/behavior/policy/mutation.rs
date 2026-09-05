use super::rejected;
use crate::capture::semantics::facts::exactly_one;
use crate::record::{PackagePolicyMutation, PackageReviewWriteFrameCompleteness};
use omega_compiler::CheckedCompilation;
use psi_checked_trees::RealizedMachineContractEnvelope;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::{machine::Machine, state::State};

pub(crate) fn mutation(
    compilation: &CheckedCompilation,
    machine: &Machine,
    entry: &State,
    envelope: &RealizedMachineContractEnvelope,
) -> Result<PackagePolicyMutation, Vec<Diagnostic>> {
    if machine.symbol != envelope.machine
        || compilation
            .machine_states(machine)
            .first()
            .map(|state| state.symbol)
            != Some(entry.symbol)
    {
        return Err(rejected(
            "mutation query does not name the canonical callable entry",
        ));
    }
    let retained = exactly_one(
        envelope
            .mutation
            .iter()
            .filter(|frame| frame.state == entry.symbol),
        machine.name.as_str(),
        "entry write frame",
    )?;
    let resolver = psi_validation::CallFrameResolver::new(&compilation.typed)
        .ok_or_else(|| rejected("entry write frame has no exact call resolver"))?;
    let derived = resolver.inferred_state_write_frame(machine, entry);
    if derived != retained.frame {
        return Err(rejected(
            "entry write frame differs from its current transitive typed derivation",
        ));
    }
    Ok(PackagePolicyMutation {
        completeness: match derived.completeness() {
            psi_facts::WriteFrameCompleteness::Complete => {
                PackageReviewWriteFrameCompleteness::Complete
            }
            psi_facts::WriteFrameCompleteness::Opaque => {
                PackageReviewWriteFrameCompleteness::Opaque
            }
        },
        paths: derived.paths().to_vec(),
    })
}
