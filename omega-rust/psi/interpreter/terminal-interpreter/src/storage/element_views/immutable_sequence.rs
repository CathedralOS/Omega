//! Runtime-sized immutable collections keep real typed element homes. A view
//! owns this sequence and its reservation; copies share the same homes instead
//! of inventing a static array type or retargeting an earlier descriptor.

use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::boundary::values::TerminalStructuralValue;
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};

pub(crate) struct ImmutableStructuralSequence {
    elements: Vec<TerminalStructuralValue>,
    _lease: Option<EvaluationStorageLease>,
}

impl ImmutableStructuralSequence {
    pub(crate) fn from_values(
        length: usize,
        values: impl IntoIterator<Item = Result<TerminalStructuralValue, TerminalInterpretError>>,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<Self, TerminalInterpretError> {
        let bytes = length
            .checked_mul(std::mem::size_of::<TerminalStructuralValue>())
            .ok_or_else(malformed)?;
        let payload_bytes = u64::try_from(bytes).map_err(|_| malformed())?;
        let lease = account
            .map(|account| account.reserve_storage(0, payload_bytes))
            .transpose()
            .map_err(TerminalInterpretError::Storage)?;
        let mut elements = Vec::new();
        elements.try_reserve_exact(length).map_err(|reason| {
            TerminalInterpretError::StorageAllocationFailed {
                requested_bytes: bytes,
                reason: reason.to_string(),
            }
        })?;
        let mut values = values.into_iter();
        for _ in 0..length {
            let value = values.next().ok_or_else(malformed)??;
            // Sequence entries own complete homes; a projected or qualified
            // alias needs its original owner rather than this snapshot route.
            if !value.path.is_empty()
                || !value.qualifications.is_empty()
                || !value.projected_qualifications.is_empty()
            {
                return Err(malformed());
            }
            elements.push(value);
        }
        if values.next().is_some() {
            return Err(malformed());
        }
        Ok(Self {
            elements,
            _lease: lease,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.elements.len()
    }

    pub(crate) fn element(&self, position: usize) -> Option<&TerminalStructuralValue> {
        self.elements.get(position)
    }
}
