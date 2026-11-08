# Psi admission and interpreter preparation

Psi consumers reuse a trusted producer's checked judgments. Interpretation keeps
concrete operation checks, while native lowering requires trusted input. The
[product decision](../../spec/terminal-psi/product.md#publication-and-separate-consumption)
replaces receiver proof rediscovery with explicit host trust and a separately
audited route for untrusted interpretation. **PSI-CLAIM-ADMISSION** remains the
tracking ID under **PROOF-KERNEL-CORE** in [TASKS.md](../../../TASKS.md).

## Existing implementation and scope

At `9f5b0b44f749c`, `TerminalProgram::load`, `load_envelope` and
`TerminalExecution::start_artifact` expose byte-loading execution routes without
an explicit host trust witness. CLI inspection has a 64 MiB input cap. Shared
decoders bound counts against remaining bytes and cap nesting and strings, but
loading has no overall enforceable work budget. Canonical decoding also runs
module-foundation validation and dynamic-result admission, including descriptor
and entailment analyses. Inspection and execution preparation therefore need
distinct responsibilities, not a switch that bypasses structural checking.

Primitive stores, reference contracts and boundary normal qualifications can
repeat declaration-dependent entailment during execution. Individual searches
have a 4,096-step limit and depth-64 limit. Their total cost scales with artifact
size and repeated dispatch. Other dispatch paths repeatedly scan type and place
tables. Closed checks of an actual value against its predicate are separate from
that static search and must survive the repair.

Keep the local proof kernel, its source-checking and interpreter evaluation uses,
and the current certificate codec. Proof-term tags 5/6, indexed theorem rows and
`PSICORE` remain canonically encoded; their presence grants no admission authority.
No codec retirement, receiver theorem prover, proof sidecar or guarded native
backend is part of this work. Source automation's remaining kernel integration
is separate from artifact admission.

## Implementation work

1. **Enforce host trust at execution entries.** Require host authorization bound
   to the exact artifact digest and producer at public execution APIs, including
   embeddings and caller-supplied modules. Preserve reuse of trusted immutable
   internal products. Inspection takes no execution authorization. Identity fields
   alone cannot construct trust. Native optimization/lowering requires trusted input.

2. **Separate inspection from execution preparation.** Keep canonical decoding
   and structural validity available without trust. Move execution-only descriptor
   and entailment analyses into authorized preparation. Retain useful computed
   results rather than recomputing at decode, preparation and dispatch. Preserve
   structural rejection of invalid references, counts, shapes and encodings.

3. **Bound shared loading.** Put the existing 64 MiB cap in shared external loaders,
   including envelope paths. Keep count, nesting and string controls. Add an overall
   budget across decoding, re-encoding, validation and preparation; charge expensive
   descriptor/entailment work as well as parsing. Distinguish exhausted work from
   malformed input and unsupported semantics. Do not charge historical producer
   search or attempt to rediscover source theorems.

4. **Prepare operation plans once.** Retain static restrictions, descriptor results
   and dense indexed lookups on the immutable prepared program. Key them by exact
   operation/type/path/domain/qualification inputs and any selected layout or
   installation dependencies. Reuse decode results only when those inputs match.
   Repeated dispatch adds no proof search or whole-declaration-table scans. Keep
   actual-value predicate evaluation and operation safety checks with accounted
   work limits; a prepared restriction never proves a future stored value valid.

5. **Audit containment before opening untrusted execution.** Cover CLI and embedding
   routes, forged facts/qualifications/handles, stale and cross-instance access,
   provider permissions and quotas, faults, budget exhaustion, pending I/O and
   teardown. Host-installed adapters must dynamically enforce what they rely on,
   or deny the crossing. Stop discarded guests, request cancellation, reclaim
   only safely disposable storage, consume late completions for settlement and
   never resume the discarded guest. Retain and charge provider-held storage
   until its obligations settle, including when a provider is uncooperative.

## Acceptance

Extend existing codec, interpreter, embedding and performance suites:

- Trusted exact artifacts execute; absent, mismatched or artifact-forged trust
  cannot authorize execution/native lowering. Inspection remains available without it.
- Proof-bearing current encodings round-trip canonically and confer no trust.
  Existing local kernel and source proof tests keep their meaning.
- Shared loaders reject oversized, malformed and excessively nested inputs before
  unsafe allocation. Structurally valid adversarial inputs hit an enforceable
  aggregate work limit with a distinct exhaustion reason.
- Inspection avoids execution-only analysis while retaining structural checks.
  Execution preparation reuses applicable results exactly once; changed layout,
  domain, path or installation context invalidates the affected prepared plan.
- A refinement-heavy loop adds no declaration-level proof search or whole-table
  scans per dispatch. A bad actual stored value still fails its closed check;
  forged facts never suppress bounds, liveness or permission enforcement.
- Measure both time to first instruction and steady-state throughput before/after,
  with a refinement-heavy workload and a small script linked to a large declaration
  catalog. Record preparation and runtime costs separately; claim no speedup before
  measurement and add no product telemetry merely for this acceptance.
- Before untrusted execution is enabled, exercise denied host access, forged and
  cross-instance handles, work exhaustion and late completion after discard.
  No premature buffer reuse, repeated host effect or discarded-guest resumption;
  an uncooperative provider leaves its outstanding storage retained and accounted.

The existing **PROOF-KERNEL-CORE** work remains: finish identity and metatheory;
source mutual, nested and derivation families; higher indices and open carrier
transport; polymorphic-identity storage receipts; and UIP/extensionality rejection
controls. Retain its `--both` acceptance for
`samples/cli/facts/{closed_integer_bound,negated_mathematical_order,source_indexed_*,erased_reference_contract}`
and `proofs/quotient_*`. This docs decision completes none of those implementations.

## Separate tooling work

The [named stage-validator options](../../spec/build/command_line.md#independent-stage-validators)
remain under **RC-RELEASE-RECORD-AND-CLOSURE**. They exercise compiler stages and
report exact coverage. They supply neither producer trust nor proof of complete
source-to-executable correspondence. No assurance profile is introduced.
