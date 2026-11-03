//! Capture entry return sources at the same point entry storage becomes sealed.
//! The ordinary return validator consumes these exact backings just as it does
//! the caller-frame roster. A host opting in accepts their retention until the
//! completed instance closes; an opaque result identity alone supplies no loan.

use super::ReferenceBacking;
use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::engine::execution::TerminalExecution;
use evaluation_accounting::EvaluationStorageLease;

#[derive(Default)]
pub(crate) struct EntryReferenceBackings {
    pub(super) rows: Vec<Vec<ReferenceBacking>>,
    // Drop captured descriptor metadata before its logical-cell reservation.
    _lease: Option<EvaluationStorageLease>,
}

impl TerminalExecution<'_> {
    pub(crate) fn capture_entry_reference_backings(
        &mut self,
    ) -> Result<(), TerminalInterpretError> {
        let machine = self.program.machine(self.current_machine)?;
        let Some(signature) = machine
            .result
            .structural()
            .filter(|signature| !signature.reference_sources.is_empty())
        else {
            return Ok(());
        };
        if self.entry_reference_backings.is_none() {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "host reference results require an explicit loan carrier interface",
            ));
        }
        // One cell per result slot and captured source, reserved before the
        // binder allocates. Referent payloads retain their existing accounts.
        let cells = signature
            .reference_sources
            .iter()
            .try_fold(0u64, |cells, source| {
                cells
                    .checked_add(1)?
                    .checked_add(u64::try_from(source.sources.len()).ok()?)
            })
            .ok_or_else(malformed)?;
        let lease = self
            .account
            .as_ref()
            .map(|account| account.reserve_storage(cells, 0))
            .transpose()
            .map_err(TerminalInterpretError::Storage)?;
        let rows = self.bind_reference_return_backings(
            &machine.structural_parameters,
            signature,
            &self.structural_values,
            &self.aggregate_argument_backings,
            &self.primitive_loan_values,
            &self.element_view_values,
            &self.byte_sequence_values,
            &self.access_view_occurrences,
        )?;
        self.entry_reference_backings = Some(EntryReferenceBackings {
            rows,
            _lease: lease,
        });
        Ok(())
    }
}
