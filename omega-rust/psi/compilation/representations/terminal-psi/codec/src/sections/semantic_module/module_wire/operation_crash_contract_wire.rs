//! Operation crash contracts on the wire: the owning machine and operation,
//! the published routes in the operation's formal namespace, then the
//! surviving continuations in the machine's actual-value namespace.

use super::super::wire_fields::{CrashRoutes, Id, wire_record};
use terminal_psi::TerminalOperationCrashContract;

wire_record! {
    Contract: TerminalOperationCrashContract {
        machine: Id,
        operation: Id,
        published_routes: CrashRoutes,
        crash_continuations: CrashRoutes,
    }
}
