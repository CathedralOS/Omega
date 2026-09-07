# Terminal codec

Public contract: [Terminal product](../../../../wiki/spec/terminal-psi/product.md).
The [remaining canonical-byte reference](../../../../wiki/architecture/pipeline/terminal_psi.md#canonical-semantic-bytes)
still owns wire details awaiting specification consolidation.

[lib.rs](src/lib.rs) is the codec entry map. The semantic module, proof bundle,
artifact envelope/manifest, debug map, installation payload, optimization
execution, obligation ledger, and observation profile have distinct owners.
Do not infer one section's identity from another section's current version.

## Observation-profile implementation

[terminal_trace_v1_profile.rs](src/terminal_trace_v1_profile.rs) implements
canonical profile encoding and module-bound acceptance for
[TerminalTraceV1](../../../../wiki/spec/terminal-psi/observations.md). Acceptance
validates the module, derives its complete identity and site roster independently,
and compares the decoded profile exactly.

The current encoder writes the terminal-external group with zero count; the
decoder rejects a nonzero count. That is an implementation fence, not an assertion
that the semantic trace excludes successful external termination. Source
completion declarations remain an owner question before that route can be built.
Scalar and whole-root structural comparison helpers provide value comparison
only, not trace construction or refinement evidence.

## Compatibility gap

The public pre-release contract requires stale artifacts to reject. The current
module decoder additionally recognizes the legacy result-path format pair
`56/59`. This is an implementation discrepancy, not a second compatibility
policy for readers to choose. Retire or explicitly resolve that route under the
current contract before claiming current-only decoding. Do not preserve the
legacy branch by adding an undocumented migration rule to the specification.
