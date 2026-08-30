# Omega representation planning

This crate is the compiler-owned entrance for provider-backed opaque value
representations.

`src/lib.rs` harvests `Build::select_representation<Opaque, Conformance>()`
from the authoritative build machine, closes the exact named conformance, and
validates its compiler-owned trait, opaque subject, and concrete carrier. It
does not accept byte sizes, alignments, ABI classes, or movement rules from
source.

D26 scopes uniqueness to the compilation activation: one opaque declaration
has at most one selected application, including an unused selection. Current
orchestration evaluates one authoritative build machine, so the local harvest
implements that invariant. `CheckedCompilation` retains the complete validated
collection unchanged, including unused selections, so downstream work need not
rediscover an application from syntax or a calling-plan digest; completed
build-configuration validation must keep it fail-closed if orchestration later
admits more than one. This custody is not a package-review availability row,
consumer demand, or physical ABI commitment. Dependency builds publish
generated-source bundles and their selections are not imported as consumer
policy.

Downstream target consumers derive physical shape from the retained carrier.
Calling-policy closure is the first consumer. General layout, package review,
artifact compatibility, and replacement contracts must consume the same
application rather than re-discovering a carrier. Package review projects the
producer's available declaration/conformance/carrier surface separately from a
consumer demand created by an actual by-value use. The selecting build source
is provenance rather than ABI identity; independently compiled artifacts
compare strong application commitments at their real by-value composition
edges.
