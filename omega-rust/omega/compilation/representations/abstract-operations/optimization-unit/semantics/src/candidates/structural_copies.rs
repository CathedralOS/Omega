//! Leaf-copy projection predicates shared by candidate replays.
//!
//! A `StructuralLeafCopy` result holds exactly its source's contents at the
//! copied projection: the copy is fresh storage of an `Unrestricted` or
//! owned non-reference leaf, so an observation of the result replays
//! against the source's own establishment chain with the projection
//! prefixed. A `StructuralCopyPath::Case` projection already spells
//! verifier handles; a `StructuralCopyPath::Ordinary` projection spells
//! source-level field identities, which the canonicalizer resolves to the
//! reached roster's verifier `StructuralFieldId` before any replay step
//! consumes the path — a name absent from the roster, a non-structural
//! field carrier, and every segment with no canonical step leave the copy
//! uncrossed.
