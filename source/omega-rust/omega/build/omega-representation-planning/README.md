# Omega representation planning

This crate is the compiler-owned entrance for provider-backed opaque value
representations.

`src/lib.rs` harvests `Build::select_representation<Opaque, Conformance>()`
from the authoritative build machine, closes the exact named conformance, and
validates its compiler-owned trait, opaque subject, and concrete carrier. It
does not accept byte sizes, alignments, ABI classes, or movement rules from
source.

Downstream target consumers derive physical shape from the retained carrier.
Calling-policy closure is the first consumer. General layout, package review,
artifact compatibility, and replacement contracts must consume the same
application rather than re-discovering a carrier.
