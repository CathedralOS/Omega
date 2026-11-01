//! Historical fold compatibility on the canonical semantic wire.
use super::super::wire_fields::{Digest, Id, Index, List, Nominal, Optional, Text, wire_record};
use terminal_psi::{FoldedConstantCandidate, FoldedConstantClosure, FoldedConstantDependency};

wire_record! {
    Closure: FoldedConstantClosure { target: Optional<Text>, dependencies: List<Dependency> }
    Dependency: FoldedConstantDependency {
        target: Text,
        requirement: Index,
        plan: Digest,
        candidate: Optional<Candidate>,
    }
    Candidate: FoldedConstantCandidate { boundary: Id, machine: Id, declaration: Nominal }
}
