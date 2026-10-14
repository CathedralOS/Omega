//! Ordered cleanup for returns and selected successors. Captured outgoing values
//! and the original frame survive hook suspension; only completion installs them.
use super::*;
use crate::errors::malformed;
use crate::values::TerminalStructuralValue;
use terminal_psi::{NominalAffineCleanup, TerminalAffineCleanupAction};

impl TerminalExecution<'_> {
    pub(super) fn continue_cleanup_actions(
        &mut self,
        mut remaining: std::vec::IntoIter<TerminalAffineCleanupAction>,
        continuation: CleanupContinuation,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        while let Some(action) = remaining.next() {
            let nominal = commit_cleanup_action(
                &self.program.structural_types(),
                &self.structural_cases,
                &self.program.machines(),
                &mut self.structural_values,
                &mut self.reference_referents,
                &mut self.structural_element_view_fields,
                &mut self.live_affine_frontier,
                &mut self.live_claims,
                &action,
            )?;
            if let Some(completed) = nominal {
                return self.start_nominal_cleanup(completed, remaining, continuation);
            }
        }
        match continuation {
            CleanupContinuation::Return(result) => {
                if !self.live_affine_frontier.is_empty() || !self.live_claims.is_empty() {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                // Initialized homes are not ownership obligations. A partial
                // move or release can settle every resource while scalar
                // siblings remain observable until this frame completes.
                // The checked cleanup schedule has now finished; retire its
                // bindings without pruning backing retained by outgoing values.
                self.structural_values.clear();
                self.finish_return(result)
            }
            CleanupContinuation::Successor { target, bindings } => {
                bindings.commit(self, target)?;
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
        let callee = self.program.verified_machine(completed.0.cleanup_machine)?;
        let entry = callee.entry;
        let target = completed.0.cleanup_machine;
        let receiver = completed
            .0
            .cleanup_receiver
            .map(|place| (place, completed.1.clone()));
        self.suspend_with(SuspendedCallResult::NominalCleanups {
            completed,
            remaining,
            continuation,
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
        self.placed_view_occurrences = caller.placed_view_occurrences;
        self.values = caller.values;
        self.structural_values = caller.structural_values;
        self.aggregate_argument_backings = caller.aggregate_argument_backings;
        self.scalar_array_values = caller.scalar_array_values;
        self.element_view_values = caller.element_view_values;
        self.byte_sequence_values = caller.byte_sequence_values;
        self.primitive_loan_values = caller.primitive_loan_values;
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
        let loan_completions = caller.loan_completions.clone();
        let expected = self.restore_return_caller(caller);
        // Structural result rebinding was staged against the suspended caller.
        // Install that candidate before loan completion so it cannot later
        // overwrite the refreshed owner roster with the old saved frontier.
        if let Some(claims) = completed.rebound_claims {
            self.live_claims = claims;
        }
        self.complete_loan_custody(&loan_completions)?;
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
                        if let Some(binding) = completed.byte_sequence {
                            self.byte_sequence_values.insert(result.place, binding);
                        }
                        if let Some(binding) = completed.element_view {
                            self.element_view_values.insert(result.place, binding);
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
                // A completed call establishes the same partition as any
                // completed owned value. Rebound active claims own their
                // regions; adding a whole-root debt would overlap them and
                // make forwarding this result fail its next return.
                let carried = crate::custody::whole_value_frontier(
                    &self.program.structural_types(),
                    result.place,
                    result.structural_type,
                    result.multiplicity,
                    self.live_claim_paths_at(result.place),
                    &|current| self.stored_case_at_discard(&self.structural_values, current),
                )?;
                if carried
                    .iter()
                    .any(|entry| self.live_affine_frontier.contains(entry))
                {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
                self.live_affine_frontier.extend(carried);
                Ok(TerminatorFlow::Continue)
            }
        }
    }

    /// Borrowed and unrestricted inputs are frame bindings, not owed owned roots.
    /// The completed result has already been removed before this retirement.
    pub(super) fn retire_return_inputs(&mut self) {
        if let Some(machine) = self.program.machines().get(&self.current_machine) {
            for parameter in machine
                .structural_parameters
                .iter()
                .chain(machine.block_structural_parameters())
            {
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
