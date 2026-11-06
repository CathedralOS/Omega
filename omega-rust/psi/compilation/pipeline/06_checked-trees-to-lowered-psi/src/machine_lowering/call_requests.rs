//! Find each emitted call once, then attach every requested safety check to it.
//! Sparse operation identities remain keys, never allocation sizes. Repeated or
//! missing identities cannot supply a unique call.

use checked_trees::{CheckedTrees, CheckedValueOrigin};
use lowered_psi::LoweredPsi;
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};
use symbols::SymbolHandle;
use terminal_psi::{Operation, OperationKind};

pub(super) fn matching_requests<Key: Copy + Ord>(
    requests: &[(Key, usize)],
    key: Key,
) -> &[(Key, usize)] {
    let first = requests.partition_point(|(found, _)| *found < key);
    let after = requests.partition_point(|(found, _)| *found <= key);
    &requests[first..after]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct StatementCallKey {
    state_index: u32,
    state_generation: u32,
    statement_index: usize,
    call_ordinal: usize,
}

impl StatementCallKey {
    pub(super) fn new(state: SymbolHandle, statement_index: usize, call_ordinal: usize) -> Self {
        Self {
            state_index: state.arena_index(),
            state_generation: state.generation(),
            statement_index,
            call_ordinal,
        }
    }

    pub(super) fn from_checked(
        checked: &CheckedTrees,
        origin: CheckedValueOrigin,
        call_ordinal: usize,
    ) -> Option<Self> {
        let CheckedValueOrigin::StateStatement {
            state_symbol,
            statement_index,
            ..
        } = checked.consistent_terminal_origin(origin)?
        else {
            return None;
        };
        Some(Self::new(state_symbol, statement_index, call_ordinal))
    }
}

struct EmittedOperation {
    operation: OperationId,
    machine: MachineId,
    boundary: Option<BoundaryMachineId>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum EmittedCallLookupError {
    MissingOrAmbiguousOperation,
    NotBoundaryCall,
}

pub(super) struct EmittedCallCatalog {
    operations: Vec<EmittedOperation>,
    states: Vec<(u32, u32)>,
}

impl EmittedCallCatalog {
    pub(super) fn for_checked_requests(checked: &CheckedTrees, lowered: &LoweredPsi) -> Self {
        let control = &checked.facts.flow.control;
        let has_requests = control.states.iter().any(|(_, state)| {
            control.calls.span_or_empty(state.calls).iter().any(|call| {
                !call.lender_preservation.is_empty() || !call.reference_forwarding.is_empty()
            })
        });
        if !has_requests {
            return Self {
                operations: Vec::new(),
                states: Vec::new(),
            };
        }
        let mut catalog =
            Self::from_operations(lowered.semantic_module.machines.iter().flat_map(|machine| {
                machine
                    .operations()
                    .map(move |operation| (machine.id, operation))
            }));
        catalog.states = lowered
            .state_blocks
            .iter()
            .map(|block| (block.state.arena_index(), block.state.generation()))
            .collect();
        catalog.states.sort_unstable();
        catalog.states.dedup();
        catalog
    }

    fn from_operations<'operation>(
        operations: impl Iterator<Item = (MachineId, &'operation Operation)>,
    ) -> Self {
        let mut operations = operations
            .map(|(machine, operation)| EmittedOperation {
                operation: operation.id,
                machine,
                boundary: match operation.kind {
                    OperationKind::BoundaryCall { boundary, .. } => Some(boundary),
                    _ => None,
                },
            })
            .collect::<Vec<_>>();
        operations.sort_unstable_by_key(|row| row.operation);
        Self {
            operations,
            states: Vec::new(),
        }
    }

    pub(super) fn boundary_call(
        &self,
        operation: OperationId,
    ) -> Result<(MachineId, BoundaryMachineId), EmittedCallLookupError> {
        let position = self
            .operations
            .partition_point(|row| row.operation < operation);
        let row = self
            .operations
            .get(position)
            .filter(|row| row.operation == operation)
            .ok_or(EmittedCallLookupError::MissingOrAmbiguousOperation)?;
        if self
            .operations
            .get(position + 1)
            .is_some_and(|next| next.operation == operation)
        {
            return Err(EmittedCallLookupError::MissingOrAmbiguousOperation);
        }
        row.boundary
            .map(|boundary| (row.machine, boundary))
            .ok_or(EmittedCallLookupError::NotBoundaryCall)
    }

    pub(super) fn state_was_emitted(&self, call: StatementCallKey) -> bool {
        self.states
            .binary_search(&(call.state_index, call.state_generation))
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use terminal_psi::OperationResult;

    fn boundary_call(operation: u64, boundary: u64) -> Operation {
        Operation::unsuspended(
            OperationId::new(operation).expect("nonzero operation"),
            OperationResult::Unit,
            OperationKind::BoundaryCall {
                boundary: BoundaryMachineId::new(boundary).expect("nonzero boundary"),
                arguments: Vec::new(),
                structural_arguments: Vec::new(),
                completion_receipts: Vec::new(),
                requirement_obligations: Vec::new(),
            },
        )
    }

    #[test]
    fn one_operation_walk_serves_repeated_sparse_call_lookups() {
        let machine = MachineId::new(7).expect("nonzero machine");
        let operations = [boundary_call(u64::MAX, 11), boundary_call(1, 13)];
        let visits = Cell::new(0);
        let catalog = EmittedCallCatalog::from_operations(operations.iter().map(|operation| {
            visits.set(visits.get() + 1);
            (machine, operation)
        }));
        for _ in 0..1_000 {
            assert_eq!(
                catalog.boundary_call(operations[0].id),
                Ok((
                    machine,
                    BoundaryMachineId::new(11).expect("nonzero boundary")
                ))
            );
        }
        assert_eq!(visits.get(), 2);
    }

    #[test]
    fn missing_duplicate_and_noncall_operations_cannot_supply_a_call() {
        let machine = MachineId::new(1).expect("nonzero machine");
        let first = boundary_call(7, 11);
        let repeated = boundary_call(7, 11);
        let noncall = Operation::unsuspended(
            OperationId::new(9).expect("nonzero operation"),
            OperationResult::Unit,
            OperationKind::BooleanConstant { value: false },
        );
        let catalog = EmittedCallCatalog::from_operations(
            [&first, &repeated, &noncall]
                .into_iter()
                .map(|operation| (machine, operation)),
        );
        for operation in [7, 8] {
            assert_eq!(
                catalog.boundary_call(OperationId::new(operation).expect("nonzero operation")),
                Err(EmittedCallLookupError::MissingOrAmbiguousOperation)
            );
        }
        assert_eq!(
            catalog.boundary_call(noncall.id),
            Err(EmittedCallLookupError::NotBoundaryCall)
        );
    }

    #[test]
    fn checked_call_keys_keep_state_generation_and_require_resolved_origins() {
        let checked = CheckedTrees::default();
        let state = SymbolHandle::from_parts(3, 7);
        let key = StatementCallKey::from_checked(
            &checked,
            CheckedValueOrigin::StateStatement {
                machine_symbol: SymbolHandle::from_arena_index(1),
                state_symbol: state,
                statement_index: 2,
                role: checked_trees::CheckedValueStatementRole::Expression,
            },
            4,
        );
        assert_eq!(key, Some(StatementCallKey::new(state, 2, 4)));
        assert_ne!(
            key,
            Some(StatementCallKey::new(SymbolHandle::from_parts(3, 8), 2, 4))
        );
        assert!(
            StatementCallKey::from_checked(
                &checked,
                CheckedValueOrigin::NestedExpression {
                    parent: typed_trees::expression::ExpressionHandle::from_arena_index(1),
                },
                4,
            )
            .is_none()
        );
    }
}
