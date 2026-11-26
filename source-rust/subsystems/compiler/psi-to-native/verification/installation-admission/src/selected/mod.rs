//! Read an installed handler's selected control graph and symbolic operands.
pub mod control;
#[cfg(test)]
mod control_tests;
pub mod cpu_origins;
pub mod expressions;
pub mod fault_origins;
pub mod spill_values;

use control::Graph;
use expressions::{Expressions, Word};
use selected_instructions::{SelectedFunction, SelectedInstruction, SelectedInstructionId};
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Mismatch(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mismatch(reason) => formatter.write_str(reason),
        }
    }
}

impl std::error::Error for Error {}

pub fn selected_constant_operand(
    function: &SelectedFunction,
    instruction: SelectedInstructionId,
    operand: u16,
) -> Result<Option<u64>, Error> {
    let mut rows = function
        .blocks
        .iter()
        .flat_map(|block| {
            block.instructions.iter().chain(std::iter::once(
                selected_instructions::terminator_instruction(&block.terminator),
            ))
        })
        .filter(|row| row.id == instruction);
    let row = rows.next().ok_or(Error::Mismatch(
        "constant operand has no unique function-local instruction",
    ))?;
    if rows.next().is_some() {
        return Err(Error::Mismatch(
            "constant operand has no unique function-local instruction",
        ));
    }
    let [_] = row.provenance.operations.as_slice() else {
        return Err(Error::Mismatch(
            "constant operand lost its exact source operation",
        ));
    };
    if row
        .operand(operand)
        .is_none_or(|value| value.access != register_model::RegisterOperandAccess::Use)
    {
        return Err(Error::Mismatch(
            "constant query does not name a selected input operand",
        ));
    }
    let graph = Graph::new(function)?;
    let mut values = Expressions::new(function, &graph)?;
    let expression = values.input(row, operand)?;
    Ok(match expression.word() {
        Word::Constant(value) => Some(*value),
        _ => None,
    })
}

pub fn correlate_native_instruction_sources(
    selected: &post_allocation_machine::PostAllocationMachineProgram,
    physical: impl IntoIterator<Item = (MachineId, SelectedInstructionId)>,
) -> Result<Vec<(MachineId, &SelectedInstruction)>, Error> {
    let mut instructions: Vec<_> = selected
        .functions
        .iter()
        .flat_map(|function| {
            function.blocks.iter().flat_map(move |block| {
                block
                    .instructions
                    .iter()
                    .chain(std::iter::once(
                        selected_instructions::terminator_instruction(&block.terminator),
                    ))
                    .map(move |instruction| ((function.machine, instruction.id), instruction))
            })
        })
        .collect();
    instructions.sort_unstable_by_key(|row| row.0);
    if instructions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(Error::Mismatch(
            "installed fault analysis has duplicate selected instructions",
        ));
    }
    let mut seen = vec![false; instructions.len()];
    let mut correlated = Vec::with_capacity(instructions.len());
    for owner in physical {
        let index = instructions
            .binary_search_by_key(&owner, |row| row.0)
            .map_err(|_| {
                Error::Mismatch("installed physical instruction has no exact selected source")
            })?;
        if std::mem::replace(&mut seen[index], true) {
            return Err(Error::Mismatch(
                "installed physical instruction is duplicated",
            ));
        }
        let ((machine, _), source) = instructions[index];
        correlated.push((machine, source));
    }
    if seen.iter().any(|present| !present) {
        return Err(Error::Mismatch(
            "installed physical closure omits a selected instruction",
        ));
    }
    Ok(correlated)
}

pub fn verify_privileged_termination(
    execution: Option<&provider_admission::AdmittedInstalledEntryExecution>,
    selected: &post_allocation_machine::PostAllocationMachineProgram,
) -> Result<(), Error> {
    use selected_instructions::{SelectedInstructionKind as Kind, SelectedTerminator};
    if let Some(execution) = execution {
        let entry = execution.entry().entry();
        if entry.psi() != selected.psi
            || entry.target() != selected.target
            || entry.machine() != selected.entry
            || execution.scope().bootstrap_assumption().is_none()
        {
            return Err(Error::Mismatch(
                "privileged termination changed its installed object domain",
            ));
        }
    }
    for function in selected.functions.iter() {
        for block in &function.blocks {
            if block
                .instructions
                .iter()
                .any(|instruction| instruction.kind == Kind::PrivilegedCrash)
            {
                return Err(Error::Mismatch(
                    "privileged stop is not an exact nonreturning terminator",
                ));
            }
            let terminal = selected_instructions::terminator_instruction(&block.terminator);
            let trap = matches!(
                block.terminator,
                SelectedTerminator::EliminateEmpty { .. }
                    | SelectedTerminator::Crash { .. }
                    | SelectedTerminator::OperationTrap { .. }
            );
            let expected = if execution.is_some() {
                Kind::PrivilegedCrash
            } else {
                Kind::Crash
            };
            if trap && terminal.kind != expected || !trap && terminal.kind == Kind::PrivilegedCrash
            {
                return Err(Error::Mismatch(
                    "trap realization differs from its admitted execution domain",
                ));
            }
        }
    }
    Ok(())
}
