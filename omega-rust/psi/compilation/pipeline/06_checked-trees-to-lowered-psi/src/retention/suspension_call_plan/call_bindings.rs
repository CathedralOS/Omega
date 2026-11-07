//! Checked call coordinates can have several real emitted instances. Keep each
//! instance's operation and operands; an older capture may select only the
//! unique invocation in the current emitted caller, never another caller's copy.

use super::{
    CheckedTrees, LoweredSourceCallOccurrence, LoweringError, TerminalModule,
    TerminalSuspensionPlace, TerminalSuspensionValueType, call_argument_binding,
    owning_callable_symbol, terminal_call_target,
};
use crate::shared::lowering_error::{SuspensionCallBindingFailure, SuspensionCallBindingMismatch};
use lowered_psi::LoweredStatementCallCoordinate;
use semantic_vocabulary::{MachineId, OperationId};
use std::ops::Range;

#[derive(Clone, Copy)]
struct OperationLocation {
    operation: OperationId,
    caller: MachineId,
    location: (usize, usize, usize),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SourceCoordinateKey {
    state_index: u32,
    state_generation: u32,
    statement_index: usize,
    call_ordinal: usize,
}

impl From<LoweredStatementCallCoordinate> for SourceCoordinateKey {
    fn from(site: LoweredStatementCallCoordinate) -> Self {
        Self {
            state_index: site.state.arena_index(),
            state_generation: site.state.generation(),
            statement_index: site.statement_index,
            call_ordinal: site.call_ordinal,
        }
    }
}

pub(super) struct SourceCallBindings<'calls> {
    source_calls: &'calls [LoweredSourceCallOccurrence],
    coordinates: Vec<(SourceCoordinateKey, usize)>,
    caller_coordinates: Vec<(MachineId, SourceCoordinateKey, usize)>,
    unlocated_coordinates: Vec<SourceCoordinateKey>,
    duplicate_occurrences: Vec<bool>,
    operations: Vec<OperationLocation>,
    bound: Vec<Option<usize>>,
    pub(super) calls: Vec<SourceCallBinding>,
}

pub(super) struct SourceCallBinding {
    pub(super) occurrence_index: usize,
    pub(super) caller: MachineId,
    pub(super) location: (usize, usize, usize),
    pub(super) operation: OperationId,
    pub(super) target: terminal_psi::TerminalSuspensionCallTarget,
    arguments: Vec<(
        symbols::SymbolHandle,
        usize,
        (TerminalSuspensionPlace, TerminalSuspensionValueType),
    )>,
}

impl<'calls> SourceCallBindings<'calls> {
    /// Prepare sparse source and emitted-operation rows once. The enclosing
    /// pass changes only suspension markers, so these exact locations stay
    /// bound to the same module operation roster for every current/old capture.
    pub(super) fn new(
        source_calls: &'calls [LoweredSourceCallOccurrence],
        module: &TerminalModule,
    ) -> Self {
        let mut coordinates = source_calls
            .iter()
            .enumerate()
            .filter_map(|(index, call)| {
                call.origin
                    .statement()
                    .map(|site| (SourceCoordinateKey::from(site), index))
            })
            .collect::<Vec<_>>();
        coordinates.sort_unstable();
        let mut source_operations = coordinates
            .iter()
            .filter_map(|(coordinate, index)| {
                source_calls[*index]
                    .terminal_site
                    .operation()
                    .map(|operation| (operation, *coordinate, *index))
            })
            .collect::<Vec<_>>();
        source_operations.sort_unstable();
        let mut duplicate_occurrences = vec![false; source_calls.len()];
        for pair in source_operations.windows(2) {
            if (pair[0].0, pair[0].1) == (pair[1].0, pair[1].1) {
                duplicate_occurrences[pair[0].2] = true;
                duplicate_occurrences[pair[1].2] = true;
            }
        }
        let mut operations = Vec::new();
        for (machine_index, machine) in module.machines.iter().enumerate() {
            for (block_index, block) in machine.blocks.iter().enumerate() {
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    if source_operations
                        .binary_search_by_key(&operation.id, |row| row.0)
                        .is_ok()
                    {
                        operations.push(OperationLocation {
                            operation: operation.id,
                            caller: machine.id,
                            location: (machine_index, block_index, operation_index),
                        });
                    }
                }
            }
        }
        operations.sort_by_key(|row| row.operation);
        let mut caller_coordinates = Vec::new();
        let mut unlocated_coordinates = Vec::new();
        for (coordinate, index) in &coordinates {
            let location = source_calls[*index]
                .terminal_site
                .operation()
                .and_then(|id| {
                    let first = operations.partition_point(|row| row.operation < id);
                    let last = operations.partition_point(|row| row.operation <= id);
                    match &operations[first..last] {
                        [location] => Some(*location),
                        _ => None,
                    }
                });
            if let Some(location) = location {
                caller_coordinates.push((location.caller, *coordinate, *index));
            } else {
                // A missing/ambiguous physical owner cannot be silently omitted
                // when selecting an older capture in a particular caller.
                unlocated_coordinates.push(*coordinate);
            }
        }
        caller_coordinates.sort_unstable();
        unlocated_coordinates.sort_unstable();
        unlocated_coordinates.dedup();
        Self {
            source_calls,
            coordinates,
            caller_coordinates,
            unlocated_coordinates,
            duplicate_occurrences,
            operations,
            bound: vec![None; source_calls.len()],
            calls: Vec::new(),
        }
    }

    pub(super) fn matching(
        &self,
        state: symbols::SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Range<usize> {
        let coordinate = SourceCoordinateKey::from(LoweredStatementCallCoordinate {
            state,
            statement_index,
            call_ordinal,
        });
        let first = self
            .coordinates
            .partition_point(|(site, _)| *site < coordinate);
        let last = self
            .coordinates
            .partition_point(|(site, _)| *site <= coordinate);
        first..last
    }

    pub(super) fn occurrence_index(&self, ordinal: usize) -> usize {
        self.coordinates[ordinal].1
    }

    pub(super) fn missing_occurrence(
        state: symbols::SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
    ) -> LoweringError {
        binding_mismatch(
            Some(LoweredStatementCallCoordinate {
                state,
                statement_index,
                call_ordinal,
            }),
            None,
            None,
            None,
            SuspensionCallBindingFailure::MissingOccurrence,
        )
    }

    /// Every published current invocation receives its own plan. Repeating the
    /// same source coordinate at the SAME physical invocation is still invalid.
    pub(super) fn bind_occurrence(
        &mut self,
        checked: &CheckedTrees,
        module: &TerminalModule,
        occurrence_index: usize,
        target: symbols::SymbolHandle,
    ) -> Result<usize, LoweringError> {
        let occurrence = self.source_calls.get(occurrence_index).ok_or_else(|| {
            binding_mismatch(
                None,
                Some(occurrence_index),
                None,
                None,
                SuspensionCallBindingFailure::MissingOccurrence,
            )
        })?;
        let mismatch = |failure| {
            let operation = occurrence.terminal_site.operation();
            let caller = operation.and_then(|operation| {
                let first = self
                    .operations
                    .partition_point(|row| row.operation < operation);
                let last = self
                    .operations
                    .partition_point(|row| row.operation <= operation);
                match &self.operations[first..last] {
                    [owner] => Some(owner.caller),
                    _ => None,
                }
            });
            binding_mismatch(
                occurrence.origin.statement(),
                Some(occurrence_index),
                caller,
                operation,
                failure,
            )
        };
        if owning_callable_symbol(checked, occurrence.source_target)
            != owning_callable_symbol(checked, target)
        {
            return Err(mismatch(SuspensionCallBindingFailure::Redirected {
                recorded: occurrence.source_target,
                requested: target,
            }));
        }
        if let Some(binding) = self.bound[occurrence_index] {
            return Ok(binding);
        }
        if occurrence.origin.statement().is_none() {
            return Err(mismatch(
                SuspensionCallBindingFailure::MissingStatementCoordinate,
            ));
        }
        let operation = occurrence
            .terminal_site
            .operation()
            .ok_or_else(|| mismatch(SuspensionCallBindingFailure::NonContinuingSite))?;
        if self.duplicate_occurrences[occurrence_index] {
            return Err(mismatch(SuspensionCallBindingFailure::RepeatedInvocation));
        }
        let first = self
            .operations
            .partition_point(|row| row.operation < operation);
        let last = self
            .operations
            .partition_point(|row| row.operation <= operation);
        let [location] = &self.operations[first..last] else {
            return Err(mismatch(
                SuspensionCallBindingFailure::OperationOwnerCount {
                    actual: last - first,
                },
            ));
        };
        let (machine, block, index) = location.location;
        let emitted = &module.machines[machine].blocks[block].operations[index];
        let target = terminal_call_target(&emitted.kind).ok_or_else(|| {
            binding_mismatch(
                occurrence.origin.statement(),
                Some(occurrence_index),
                Some(location.caller),
                Some(operation),
                SuspensionCallBindingFailure::NonCallOperation,
            )
        })?;
        let position = self.calls.len();
        self.calls.push(SourceCallBinding {
            occurrence_index,
            caller: location.caller,
            location: location.location,
            operation,
            target,
            arguments: Vec::new(),
        });
        self.bound[occurrence_index] = Some(position);
        Ok(position)
    }

    /// A completed old input/output belongs to the current emitted caller.
    /// Several remaining copies INSIDE that caller are unavailable: neither
    /// dominance guesses nor equal roots select its captured invocation.
    pub(super) fn bind_previous(
        &mut self,
        checked: &CheckedTrees,
        module: &TerminalModule,
        caller: MachineId,
        state: symbols::SymbolHandle,
        statement_index: usize,
        call_ordinal: usize,
        target: symbols::SymbolHandle,
    ) -> Result<usize, LoweringError> {
        let coordinate = SourceCoordinateKey::from(LoweredStatementCallCoordinate {
            state,
            statement_index,
            call_ordinal,
        });
        let mismatch = |failure| {
            binding_mismatch(
                Some(LoweredStatementCallCoordinate {
                    state,
                    statement_index,
                    call_ordinal,
                }),
                None,
                Some(caller),
                None,
                failure,
            )
        };
        if self
            .unlocated_coordinates
            .binary_search(&coordinate)
            .is_ok()
        {
            return Err(mismatch(
                SuspensionCallBindingFailure::UnavailableEmittedOwner,
            ));
        }
        let first = self
            .caller_coordinates
            .partition_point(|row| (row.0, row.1) < (caller, coordinate));
        let last = self
            .caller_coordinates
            .partition_point(|row| (row.0, row.1) <= (caller, coordinate));
        let occurrence = match &self.caller_coordinates[first..last] {
            [(_, _, occurrence)] => *occurrence,
            matches => {
                return Err(mismatch(
                    SuspensionCallBindingFailure::CallerOccurrenceCount {
                        actual: matches.len(),
                    },
                ));
            }
        };
        self.bind_occurrence(checked, module, occurrence, target)
    }
}

impl SourceCallBinding {
    /// Preserve the requested checked telescope and the actual invocation's
    /// emitted operand. Aliases of a callable do not share a formal cache key.
    pub(super) fn argument(
        &mut self,
        checked: &CheckedTrees,
        module: &TerminalModule,
        target: symbols::SymbolHandle,
        position: usize,
    ) -> Result<(TerminalSuspensionPlace, TerminalSuspensionValueType), LoweringError> {
        if let Some((_, _, binding)) = self
            .arguments
            .iter()
            .find(|(owner, formal, _)| *owner == target && *formal == position)
        {
            return Ok(binding.clone());
        }
        let (machine, block, index) = self.location;
        let operation = &module.machines[machine].blocks[block].operations[index];
        let binding = call_argument_binding(checked, target, &operation.kind, module, position)?;
        self.arguments.push((target, position, binding.clone()));
        Ok(binding)
    }
}

fn binding_mismatch(
    coordinate: Option<LoweredStatementCallCoordinate>,
    occurrence_index: Option<usize>,
    caller: Option<MachineId>,
    operation: Option<OperationId>,
    failure: SuspensionCallBindingFailure,
) -> LoweringError {
    LoweringError::SuspensionCallBindingMismatch(Box::new(SuspensionCallBindingMismatch {
        coordinate,
        occurrence_index,
        caller,
        operation,
        failure,
    }))
}
