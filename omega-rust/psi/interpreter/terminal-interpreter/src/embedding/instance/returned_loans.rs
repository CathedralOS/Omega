//! Explicit host retention of checked entry-result loans. Handles borrow the
//! completed instance, so closing it or resuming guest work cannot invalidate a
//! live host observation. They expose no mutable access or guest value token.

use super::TerminalInstance;
use crate::boundary::values::TerminalStructuralValue;
use crate::engine::execution::TerminalExecution;
use crate::storage::reference::EntryReferenceBackings;
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, PlaceId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralTypeShape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalReturnedLoanError {
    Closed,
    AdmissionAfterStart,
    AdmissionAlreadyProvided,
    NoReferenceResult,
    AdmissionRequired,
    NormalReturnRequired,
    UnknownPath,
    UnreadableAccess,
    Execution(crate::TerminalInterpretError),
}

impl From<crate::TerminalInterpretError> for TerminalReturnedLoanError {
    fn from(error: crate::TerminalInterpretError) -> Self {
        Self::Execution(error)
    }
}

/// A read-only host observation of one returned reference's original storage.
/// `access` reports the guest contract's ceiling; it grants no host mutation,
/// disposal, qualification, address, or permission to construct a guest value.
pub struct TerminalReturnedLoan<'instance, 'artifact> {
    execution: &'instance TerminalExecution<'artifact>,
    carrier: &'instance TerminalStructuralValue,
    source: PlaceId,
    path: Vec<Path>,
    referent: StructuralTypeId,
    access: StructuralAccess,
}

impl<'artifact> TerminalInstance<'_, 'artifact> {
    /// Accept retention of the entry result's checked loans until this instance
    /// closes. Call before the first invoke/resume, including a zero-work one.
    /// Final entry backing is captured when execution starts, after any explicit
    /// entry-storage installation. Ordinary return checks still authenticate
    /// every active descriptor against its complete possible source roster.
    pub fn retain_returned_loans(&mut self) -> Result<(), TerminalReturnedLoanError> {
        let execution = self
            .execution
            .as_mut()
            .ok_or(TerminalReturnedLoanError::Closed)?;
        if !execution.entry_storage_open {
            return Err(TerminalReturnedLoanError::AdmissionAfterStart);
        }
        if execution.entry_reference_backings.is_some() {
            return Err(TerminalReturnedLoanError::AdmissionAlreadyProvided);
        }
        if execution
            .program
            .machine(execution.current_machine)?
            .result
            .structural()
            .is_none_or(|signature| signature.reference_sources.is_empty())
        {
            return Err(TerminalReturnedLoanError::NoReferenceResult);
        }
        execution.entry_reference_backings = Some(EntryReferenceBackings::default());
        Ok(())
    }

    /// Borrow a declared, active reference leaf of this instance's actual normal
    /// result. Declaration-ID paths cannot select another instance or create a
    /// backing association. The handle cannot outlive the completed instance.
    pub fn returned_loan(
        &self,
        path: &[Path],
    ) -> Result<TerminalReturnedLoan<'_, 'artifact>, TerminalReturnedLoanError> {
        let execution = self
            .execution
            .as_ref()
            .ok_or(TerminalReturnedLoanError::Closed)?;
        if execution.entry_reference_backings.is_none() {
            return Err(TerminalReturnedLoanError::AdmissionRequired);
        }
        let result = match execution.result.as_ref() {
            Some(crate::TerminalExecutionResult::Structural(result)) => result,
            // A compact scalar case has no resident reference leaves. Its
            // active payload cannot select a declared sibling's reference.
            Some(crate::TerminalExecutionResult::ScalarCase(_)) => {
                return Err(TerminalReturnedLoanError::UnknownPath);
            }
            Some(_) => return Err(TerminalReturnedLoanError::NoReferenceResult),
            None => return Err(TerminalReturnedLoanError::NormalReturnRequired),
        };
        let signature = execution
            .program
            .machine(execution.current_machine)?
            .result
            .structural()
            .ok_or(TerminalReturnedLoanError::NoReferenceResult)?;
        let types = execution.structural_types();
        let tip = terminal_semantics::canonical_structural_path_tip(
            types.values(),
            signature.structural_type,
            path,
        )
        .ok_or(TerminalReturnedLoanError::UnknownPath)?;
        let StructuralTypeShape::Reference { referent, access } = tip.shape else {
            return Err(TerminalReturnedLoanError::NoReferenceResult);
        };
        let storage_path = terminal_semantics::canonical_structural_storage_path(
            types.values(),
            signature.structural_type,
            path,
        )
        .ok_or(TerminalReturnedLoanError::UnknownPath)?;
        if !signature.reference_sources.iter().any(|source| source.path == storage_path)
            || !crate::boundary::structural_inputs::case_membership::case_projection_in_value_is_active(
                &types, &result.value, execution.structural_cases.as_map(), &storage_path,
            )? {
            return Err(TerminalReturnedLoanError::UnknownPath);
        }
        Ok(TerminalReturnedLoan {
            execution,
            carrier: &result.value,
            source: signature.place,
            path: path.to_vec(),
            referent,
            access,
        })
    }
}

impl TerminalReturnedLoan<'_, '_> {
    pub fn access(&self) -> StructuralAccess {
        self.access
    }
    pub fn structural_type(&self) -> StructuralTypeId {
        self.referent
    }

    /// Observe one actual scalar through the retained descriptor and ordinary
    /// typed storage reader. Cases, bounds, re-view codecs, backing liveness and
    /// the reference's readable access remain ordinary runtime obligations.
    pub fn read_scalar(
        &self,
        path: &[Path],
    ) -> Result<crate::TerminalScalarValue, TerminalReturnedLoanError> {
        if !matches!(
            self.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) {
            return Err(TerminalReturnedLoanError::UnreadableAccess);
        }
        let mut complete = self.path.clone();
        complete.push(Path::Referent);
        complete.extend_from_slice(path);
        let types = self.execution.structural_types();
        let storage_path = terminal_semantics::canonical_structural_storage_path(
            types.values(),
            self.carrier.structural_type,
            &complete,
        )
        .ok_or(TerminalReturnedLoanError::UnknownPath)?;
        let expected = terminal_semantics::primitive_read_projection_type(
            types.values(),
            self.carrier.structural_type,
            &storage_path,
        )
        .ok_or(TerminalReturnedLoanError::UnknownPath)?;
        Ok(self.execution.read_retained_reference_scalar(
            self.source,
            self.carrier,
            &storage_path,
            expected,
        )?)
    }
}
