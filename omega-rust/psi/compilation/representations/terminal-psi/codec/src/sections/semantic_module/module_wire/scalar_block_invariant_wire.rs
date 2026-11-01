//! Scalar block invariants on the wire: the header block, predicate and
//! arrivals of one invariant.

use super::super::wire_fields::{Id, List, Proposition, wire_record};
use terminal_psi::{ScalarBlockInvariant, ScalarBlockInvariantArrival};

wire_record! {
    Invariant: ScalarBlockInvariant {
        machine: Id,
        header: Id,
        predicate: Proposition,
        arrivals: List<Arrival>,
    }
    Arrival: ScalarBlockInvariantArrival { edge: Id, obligation: Id }
}
