//! Request-sized joins for callback custody. Scan source and Terminal inputs
//! once, retaining exact match counts so callers report errors in authored order.

use checked_trees::NominalMachineUseSite;
use lowered_psi::LoweredSourceCallOccurrence;
use semantic_vocabulary::OperationId;
use symbols::SymbolHandle;
use terminal_psi::Operation;

#[derive(Clone, Copy)]
struct OccurrenceMatches<T> {
    first: Option<T>,
    count: usize,
}

impl<T: Copy> OccurrenceMatches<T> {
    fn empty() -> Self {
        Self {
            first: None,
            count: 0,
        }
    }

    fn record(&mut self, occurrence: T) {
        self.first.get_or_insert(occurrence);
        self.count += 1;
    }

    fn unique(&self) -> Result<T, usize> {
        match (self.count, self.first) {
            (1, Some(occurrence)) => Ok(occurrence),
            _ => Err(self.count),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct SourceCallKey {
    // Variant and complete generational identity; equal arena indices alone
    // must not join an expression to a statement or resurrect a stale handle.
    site: (u8, u32, u32),
    target: (u32, u32),
}

impl SourceCallKey {
    fn new(site: NominalMachineUseSite, target: SymbolHandle) -> Self {
        let site = match site {
            NominalMachineUseSite::Statement(handle) => {
                (0, handle.arena_index(), handle.generation())
            }
            NominalMachineUseSite::Expression(handle) => {
                (1, handle.arena_index(), handle.generation())
            }
        };
        Self {
            site,
            target: (target.arena_index(), target.generation()),
        }
    }
}

pub(super) struct CallbackSourceCalls<'source> {
    requested: Vec<(
        SourceCallKey,
        OccurrenceMatches<&'source LoweredSourceCallOccurrence>,
    )>,
}

impl<'source> CallbackSourceCalls<'source> {
    pub(super) fn new(
        placements: impl Iterator<Item = (NominalMachineUseSite, SymbolHandle)>,
        source_calls: &'source [LoweredSourceCallOccurrence],
    ) -> Self {
        let mut requested = placements
            .map(|(site, target)| (SourceCallKey::new(site, target), OccurrenceMatches::empty()))
            .collect::<Vec<_>>();
        requested.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        requested.dedup_by(|left, right| left.0 == right.0);
        if !requested.is_empty() {
            for occurrence in source_calls {
                let Some(site) = occurrence.source_site else {
                    continue;
                };
                let key = SourceCallKey::new(site, occurrence.source_target);
                if let Ok(position) = requested.binary_search_by(|entry| entry.0.cmp(&key)) {
                    requested[position].1.record(occurrence);
                }
            }
        }
        Self { requested }
    }

    pub(super) fn find(
        &self,
        site: NominalMachineUseSite,
        target: SymbolHandle,
    ) -> Result<&'source LoweredSourceCallOccurrence, usize> {
        let key = SourceCallKey::new(site, target);
        self.requested
            .binary_search_by(|entry| entry.0.cmp(&key))
            .map_err(|_| 0usize)
            .and_then(|position| self.requested[position].1.unique())
    }
}

pub(super) struct CallbackTerminalOperations<'terminal> {
    requested: Vec<(OperationId, OccurrenceMatches<&'terminal Operation>)>,
}

impl<'terminal> CallbackTerminalOperations<'terminal> {
    pub(super) fn new(
        operation_ids: impl Iterator<Item = OperationId>,
        operations: impl Iterator<Item = &'terminal Operation>,
    ) -> Self {
        let mut requested = operation_ids
            .map(|operation| (operation, OccurrenceMatches::empty()))
            .collect::<Vec<_>>();
        requested.sort_unstable_by_key(|entry| entry.0);
        requested.dedup_by_key(|entry| entry.0);
        if !requested.is_empty() {
            for operation in operations {
                if let Ok(position) = requested.binary_search_by_key(&operation.id, |entry| entry.0)
                {
                    requested[position].1.record(operation);
                }
            }
        }
        Self { requested }
    }

    pub(super) fn find(&self, operation: OperationId) -> Result<&'terminal Operation, usize> {
        self.requested
            .binary_search_by_key(&operation, |entry| entry.0)
            .map_err(|_| 0usize)
            .and_then(|position| self.requested[position].1.unique())
    }
}

#[cfg(test)]
mod tests;
