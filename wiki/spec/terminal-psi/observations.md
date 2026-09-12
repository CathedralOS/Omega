# Terminal observations

[Portable product](product.md) | [Boundary realization](boundary_calls.md)

[Calls and outcomes](calls_and_outcomes.md) defines completion, cleanup,
suspension, and crash behavior; this page defines their observation profile.

## Meaning of a profile

`TerminalTraceV1` is a static observer reconstructed from the exact canonical
module, not a record filled in by an execution. An execution has one maximal
semantic trace. A certificate proves refinement under that independently
reconstructed profile. Equal profile identities establish use of the same
observer, not behavior equality without a subject-bound refinement proof.

The ordered, termination-sensitive trace domain is:

```text
ExternalEvent* Return(exact semantic value)
ExternalEvent* Crash(Trap | Abort)
ExternalEvent* ExternalTerminate(effect identity, exact semantic arguments)
infinite maximal execution, represented by its finite observable prefixes
```

Unit return is a value-free return, distinct from every other outcome. Silent
infinite reduction and infinite event-producing reduction are both divergence.
A step simulation may preserve those traces without proving global termination.
Missing ranking evidence, unknown analysis results, and lack of a finite work
guarantee create no execution outcome or observation row.

## Reconstructed rows

The profile binds, in closed order:

1. Domain tag, schema version, Terminal vocabulary, and exact module commitment.
2. One mandatory root row: entry, ordered scalar/structural input schemas, and
   Unit/scalar/structural result-comparison schema.
3. Crash sites ordered by machine, block, and edge, with exact closed cause.
4. Ordinary external-event sites ordered by machine, block, and operation,
   with event kind, exact public boundary/service identity, ordered argument
   schemas, and result schema.
5. Terminal-external sites with exact site, public effect identity, and argument
   schemas.

The canonical profile encoding begins with
`omega.terminal.observation-profile.v1`. It uses fixed little-endian coordinates,
length-prefixed canonical identities, and explicit row-group tags and counts,
including an empty terminal-external group when none exists. Unknown schemas,
vocabularies, tags, classifications, malformed ordering, duplicate coordinates,
missing/extra sites, zero module commitments, and empty profiles reject.
Decoding rejects trailing bytes.

The current V1 implementation rejects modules with boundary crash routes: its
crash rows describe terminator edges, not operation-level boundary outcomes.
Interpreter support for a tagged boundary crash site does not establish a
complete V1 observation profile or permit inventing an edge for that invocation.

The consumer selects the typed schema and may retain an authenticated expected
commitment. The verifier independently derives the instance from the validated
module and compares the complete decoded profile. The proof producer supplies
neither site rows nor weakening flags. Cross-profile reuse requires a checked
canonical forgetting projection; profile equality is not such a projection.

Each ordinary `BoundaryCall` and direct semantic service operation such as
`PortWrite` is an ordered external event. Every new operation must be explicitly
classified as internal, ordinary external, or terminal-external under a known
profile. No default-pure or unknown classification is accepted.

## Values and correspondence

Static rows carry semantic types and comparison rules. Runtime traces carry
actual arguments, results, and return values in execution order. Machine,
block, operation, and edge coordinates establish proof correspondence; they
are not user-visible trace values unless a separate language rule exposes them.

Scalar comparison requires the exact verifier-derived Boolean, fixed-integer,
binary32, or binary64 schema. Malformed integers, type drift, and address-carrier
schemas reject. Booleans and integers compare exactly; IEEE values compare
interchange bits, preserving signed zero and NaN payloads, not host float equality.
Whole-root structural comparison requires exact type, complete canonical
required qualifications, empty runtime paths, and complete opaque value identity.
Projected qualifications and nested runtime-value comparison require additional
support; a comparison helper does not construct a trace or issue refinement.

A digest cannot silently replace semantic value equality. Its use as a compact
coordinate requires an explicit commitment/collision admission.

## Process exit observations

The [process-exit contract](../language/process_exit.md) supplies the canonical
core `ProcessExit` boundary requirement, domain-bound authority, ordinary reach
propagation, and non-crashing external terminal outcome. The exact canonical
requirement owns its public effect identity; provider selection preserves it.
No separate effect name, completion keyword, or public bottom type is required.

Checking and Terminal declarations retain an explicit closed completion kind;
invocation is a terminal transfer with exact arguments and no normal result or
successor, not an ordinary Unit call. A helper that only conditionally exits
retains its normal continuation when it returns. The static reach ceiling is
not an unconditional terminal event, nor a guard describing when exit occurs.
Providers and backends consume the retained fact, never infer it from spelling,
selected implementation, or syscall.

The exit event preserves the preceding ordered events and exact semantic `i32`
status. Host status presentation cannot weaken argument comparison. Successful
external termination establishes neither zero status nor cleanup or discharge
of abandoned obligations. Interpreter exit ends the simulated domain, not the
embedding process. A native containment trap after a violated provider premise
is not a permitted substitute terminal event.

The source-to-Terminal route remains unimplemented. An ordinary Unit
`Console::exit_process` call or a provider's nonreturning native operation still
establishes no terminal-external observation. Unsupported rows reject until
the [implementation task](../../../TASKS.md#process-exit-contract) closes; do
not populate them from interpreter name matching or backend behavior.

## Other observation domains

Fuel exhaustion, evaluator timeout, and producer/checker incomplete outcomes are
consumer/product results, not Terminal program meaning. Compiler-product subjects
add sealed inputs, source diagnostics, artifact bytes, and resource outcomes.
Deployment separately admits the formal-target-to-silicon relationship.
Neither accounting limits nor deployment evidence weakens the reusable trace.
