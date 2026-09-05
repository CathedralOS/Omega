use super::rejected;
use crate::capture::semantics::facts::exactly_one;
use crate::record::{PackagePolicyMutation, PackageReviewWriteFrameCompleteness};
use checked_trees::RealizedMachineContractEnvelope;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use typed_trees::{machine::Machine, state::State};

pub(crate) fn mutation<'program>(
    compilation: &CheckedCompilation,
    source: &'program typed_trees::TypedTrees,
    resolver: &validation::CallFrameResolver<'program>,
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
    let source_machine = exactly_one(
        source
            .machines()
            .iter()
            .filter(|candidate| candidate.symbol == machine.symbol),
        machine.name.as_str(),
        "pre-settlement machine",
    )?;
    let source_entry = source
        .machine_states(source_machine)
        .first()
        .ok_or_else(|| rejected("pre-settlement callable has no entry"))?;
    if source_machine != machine || source_entry != entry {
        return Err(rejected("pre-settlement callable declaration changed"));
    }
    let derived = resolver.inferred_state_write_frame(source_machine, source_entry);
    if derived != retained.frame {
        return Err(rejected(&format!(
            "entry write frame for `{}` differs from its current transitive typed derivation: retained {:?}, derived {:?}",
            machine.name.as_str(),
            retained.frame,
            derived,
        )));
    }
    Ok(PackagePolicyMutation {
        completeness: match derived.completeness() {
            facts::WriteFrameCompleteness::Complete => {
                PackageReviewWriteFrameCompleteness::Complete
            }
            facts::WriteFrameCompleteness::Opaque => PackageReviewWriteFrameCompleteness::Opaque,
        },
        paths: derived.paths().to_vec(),
    })
}
