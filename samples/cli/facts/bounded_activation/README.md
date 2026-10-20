# Bounded activation establishment

Owning requirement: **RUNTIME-SIZED-ACTIVATION-CONTRACT** in `TASKS.md`.
Contract: [activation storage](../../../../wiki/spec/resources/activation_storage.md).

`main.omg` commits three bytes under bound eight, writes and reads byte two
as 37, releases, then requires checked rejection of nine. The frozen source
is unchanged from the genuine failing baseline. All companions bind the four
hosted targets; execution evidence is Linux x86_64 only.

The baseline producer at `53cd2938437a29356a48d6eaf29f14de07a4bb99` plus the
proposed core API rejected the authored program with seven bodyless `try_claim`
and `bytes` diagnostics (exit 200, 6.9475 seconds). This is a missing feature,
not an existing-program regression. No baseline engine executed.

The exact sealed core declarations now establish caller-activation provenance.
Checked calls retain a closed static bound and runtime committed operand;
Terminal operations retain the bound, conditional claim and release identity.
Publication and optimized-graph validation replay actual CFG custody and loan
lineage. Native storage reserves the full bound in the fixed frame, including
probe and unwind coverage; Terminal keeps stable backing per activation.
Successful committed bytes are initialized before readable loans are exposed.
Requests above the bound produce `Rejected`, without clamping or trapping.

The mechanism witnesses exercise distinct paths rather than a transport matrix:

| Witness | Obligation |
| --- | --- |
| root and `field_transport` | Same 3/37/9 observation, local and owned field transport |
| `parameter_committed` | Runtime operand through a helper; scalar return after release |
| `named_attempt` | Conditional claim survives a named sum before selection |
| `named_bound`, `computed_bound`, `generic_bound` | Closed bound through static identity and specialization |
| `zero_bound`, `unsigned_max_request` | Empty grant and unsigned checked admission |
| `nested_lifo`, `multiple_bounds` | Co-live demand, reverse release and disjoint full backing |
| `cleanup_pure` and the cleanup-crossing reject control | Real nominal hook preserves live claims only with a closed pure call graph |
| `page_bound` | Full 8192-byte bound crosses a 4096-byte host page with committed extent three |

Matching `bounded_activation_*` corpus fixtures pin both engine exits as zero.
Source assertions check extent and contents; those values are not printed
measurements. An independent oracle falsifier changes the expected byte to 36
while retaining the store of 37 and must reach a matching runtime Trap.

The four original one-obligation negative controls under `tests/omega/fail/facts` reject
escape and stale loans during source checking, and wrong activation and non-LIFO
release during Terminal publication, before either engine executes. The additional cleanup control changes only the hook call from a pure leaf
to a boundary-reaching machine and refuses a possible suspension. A later
publication refusal is deliberately not described as a source-check refusal.

This establishes the bounded serial path through source checking, checked-to-
Terminal lowering, current optimized semantics, ordinary calling plans, fixed
frame realization, instruction selection and native image emission. No provider
supplies the grant. Suspension retention, claim splitting/merging, partial
mutable-owner windows and further target execution remain open. The serial
realization rejects possible suspension crossings while a claim is live,
including transitive calls, unknown targets and nominal cleanup calls.

Invocation-local ignored `build/activation-slot4` retains producer identities,
canonical passages with source revisions, raw observations, qualification and
review receipts. Historical failed experiments remain historical evidence.
