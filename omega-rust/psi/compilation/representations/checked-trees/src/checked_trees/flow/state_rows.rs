//! Machine-local flow rows, in their original arena order.
//!
//! Consumers already know the owning machine, but repeatedly scanned every
//! machine's states to find its calls and statements. The table owns the
//! arena and its two groupings — by owning machine and by state symbol —
//! together; stable grouping retains duplicate rows and full symbol
//! generations, and the first-match and unique readers keep their distinct
//! rules. Mutable arena access clears the groupings, including replacement
//! by an unrelated arena with the same revision during product pruning.

use super::{FlowControlFacts, FlowStateFact};
use crate::checked_trees::keyed_rows::KeyedRows;
use arena::Handle;
use symbols::SymbolHandle;
type Arena<T> = arena::PagedArenaSet<T>;

/// Flow-state storage and its derived machine relationships share one owner.
/// Arena mutation is exposed for producers, but cannot retain a stale index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowStateTable {
    arena: Arena<FlowStateFact>,
    machine_rows: KeyedRows<SymbolHandle, FlowStateFact>,
    state_rows: KeyedRows<SymbolHandle, FlowStateFact>,
}

impl From<Arena<FlowStateFact>> for FlowStateTable {
    fn from(arena: Arena<FlowStateFact>) -> Self {
        Self {
            arena,
            machine_rows: KeyedRows::default(),
            state_rows: KeyedRows::default(),
        }
    }
}

impl std::ops::Deref for FlowStateTable {
    type Target = Arena<FlowStateFact>;

    fn deref(&self) -> &Self::Target {
        &self.arena
    }
}

impl std::ops::DerefMut for FlowStateTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.machine_rows.clear();
        self.state_rows.clear();
        &mut self.arena
    }
}

impl FlowControlFacts {
    fn with_machine_rows<Result>(
        &self,
        machine: SymbolHandle,
        read: impl FnOnce(&[Handle<FlowStateFact>]) -> Result,
    ) -> Result {
        self.states.machine_rows.with_rows(
            &self.states.arena,
            |state| state.machine_symbol,
            &machine,
            read,
        )
    }

    /// First recorded row with this exact state symbol.
    pub fn state_by_symbol(&self, state: SymbolHandle) -> Option<&FlowStateFact> {
        self.states.state_rows.with_rows(
            &self.states.arena,
            |row| row.state_symbol,
            &state,
            |rows| rows.first().map(|handle| self.states.get(*handle)),
        )
    }

    /// First recorded row with this exact machine and state identity.
    pub fn state(&self, machine: SymbolHandle, state: SymbolHandle) -> Option<&FlowStateFact> {
        self.with_machine_rows(machine, |rows| {
            rows.iter().find_map(|handle| {
                let row = self.states.get(*handle);
                (row.state_symbol == state).then_some(row)
            })
        })
    }

    /// A unique recorded row; missing and duplicate identities both return None.
    pub fn unique_state(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Option<&FlowStateFact> {
        self.with_machine_rows(machine, |rows| {
            let mut matching = rows.iter().filter_map(|handle| {
                let row = self.states.get(*handle);
                (row.state_symbol == state).then_some(row)
            });
            let first = matching.next()?;
            matching.next().is_none().then_some(first)
        })
    }

    /// Every row of a machine, preserving arena order even for duplicate states.
    /// References borrow the arena, not the derived grouping's lock.
    pub fn machine_states(&self, machine: SymbolHandle) -> Vec<&FlowStateFact> {
        self.with_machine_rows(machine, |rows| {
            rows.iter().map(|handle| self.states.get(*handle)).collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_rows_preserve_roster_selection_and_mutation_visibility() {
        let machine = SymbolHandle::from_parts(7, 1);
        let other_machine = SymbolHandle::from_parts(2, 1);
        let replacement_machine = SymbolHandle::from_parts(7, 2);
        let state = SymbolHandle::from_parts(3, 1);
        let other_state = SymbolHandle::from_parts(1, 1);
        let replacement_state = SymbolHandle::from_parts(3, 2);
        let assert_roster = |facts: &FlowControlFacts| {
            for owner in [
                machine,
                other_machine,
                replacement_machine,
                SymbolHandle::invalid(),
            ] {
                let expected: Vec<_> = facts
                    .states
                    .iter()
                    .filter_map(|(_, row)| (row.machine_symbol == owner).then_some(row))
                    .collect();
                assert_eq!(facts.machine_states(owner), expected);
                for symbol in [
                    state,
                    other_state,
                    replacement_state,
                    SymbolHandle::invalid(),
                ] {
                    let matching: Vec<_> = expected
                        .iter()
                        .copied()
                        .filter(|row| row.state_symbol == symbol)
                        .collect();
                    assert_eq!(facts.state(owner, symbol), matching.first().copied());
                    assert_eq!(
                        facts.unique_state(owner, symbol),
                        match matching.as_slice() {
                            [only] => Some(*only),
                            _ => None,
                        }
                    );
                }
            }
        };
        let mut facts = FlowControlFacts::default();
        assert_roster(&facts);
        let first = facts.states.insert(FlowStateFact {
            machine_symbol: machine,
            state_symbol: state,
            mutable_parameter_count: 1,
            ..FlowStateFact::default()
        });
        assert_roster(&facts);
        for (owner, symbol, marker) in [
            (other_machine, state, 2),
            (machine, other_state, 3),
            (machine, state, 4),
            (replacement_machine, state, 5),
            (machine, replacement_state, 6),
        ] {
            facts.states.insert(FlowStateFact {
                machine_symbol: owner,
                state_symbol: symbol,
                mutable_parameter_count: marker,
                ..FlowStateFact::default()
            });
            assert_roster(&facts);
        }
        let snapshot = facts.clone();
        facts.states.get_mut(first).machine_symbol = other_machine;
        assert_roster(&facts);
        assert_roster(&snapshot);
        // Replacing the arena through mutable access cannot inherit a warm
        // index, even when the incoming arena has an equal revision.
        let mut replacement = Arena::default();
        replacement.insert(FlowStateFact {
            machine_symbol: replacement_machine,
            state_symbol: replacement_state,
            ..FlowStateFact::default()
        });
        let mut equal_revision = FlowControlFacts::default();
        equal_revision.states.insert(FlowStateFact {
            machine_symbol: machine,
            state_symbol: state,
            ..FlowStateFact::default()
        });
        assert_roster(&equal_revision);
        assert_eq!(equal_revision.states.revision(), replacement.revision());
        *equal_revision.states = replacement;
        assert_roster(&equal_revision);
        facts.states.insert(FlowStateFact {
            machine_symbol: replacement_machine,
            state_symbol: replacement_state,
            ..FlowStateFact::default()
        });
        assert_roster(&facts);
        facts.states.reset_retain_capacity();
        assert_roster(&facts);
        assert_roster(&snapshot);
    }
}
