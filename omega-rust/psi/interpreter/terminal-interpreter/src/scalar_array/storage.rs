//! One packed scalar payload owns its logical element reservation. Guest copies
//! allocate independent local storage; inspection aliases retain the same cells.
//! Mutation funds a detached container while preserving resident endpoints.

use crate::errors::{TerminalInterpretError, malformed};
use crate::scalar_storage::ScalarCell;
use crate::values::TerminalScalarValue;
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};

#[derive(compiler_storage::Record)]
pub(super) struct ScalarArrayStorage<Element> {
    elements: Vec<Element>,
    // Retire the payload before releasing its logical cells.
    // A stored receipt retains value/usage, never a prior invocation reservation.
    #[record(skip)]
    _lease: Option<EvaluationStorageLease>,
    #[record(skip)]
    account: Option<EvaluationInvocationAccount>,
}

impl<Element> ScalarArrayStorage<Element> {
    pub(super) fn collect(
        count: u64,
        values: impl IntoIterator<Item = Result<Element, TerminalInterpretError>>,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<Self, TerminalInterpretError> {
        let lease = account
            .map(|account| account.reserve_storage(count, 0))
            .transpose()
            .map_err(TerminalInterpretError::Storage)?;
        let length = usize::try_from(count).map_err(|_| malformed())?;
        let requested_bytes = length
            .checked_mul(std::mem::size_of::<Element>())
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

    pub(super) fn copy(&self) -> Result<Self, TerminalInterpretError>
    where
        Element: Clone,
    {
        Self::collect(
            u64::try_from(self.elements.len()).map_err(|_| malformed())?,
            self.elements.iter().cloned().map(Ok),
            self.account.as_ref(),
        )
    }

    pub(super) fn elements(&self) -> &[Element] {
        &self.elements
    }

    pub(super) fn elements_mut(&mut self) -> &mut [Element] {
        &mut self.elements
    }
}

impl ScalarArrayStorage<TerminalScalarValue> {
    /// Convert unique owned payload without reserving its logical cells again.
    pub(super) fn into_cells(
        self,
    ) -> Result<ScalarArrayStorage<ScalarCell>, TerminalInterpretError> {
        let requested_bytes = self
            .elements
            .len()
            .checked_mul(std::mem::size_of::<ScalarCell>())
            .ok_or_else(malformed)?;
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(self.elements.len())
            .map_err(|reason| TerminalInterpretError::StorageAllocationFailed {
                requested_bytes,
                reason: reason.to_string(),
            })?;
        for value in self.elements {
            cells.push(ScalarCell::Local(value));
        }
        Ok(ScalarArrayStorage {
            elements: cells,
            _lease: self._lease,
            account: self.account,
        })
    }
}

impl ScalarArrayStorage<ScalarCell> {
    pub(super) fn observe(
        &self,
    ) -> Result<ScalarArrayStorage<TerminalScalarValue>, TerminalInterpretError> {
        ScalarArrayStorage::collect(
            u64::try_from(self.elements.len()).map_err(|_| malformed())?,
            self.elements.iter().map(ScalarCell::read),
            self.account.as_ref(),
        )
    }

    pub(super) fn copy_for_mutation(&self) -> Result<Self, TerminalInterpretError> {
        self.copy()
    }

    pub(super) fn cells(&self) -> &[ScalarCell] {
        self.elements()
    }
    pub(super) fn cells_mut(&mut self) -> &mut [ScalarCell] {
        self.elements_mut()
    }
}

// Sponsor identity and mutable counters are not part of value meaning or debug
// output. Locals compare by contents; residents by exact endpoint identity.
impl<Element: PartialEq> PartialEq for ScalarArrayStorage<Element> {
    fn eq(&self, other: &Self) -> bool {
        self.elements == other.elements
    }
}
impl<Element: Eq> Eq for ScalarArrayStorage<Element> {}
impl<Element: std::fmt::Debug> std::fmt::Debug for ScalarArrayStorage<Element> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.elements.fmt(formatter)
    }
}
