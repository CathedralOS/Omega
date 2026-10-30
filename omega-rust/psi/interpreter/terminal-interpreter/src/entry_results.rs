//! Provisional values captured at successful root-activation completion events.
//!
//! Runtime value tables overwrite on repeated execution; they cannot serve as
//! occurrence evidence. A requested result instead owns one bounded slot for
//! this invocation. Recursive activations cannot fill it, a second completion
//! refuses, and normal root return requires every requested occurrence. These
//! observations grant no proof or target admission.

use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::runtime_rows::RuntimeRows;
use crate::values::TerminalScalarValue;
use semantic_vocabulary::{MachineId, ScalarType, ValueId};

#[cfg(test)]
mod tests;

/// Observe exact scalar operation results of the resolved entry activation.
/// Deduplicate repeated references before requesting them. The roster is fixed
/// at instantiation and carries no permission to execute an otherwise pending
/// program or omit its retained conditions.
#[derive(Clone, Copy)]
pub struct TerminalEntryResultRequest<'request> {
    pub machine: MachineId,
    pub results: &'request [(ValueId, ScalarType)],
}

pub(crate) struct EntryResults {
    machine: MachineId,
    rows: RuntimeRows<ValueId, (ScalarType, Option<TerminalScalarValue>)>,
}

fn failure(reason: &'static str) -> TerminalInterpretError {
    TerminalInterpretError::EntryResultObservation(reason)
}

impl TerminalExecution<'_> {
    pub(crate) fn request_entry_results(
        &mut self,
        request: TerminalEntryResultRequest<'_>,
    ) -> Result<(), TerminalInterpretError> {
        if !self.entry_storage_open || self.entry_results.is_some() {
            return Err(failure(
                "entry result observations must be fixed before execution",
            ));
        }
        if request.machine != self.current_machine || !self.call_stack.is_empty() {
            return Err(failure(
                "entry result observations name a different root activation",
            ));
        }
        let machine = self.program.verified_machine(request.machine)?;
        let mut rows = RuntimeRows::new(self.account.as_ref());
        for &(result, scalar_type) in request.results {
            if rows.contains_key(&result) {
                return Err(failure("entry result observation request repeats a result"));
            }
            let declared = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| operation.result.scalar())
                .find(|declaration| declaration.id == result);
            if declared.is_none_or(|declaration| declaration.scalar_type != scalar_type) {
                return Err(failure(
                    "entry result observation lacks its exact operation and carrier",
                ));
            }
            rows.insert_vacant(result, (scalar_type, None))?;
        }
        self.entry_results = Some(EntryResults {
            machine: request.machine,
            rows,
        });
        Ok(())
    }

    /// Called only after a successful operation or caller-return installation.
    /// Stack depth distinguishes recursive uses of the same machine. Ordinary
    /// calls use return installation, not the immediate-operation hook.
    pub(crate) fn observe_completed_entry_result(
        &mut self,
        result: ValueId,
    ) -> Result<(), TerminalInterpretError> {
        if !self.call_stack.is_empty() {
            return Ok(());
        }
        let Some(observations) = &mut self.entry_results else {
            return Ok(());
        };
        if self.current_machine != observations.machine {
            return Ok(());
        }
        let Some((expected, observed)) = observations.rows.get_mut(&result) else {
            return Ok(());
        };
        if observed.is_some() {
            return Err(failure("entry result occurrence completed more than once"));
        }
        let value = self
            .values
            .get(&result)
            .copied()
            .ok_or_else(|| failure("completed entry result has no scalar value"))?;
        if value.scalar_type() != *expected {
            return Err(failure(
                "completed entry result changed its declared carrier",
            ));
        }
        *observed = Some(value);
        Ok(())
    }

    pub(crate) fn require_entry_results_complete(&self) -> Result<(), TerminalInterpretError> {
        if self.entry_results.as_ref().is_some_and(|observations| {
            observations.rows.values().any(|(_, value)| value.is_none())
        }) {
            return Err(failure(
                "entry returned without completing every requested result occurrence",
            ));
        }
        Ok(())
    }

    /// A completed observation remains provisional: the consumer must discharge
    /// the application's conditions and retain exact artifact/target custody.
    pub(crate) fn observed_entry_result(
        &self,
        machine: MachineId,
        result: ValueId,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        if self.result.is_none() || self.failure.is_some() {
            return Err(failure(
                "entry result observations require successful normal completion",
            ));
        }
        let observations = self
            .entry_results
            .as_ref()
            .filter(|observations| observations.machine == machine)
            .ok_or_else(|| failure("entry result observation names an unrequested activation"))?;
        observations
            .rows
            .get(&result)
            .and_then(|(_, value)| *value)
            .ok_or_else(|| failure("entry result occurrence was not requested or not completed"))
    }
}
