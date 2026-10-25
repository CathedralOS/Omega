//! One packed scalar payload owns its logical element reservation. Guest copies
//! allocate independent storage; inspection aliases retain the same immutable
//! backing. Mutation detaches a shared backing only after funding the overlap.

use crate::errors::{TerminalInterpretError, malformed};
use crate::values::TerminalScalarValue;
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};

pub(super) struct ScalarArrayStorage {
    elements: Vec<TerminalScalarValue>,
    // Retire the payload before releasing its logical cells.
    _lease: Option<EvaluationStorageLease>,
    account: Option<EvaluationInvocationAccount>,
}

impl ScalarArrayStorage {
    pub(super) fn collect(
        count: u64,
        values: impl IntoIterator<Item = Result<TerminalScalarValue, TerminalInterpretError>>,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<Self, TerminalInterpretError> {
        let lease = account
            .map(|account| account.reserve_storage(count, 0))
            .transpose()
            .map_err(TerminalInterpretError::Storage)?;
        let length = usize::try_from(count).map_err(|_| malformed())?;
        let requested_bytes = length
            .checked_mul(std::mem::size_of::<TerminalScalarValue>())
            .ok_or_else(malformed)?;
        let mut elements = Vec::new();
        elements.try_reserve_exact(length).map_err(|reason| {
            TerminalInterpretError::StorageAllocationFailed {
                requested_bytes,
                reason: reason.to_string(),
            }
        })?;
        let mut values = values.into_iter();
        for _ in 0..length {
            elements.push(values.next().ok_or_else(malformed)??);
        }
        if values.next().is_some() {
            return Err(malformed());
        }
        Ok(Self {
            elements,
            _lease: lease,
            account: account.cloned(),
        })
    }

    pub(super) fn copy(&self) -> Result<Self, TerminalInterpretError> {
        Self::collect(
            u64::try_from(self.elements.len()).map_err(|_| malformed())?,
            self.elements.iter().copied().map(Ok),
            self.account.as_ref(),
        )
    }

    pub(super) fn elements(&self) -> &[TerminalScalarValue] {
        &self.elements
    }

    pub(super) fn elements_mut(&mut self) -> &mut [TerminalScalarValue] {
        &mut self.elements
    }
}

// Sponsor identity and mutable counters are not part of value meaning or debug
// output. Distinct owned snapshots compare by their exact scalar contents.
impl PartialEq for ScalarArrayStorage {
    fn eq(&self, other: &Self) -> bool {
        self.elements == other.elements
    }
}
impl Eq for ScalarArrayStorage {}
impl std::fmt::Debug for ScalarArrayStorage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.elements.fmt(formatter)
    }
}
