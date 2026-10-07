//! Ordered cleanup for returns and selected successors. Captured outgoing values
//! and the original frame survive hook suspension; only completion installs them.
use super::*;
use crate::errors::malformed;
use crate::values::TerminalStructuralValue;
use terminal_psi::{NominalAffineCleanup, TerminalAffineCleanupAction};

impl TerminalExecution {
    pub(super) fn continue_cleanup_actions(
        &mut self,
        mut remaining: std::vec::IntoIter<TerminalAffineCleanupAction>,
        continuation: CleanupContinuation,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        while let Some(action) = remaining.next() {
            let nominal = commit_cleanup_action(
                &self.program.structural_types,
                &self.program.machines,
                &mut self.structural_values,
                &mut self.reference_referents,
                &mut self.structural_element_view_fields,
                &mut self.scalar_case_values,
                &mut self.live_affine_frontier,
                &mut self.live_claims,
                &action,
                remaining.as_slice().first(),
            )?;
            if let Some(completed) = nominal {
                return self.start_nominal_cleanup(completed, remaining, continuation);
            }
        }
        match continuation {
            CleanupContinuation::Return(result) => {
                if !self.structural_values.is_empty()
                    || !self.live_affine_frontier.is_empty()
                    || !self.live_claims.is_empty()
                {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                self.finish_return(result)
            }
            CleanupContinuation::Successor { target, bindings } => {
                bindings.commit(self);
                self.current = target;
                self.next_operation = 0;
                Ok(TerminatorFlow::Continue)
            }
        }
    }

    fn start_nominal_cleanup(
        &mut self,
        completed: (NominalAffineCleanup, TerminalStructuralValue),
        remaining: std::vec::IntoIter<TerminalAffineCleanupAction>,
        continuation: CleanupContinuation,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let callee = self
            .program
            .machines
            .get(&completed.0.cleanup_machine)
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(
                completed.0.cleanup_machine,
            ))?;
        let entry = callee.entry;
        let target = completed.0.cleanup_machine;
        let receiver = completed
            .0
            .cleanup_receiver
            .map(|place| (place, completed.1.clone()));
        self.call_stack.push(SuspendedCall {
            values: std::mem::take(&mut self.values),
            structural_values: std::mem::take(&mut self.structural_values),
            byte_sequence_values: std::mem::take(&mut self.byte_sequence_values),
            scalar_case_values: std::mem::take(&mut self.scalar_case_values),
            scalar_array_values: std::mem::take(&mut self.scalar_array_values),
            element_view_values: std::mem::take(&mut self.element_view_values),
            live_affine_frontier: std::mem::take(&mut self.live_affine_frontier),
            live_claims: std::mem::take(&mut self.live_claims),
            current_machine: self.current_machine,
            current: self.current,
            next_operation: self.next_operation,
            result: SuspendedCallResult::NominalCleanups {
                completed,
                remaining,
                continuation,
            },
        });
        if let Some((place, value)) = receiver {
            self.structural_values.insert(place, value);
        }
        self.current_machine = target;
        self.current = entry;
        self.next_operation = 0;
        Ok(TerminatorFlow::Continue)
    }

    fn restore_return_caller(&mut self, caller: SuspendedCall) -> SuspendedCallResult {
        self.retire_plain_locals();
        self.values = caller.values;
        self.structural_values = caller.structural_values;
        self.scalar_case_values = caller.scalar_case_values;
        self.scalar_array_values = caller.scalar_array_values;
        self.element_view_values = caller.element_view_values;
        self.byte_sequence_values = caller.byte_sequence_values;
        self.live_affine_frontier = caller.live_affine_frontier;
        self.live_claims = caller.live_claims;
        self.current_machine = caller.current_machine;
        self.current = caller.current;
        self.next_operation = caller.next_operation;
        caller.result
    }

    pub(super) fn finish_return(
        &mut self,
        completed: CompletedReturn,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Some(caller) = self.call_stack.pop() else {
            if completed.destination.is_some() {
                return Err(malformed());
            }
            self.retire_plain_locals();
            self.result = Some(completed.value.clone());
            return Ok(TerminatorFlow::Yield(TerminalExecutionStatus::Complete(
                completed.value,
            )));
        };
        let expected = self.restore_return_caller(caller);
        match expected {
            SuspendedCallResult::NominalCleanups {
                completed: cleanup,
                remaining,
                continuation,
            } => {
                if completed.value != TerminalExecutionResult::Unit {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                crate::custody::complete_nominal_cleanup(
                    &mut self.structural_values,
                    &mut self.live_affine_frontier,
                    &self.live_claims,
                    &cleanup.0,
                    remaining.as_slice().first(),
                )?;
                self.continue_cleanup_actions(remaining, continuation)
            }
            SuspendedCallResult::Unit => {
                if completed.value != TerminalExecutionResult::Unit {
                    return Err(malformed());
                }
                Ok(TerminatorFlow::Continue)
            }
            SuspendedCallResult::Scalar(destination) => {
                let TerminalExecutionResult::Scalar(value) = completed.value else {
                    return Err(malformed());
                };
                self.values.insert(destination, value);
                Ok(TerminatorFlow::Continue)
            }
            SuspendedCallResult::Structural { result, .. } => {
                if completed.destination.as_ref() != Some(&result) {
                    return Err(malformed());
                }
                match completed.value {
                    TerminalExecutionResult::Structural(returned) => {
                        if self
                            .structural_values
                            .insert(result.place, returned.value)
                            .is_some()
                        {
                            return Err(malformed());
                        }
                        if let Some(claims) = completed.rebound_claims {
                            self.live_claims = claims;
                        }
                        if let Some(binding) = completed.byte_sequence {
                            self.byte_sequence_values.insert(result.place, binding);
                        }
                        if let Some(binding) = completed.element_view {
                            self.element_view_values.insert(result.place, binding);
                        }
                    }
                    TerminalExecutionResult::ScalarCase(returned) => {
                        if self
                            .scalar_case_values
                            .insert(result.place, returned.value)
                            .is_some()
                        {
                            return Err(malformed());
                        }
                    }
                    TerminalExecutionResult::ScalarArray(returned) => {
                        if self
                            .scalar_array_values
                            .insert(result.place, returned.value)
                            .is_some()
                        {
                            return Err(malformed());
                        }
                    }
                    _ => return Err(malformed()),
                }
                if result.multiplicity == StructuralMultiplicity::Affine
                    && !self.live_affine_frontier.insert(StructuralAffineDiscard {
                        place: result.place,
                        path: Vec::new(),
                        structural_type: result.structural_type,
                    })
                {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                Ok(TerminatorFlow::Continue)
            }
        }
    }

    /// Borrowed and unrestricted inputs are frame bindings, not owed owned roots.
    /// The completed result has already been removed before this retirement.
    pub(super) fn retire_return_inputs(&mut self) {
        if let Some(machine) = self.program.machines.get(&self.current_machine) {
            for parameter in machine.structural_parameters.iter().chain(
                machine
                    .blocks
                    .values()
                    .flat_map(|block| &block.structural_parameters),
            ) {
                if parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    || parameter.access != StructuralAccess::Owned
                {
                    self.structural_values.remove(&parameter.place);
                }
            }
        }
        self.retire_plain_locals();
    }
}
