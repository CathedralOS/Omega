//! Filesystem provider dispatch, rooted output custody, and logical handle tracking.
//! `filesystem_calls.rs` dispatches calls to the selected provider.
//! `observations.rs` and `logical_handles.rs` retain operation custody,
//! and `virtual_filesystem.rs` supports the differential oracle.

use crate::interpreter::evaluator::filesystem::preparation::PreparedByteOutput;
use crate::interpreter::evaluator::{EvalResult, Evaluator};

mod filesystem_calls;
pub(super) mod host_open_flags;
pub(super) mod host_operation;
pub(super) mod logical_handle_store;
mod logical_handles;
mod observations;
pub(super) mod preparation;
/// The REAL-filesystem provider (opt-in `FilesystemAccess::RealUnscoped`; the
/// build.omg rung). A CHILD module so it can serve ops against the private
/// `Evaluator` internals (the fs argument/buffer helpers) without widening
/// their visibility outside the interpreter owner.
pub(in crate::interpreter) mod real;
mod virtual_filesystem;

impl<'program> Evaluator<'program> {
    /// Shared find_next cursor arm: pop the next snapshotted entry and write
    /// it into `data` (1 = filled, 0 = end-of-enumeration or unknown handle).
    pub(in crate::interpreter::evaluator) fn pop_virtual_find(
        &mut self,
        handle: i64,
        data: &PreparedByteOutput,
    ) -> EvalResult<i64> {
        Ok(
            match self
                .virtual_finds
                .get_mut(&handle)
                .and_then(std::collections::VecDeque::pop_front)
            {
                Some((name, is_dir)) => {
                    self.write_find_data(data, &name, is_dir)?;
                    1
                }
                None => 0,
            },
        )
    }

    /// Shared find_close arm: release the cursor (BOOL, like Win32).
    pub(in crate::interpreter::evaluator) fn close_virtual_find(&mut self, handle: i64) -> i64 {
        if self.virtual_finds.remove(&handle).is_some() {
            1
        } else {
            0
        }
    }
}
