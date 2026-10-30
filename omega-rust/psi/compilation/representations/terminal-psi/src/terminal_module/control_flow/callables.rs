//! Exact executable and boundary declaration identities in one Terminal catalog.

use semantic_vocabulary::{BoundaryMachineId, MachineId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalCallable {
    Machine(MachineId),
    Boundary(BoundaryMachineId),
}
