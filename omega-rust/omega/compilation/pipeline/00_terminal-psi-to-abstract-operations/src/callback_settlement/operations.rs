//! Scan Terminal operations once for the requested callback identities.
//! Preserve match counts so settlement reports ambiguity in authored order.

use semantic_vocabulary::OperationId;
use terminal_psi::Operation;

struct RequestedCallbackOperation<'terminal> {
    identity: OperationId,
    first_match: Option<&'terminal Operation>,
    match_count: usize,
}

pub(crate) struct CallbackTerminalOperations<'terminal> {
    requested: Vec<RequestedCallbackOperation<'terminal>>,
}

impl<'terminal> CallbackTerminalOperations<'terminal> {
    pub(crate) fn new(
        operation_ids: impl Iterator<Item = OperationId>,
        operations: impl Iterator<Item = &'terminal Operation>,
    ) -> Self {
        let mut requested = operation_ids
            .map(|identity| RequestedCallbackOperation {
                identity,
                first_match: None,
                match_count: 0,
            })
            .collect::<Vec<_>>();
        requested.sort_unstable_by_key(|entry| entry.identity);
        requested.dedup_by_key(|entry| entry.identity);
        if !requested.is_empty() {
            for operation in operations {
                if let Ok(position) =
                    requested.binary_search_by_key(&operation.id, |entry| entry.identity)
                {
                    let entry = &mut requested[position];
                    entry.first_match.get_or_insert(operation);
                    entry.match_count += 1;
                }
            }
        }
        Self { requested }
    }

    pub(crate) fn find(&self, operation: OperationId) -> Result<&'terminal Operation, usize> {
        let position = self
            .requested
            .binary_search_by_key(&operation, |entry| entry.identity)
            .map_err(|_| 0usize)?;
        let entry = &self.requested[position];
        match (entry.match_count, entry.first_match) {
            (1, Some(operation)) => Ok(operation),
            (match_count, _) => Err(match_count),
        }
    }
}
