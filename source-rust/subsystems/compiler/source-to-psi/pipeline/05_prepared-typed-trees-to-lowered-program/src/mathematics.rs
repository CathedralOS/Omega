/*
The compilation's mathematical declarations as one kernel signature.

Data that contains itself has no storage. The machines over it are read by the
ordinary reader, in a reader of their own so none of them is lowered, and each
becomes one entry of the signature: the data a W type, a machine defined by
cases a definition, and a machine with no result a law whose proof stage 06
produces from the calls its body makes.
*/

use crate::source_state::TypedTrees;
use semantic_vocabulary::{MachineId, PackageKeyIdentity};
use source_provenance::DependencyScope;
use terminal_check_tables::Refusal;
use terminal_check_tables::mathematics::MathematicalSignature;

pub(crate) fn read(
    typed: &TypedTrees,
    (subjects, purpose): &(Vec<Option<PackageKeyIdentity>>, DependencyScope),
    captures: crate::lowering::SharedReading,
) -> Result<MathematicalSignature, Vec<Refusal>> {
    let mut program =
        crate::lowering::read::Program::with_captures(typed, false, *purpose, captures);
    program.mathematical = true;
    program.local_summary_scope = Some(crate::lowering::read::LocalSummaryScope::Compilation(
        subjects.clone(),
    ));
    let first = MachineId::new(1).expect("1 is nonzero");
    let alone = |refused: Refusal| vec![refused];
    let (machines, data) = program
        .mathematical_subjects(first, subjects, *purpose)
        .map_err(alone)?;
    if machines.is_empty() && data.is_empty() {
        return Ok(MathematicalSignature::default());
    }
    program.collected = Some(Vec::new());
    for declared in data {
        let read = program.mathematical_data(first, declared);
        program.keep(read).map_err(alone)?;
    }
    for root in machines {
        let read = program.machine(root).map(|_| ());
        program.keep(read).map_err(alone)?;
    }
    let bodies = crate::lowering::read_bodies(&mut program, first).map_err(alone)?;
    let refused = program.collected.take().unwrap_or_default();
    if !refused.is_empty() {
        return Err(refused);
    }
    let mut satisfactions = Vec::new();
    let mut refused = Vec::new();
    for position in 1..=program.symbols.len() as u64 {
        let id = MachineId::new(position).expect("positions start at 1");
        match program.satisfaction(id) {
            Ok(Some(satisfaction)) => satisfactions.push(satisfaction),
            Ok(None) => {}
            Err(refusal) => refused.push(refusal),
        }
    }
    if !refused.is_empty() {
        return Err(refused);
    }
    program.mathematical_signature(&bodies, &satisfactions)
}
