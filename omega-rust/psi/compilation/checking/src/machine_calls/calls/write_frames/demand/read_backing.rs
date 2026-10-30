//! Frozen dependencies of one admitted storage read.
//!
//! Typed coordinate resolution establishes identity. These presentation paths
//! only retire an already captured value after overlapping effects; they never
//! establish a new value, lifetime, loan or reference origin.

use super::super::{caller_aliases, reference_subjects, state_write_walk, stored_calls};
use super::{
    CallFrameResolver, CollectedStatementPrefix, ExpressionHandle, FrameInference, Machine, State,
};
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedStorageReadBacking {
    pub(crate) sources: Vec<super::super::LocalWriteSource>,
    pub(crate) values: Vec<String>,
    pub(crate) bindings: Vec<String>,
}

impl CapturedStorageReadBacking {
    pub(crate) fn survives(&self, written: &[String]) -> bool {
        self.values.iter().all(|value| {
            written
                .iter()
                .all(|write| !super::frame_paths_overlap(write, value))
        }) && self.bindings.iter().all(|binding| {
            written.iter().all(|write| {
                // A write below a loaded pointer changes its referent, not
                // the pointer slot. Whole-slot/ancestor and coarse writes do.
                !(write == binding
                    || write
                        .strip_suffix("[*]")
                        .is_some_and(|_| super::frame_paths_overlap(write, binding))
                    || binding
                        .strip_prefix(write)
                        .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('[')))
            })
        })
    }
}

impl CallFrameResolver<'_> {
    /// An invocation-entry contract observes this original statement prefix.
    pub(crate) fn storage_read_backing_before_statement(
        &self,
        machine: &Machine,
        state: &State,
        statement: usize,
        root: SymbolHandle,
        path: &[facts::PlaceSegment],
    ) -> Option<CapturedStorageReadBacking> {
        let prefix = state_write_walk::collected_prefix_at(
            &self.caches.state_write_shared_collections,
            self.program,
            machine,
            state,
            &self.tables.symbols,
            statement,
            true,
        )?;
        reference_subjects::read_backing::capture(
            self.program,
            machine,
            state,
            statement,
            root,
            path,
            &prefix,
        )
    }

    /// A completed read follows its evaluated children. Earlier operands may
    /// replace a descriptor, so its incoming statement prefix is insufficient.
    pub(crate) fn storage_read_backing_at_expression(
        &self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        root: SymbolHandle,
        path: &[facts::PlaceSegment],
    ) -> Option<CapturedStorageReadBacking> {
        let (owner, statement, index) = caller_aliases::caller_statement_owner(
            self.program,
            machine,
            caller_aliases::CallerWriteSite::Expression(expression),
        )?;
        if owner.symbol != state.symbol {
            return None;
        }
        let prefix = state_write_walk::collected_prefix_at(
            &self.caches.state_write_shared_collections,
            self.program,
            machine,
            state,
            &self.tables.symbols,
            index,
            true,
        )?;
        let transfer = stored_calls::effects_at_expression(
            self.program,
            machine,
            state,
            statement,
            &prefix,
            &self.tables.symbols,
            &mut FrameInference::for_state(state.symbol),
            expression,
        )?;
        if !transfer.is_complete() {
            return None;
        }
        let mut after_children: CollectedStatementPrefix = prefix.clone();
        stored_calls::apply_transfer(self.program, &mut after_children.stored, &transfer);
        reference_subjects::read_backing::capture(
            self.program,
            machine,
            state,
            index,
            root,
            path,
            &after_children,
        )
    }
}
