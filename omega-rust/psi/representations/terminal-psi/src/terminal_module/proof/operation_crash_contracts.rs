//! Invocation-specific crash contracts at operation granularity.
//!
//! An in-module call carries its surviving crash continuations beside the
//! callee identity, and a boundary call reconstructs them from the boundary
//! declaration's published routes. A selected operator invocation that lowers
//! to one ordinary scalar operation has neither: the operator's published
//! routes belong to no Terminal machine or boundary declaration. These rows
//! carry that contract beside the exact operation so the verifier can
//! substitute the operation's own operands and check same-cause caller
//! coverage exactly as it does for calls.

use crate::CrashRouteBucket;
use semantic_vocabulary::{MachineId, OperationId};

/// One operation's published crash contract and its surviving continuations.
///
/// Rows are strictly ordered by `(machine, operation)`. The operation must
/// expose a positional scalar operand roster of its own; call operations keep
/// their existing carriers and never take a row here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalOperationCrashContract {
    pub machine: MachineId,
    pub operation: OperationId,
    /// Nonempty canonical may-routes in the operation's declaration-local
    /// formal namespace: the scalar operand at zero-based ordinal `k` is the
    /// formal `ValueId` `k + 1`, typed by that operand's scalar type. These
    /// identities are not executable caller values, even when their numbers
    /// coincide with values of the owning machine.
    pub published_routes: Vec<CrashRouteBucket>,
    /// Invocation-specific routes in the owning machine's actual-value
    /// namespace: exactly the published routes with every formal replaced by
    /// the operation's operand at that ordinal. Empty or untranslated rows
    /// cannot erase a published crash.
    pub crash_continuations: Vec<CrashRouteBucket>,
}
