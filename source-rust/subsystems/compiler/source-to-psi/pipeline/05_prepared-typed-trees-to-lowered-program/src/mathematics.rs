/*
The compilation's mathematical declarations as one kernel signature.

Data that contains itself has no storage. The machines over it are read by the
ordinary reader, in a reader of their own so none of them is lowered, and each
becomes one entry of the signature: the data its canonical trees, a machine
defined by cases a definition, and a machine that states an equality a law
whose proof stage 06 produces from the calls its body makes.

The roots are the subject's own machines over such data. A machine that
satisfies a requirement of a trait also brings the rest of that conformance,
wherever it is declared, so a program that only applies a library operation
still has the library's laws for it judged and available.
*/

use crate::source_state::TypedTrees;
use semantic_vocabulary::{MachineId, PackageKeyIdentity};
use source_provenance::DependencyScope;
use terminal_check_tables::Refusal;
use terminal_check_tables::mathematics::MathematicalSignature;

pub(crate) fn read(
    typed: &TypedTrees,
    (subjects, purpose): &(Vec<Option<PackageKeyIdentity>>, DependencyScope),
    captures: crate::SharedReading,
) -> Result<MathematicalSignature, Vec<Refusal>> {
    let mut program =
        crate::read::Program::with_captures(typed, false, *purpose, captures);
    program.mathematical = true;
    program.local_summary_scope = Some(crate::read::LocalSummaryScope::Compilation(
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
    // The machines that run and hold evidence of indexed data: what they
    // build and pass is checked here, and nowhere else.
    let read = program.evidence_uses(first, subjects, *purpose);
    program.keep(read).map_err(alone)?;
    // A machine that satisfies a requirement brings the rest of its
    // conformance: every machine over such data that satisfies a requirement
    // of the same trait at the same data. A program that only adds two
    // values still gets the laws of that addition, which is what licenses
    // regrouping and reordering its operands. The laws are judged with the
    // program, like everything else the signature holds.
    loop {
        let mut conformances = Vec::new();
        for (_, symbol) in program.symbols.iter() {
            if let Some(conformance) = typed
                .machine_by_symbol(*symbol)
                .and_then(|machine| program.conformance(machine))
                && !conformances.contains(&conformance)
            {
                conformances.push(conformance);
            }
        }
        let mut missing = Vec::new();
        for machine in typed.machines() {
            if machine.purpose.is_active_case_visitor()
                || !program.proof_machine(machine)
                || crate::is_generic_template(typed, machine)
                || program
                    .symbols
                    .iter()
                    .any(|(_, read)| *read == machine.symbol)
                || typed
                    .machine_instantiation(machine.symbol)
                    .is_ok_and(|instantiation| instantiation.is_some())
            {
                continue;
            }
            if program
                .conformance(machine)
                .is_some_and(|conformance| conformances.contains(&conformance))
            {
                missing.push(machine.symbol);
            }
        }
        if missing.is_empty() {
            break;
        }
        for root in missing {
            let read = program.machine(root).map(|_| ());
            program.keep(read).map_err(alone)?;
        }
    }
    // The law that a machine's fuel suffices cites core's natural-order
    // bound, which no call names, so it is read with the naturals.
    // A program may hold more than one copy of core's naturals: the bound of
    // a copy the program reads from is read, and only that.
    let source = |symbol| {
        typed
            .symbols
            .symbol_source_span(symbol)
            .map(|span| span.source_id)
    };
    let used: Vec<_> = program
        .symbols
        .iter()
        .filter_map(|(_, &symbol)| source(symbol))
        .collect();
    let mut bounds = Vec::new();
    for machine in typed.machines() {
        if source(machine.symbol).is_some_and(|source| used.contains(&source))
            && typed.core_vocabulary(machine.symbol)
                == Some(language_core::core_vocabulary::CoreVocabulary::NaturalFuelBound)
            && program.proof_machine(machine)
            && !program
                .symbols
                .iter()
                .any(|(_, read)| *read == machine.symbol)
        {
            bounds.push(machine.symbol);
        }
    }
    for bound in bounds {
        let read = program.machine(bound).map(|_| ());
        program.keep(read).map_err(alone)?;
    }
    let bodies = crate::read_bodies(&mut program, first).map_err(alone)?;
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
