//! A source location identifies the park instruction, not a visit to it. Two
//! activations of the same program can park there at the same time.

use crate::{TerminalEffectWait, TerminalInterpretError};
use std::num::NonZeroU64;

/// Observing this occurrence does not establish a reply or provider readiness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSuspension {
    pub site: TerminalEffectWait,
    pub(crate) occurrence: ParkOccurrence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParkOccurrence {
    namespace: u64,
    visit: NonZeroU64,
}

impl ParkOccurrence {
    pub(crate) fn fresh(
        namespace: Option<u64>,
        last: &mut u64,
    ) -> Result<Self, TerminalInterpretError> {
        // Reuse the execution's namespace. Stepping independent producers
        // needs neither a process-wide counter per item nor a table of visits.
        let namespace = namespace.ok_or(TerminalInterpretError::StructuralIdentityExhausted)?;
        let visit = last
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .ok_or(TerminalInterpretError::StructuralIdentityExhausted)?;
        *last = visit.get();
        Ok(Self { namespace, visit })
    }
}
