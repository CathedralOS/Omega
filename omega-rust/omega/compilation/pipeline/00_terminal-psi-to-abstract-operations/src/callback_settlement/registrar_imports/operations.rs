//! Scan Terminal operations once for the requested callback identities.
//! Preserve match counts so settlement reports ambiguity in authored order.

use semantic_vocabulary::OperationId;
use terminal_psi::Operation;

struct RequestedCallbackOperation<'terminal> {
    identity: OperationId,
    first_match: Option<&'terminal Operation>,
    match_count: usize,
}

pub(super) struct CallbackTerminalOperations<'terminal> {
    requested: Vec<RequestedCallbackOperation<'terminal>>,
}

impl<'terminal> CallbackTerminalOperations<'terminal> {
    pub(super) fn new(
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

    pub(super) fn find(&self, operation: OperationId) -> Result<&'terminal Operation, usize> {
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

#[cfg(test)]
mod tests {
    use super::CallbackTerminalOperations;
    use semantic_vocabulary::{BoundaryMachineId, OperationId};
    use terminal_psi::{Operation, OperationKind, OperationResult};

    fn operation(identity: u64) -> Operation {
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(identity).expect("nonzero operation identity"),
            result: OperationResult::Unit,
            kind: OperationKind::BoundaryCall {
                requirement_obligations: Vec::new(),
                boundary: BoundaryMachineId::new(1).expect("nonzero boundary identity"),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
            },
        }
    }

    #[test]
    fn sparse_requested_operations_retain_exact_match_counts() {
        let operations = [operation(1), operation(9), operation(17), operation(9)];
        let missing = OperationId::new(u64::MAX).expect("sparse operation identity");
        let requested = [
            operations[2].id,
            operations[1].id,
            operations[2].id,
            missing,
        ];
        let joined = CallbackTerminalOperations::new(requested.into_iter(), operations.iter());

        assert!(std::ptr::eq(
            joined.find(operations[2].id).expect("one exact operation"),
            &operations[2]
        ));
        assert_eq!(joined.find(operations[1].id), Err(2));
        assert_eq!(joined.find(missing), Err(0));
        assert_eq!(joined.find(operations[0].id), Err(0));
    }
}
