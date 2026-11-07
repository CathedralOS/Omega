//! Privileged stop bytes require the exact installed private-object domain.
//! Source and target replay still validate each crash edge or containment guard;
//! this check prevents ordinary objects or unrelated instructions using the form.
use crate::FunctionFragmentObjectArtifactError as Error;
use provider_admission::AdmittedInstalledEntryExecution;
use selected_instructions::{
    SelectedInstructionKind as Kind, SelectedInstructionPlan, SelectedTerminator,
};

pub(super) fn validate(
    execution: Option<&AdmittedInstalledEntryExecution>,
    selected: &SelectedInstructionPlan,
) -> Result<(), Error> {
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
    for function in &selected.functions {
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
            let crash = matches!(
                block.terminator,
                SelectedTerminator::Crash { .. } | SelectedTerminator::OperationTrap { .. }
            );
            let expected = if execution.is_some() {
                Kind::PrivilegedCrash
            } else {
                Kind::Crash
            };
            if crash && terminal.kind != expected
                || !crash && terminal.kind == Kind::PrivilegedCrash
            {
                return Err(Error::Mismatch(
                    "crash realization differs from its admitted execution domain",
                ));
            }
        }
    }
    Ok(())
}
