# Gamma expression serialization

Run `sh tests/delta/emission/run.sh`. The small shell entrance materializes the
explicit Gamma control closure, prefixes the selected compiler implementation
closure, and invokes the selected Gamma evaluator. `gate.py` only frames inputs,
checks identities, and compares authored bytes. `fixtures.py` is an exact byte
inventory, not a host serializer or lowering implementation.

Start at `controls/main.gamma`: it routes to fixed-word cases, ordinary call
fallbacks, expression ordering, and production capture reconstruction.
`observation.gamma` measures the node with production count-only serialization,
then publishes it through production expression serialization. Both running
counts start at 11. Each observation contains little-endian u32 extent and
count-only result, a cache-present byte, exact expression bytes, the publication
result as u32, and the selected unmarked evaluator's final zero byte. Count-only
publication, stale extents, missing separators, and misplaced closes therefore
change the exact observation.

Eighteen publication controls cover fixed-word lengths 1 and 7 versus fallback
lengths 0 and 8, ignored packed high bytes, source-span and generated heads,
nullary/binary calls, mixed unary chains, siblings, and let initializers/bodies.
Production capture rebuilds a call and a let while replacing `$v3` with `$c100`;
the changed spelling requires a new extent, and reconstructed unary calls must
retain their cache. An assertion checks that the maximum positive packed input
at length 7 produces exactly `2^59 - 1` cache metadata. The NUL word checks that
a cached zero byte remains distinguishable from absent metadata.

The nineteenth control checks negative packed metadata without invoking its
byte writer. All 19 controls run twice, requiring 38 exact observations with
status zero and empty stderr under unchanged 30-second watchdogs.

These are synthetic internal Gamma-plan controls, not Delta admissions or
executable Gamma program claims. Empty heads, NUL, and high-byte words are
deliberate private representation boundaries. Normal admitted programs, full
compiler receipts, resource refusal, and selected-program execution remain the
responsibility of their existing gates. This gate makes no performance claim.

`controls/emission.gamma.sources` pins every authored Gamma control member;
`compiler.tsv` pins the complete diagnostic-plus-implementation identity, checked
before any observation runs.
