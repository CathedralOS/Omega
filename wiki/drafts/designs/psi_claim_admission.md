# Psi admission and interpreter preparation

Psi consumers reuse a trusted producer's checked judgments. Interpretation keeps
concrete operation checks, while native lowering requires trusted input. The
[product decision](../../spec/terminal-psi/product.md#publication-and-separate-consumption)
replaces receiver proof rediscovery with explicit host trust and a separately
audited route for untrusted interpretation. Its five steps, guest discard and
the embedding's contained interpretation route have landed; what remains is
**PSI-UNTRUSTED-INTERPRETATION** in [TASKS.md](../../../TASKS.md).

## Existing implementation and scope

Steps 1 to 5 below have landed, a faulted, stopped or waiting guest can be
discarded under the embedding's four-step sequence, and a host can run an
artifact it does not trust through `psi_authorization::admit_contained_interpretation`
and `TerminalProgram::admit_contained`. The
[containment audit](psi_containment_audit.md) reviews each binding and lists
what remains. Decoding marks a closed module as
received, and the
interpreter's `TerminalProgram::borrow` and `retain` refuse received code. Received
Psi executes only through `TerminalProgram::admit`, `retain_authorized` or
`TerminalExecution::start_artifact`, each of which takes the host's
`AuthorizedTerminalArtifact`; `psi_authorization::authorize_terminal_artifact` is
the only code that makes an executable copy of received bytes. The byte-loading
`TerminalProgram::load` and `load_envelope` are gone. CLI inspection has a 64 MiB
input cap. Shared
decoders bound counts against remaining bytes and cap nesting and strings.
`terminal_wire::PsiLoadLimits` carries the host's optional input size cap and
one work budget; `CanonicalTerminalArtifact::from_bytes_within` and
`ClosedTerminalModule::from_semantic_bytes_within` check the cap before reading
and charge parsing, section re-encoding, structural validation and, at
authorization, execution preparation to one `LoadAccount` before each phase runs.
Exceeding either limit is `CodecError::InputTooLarge` or `LoadWorkExhausted`, a
resource refusal distinct from malformed input. Canonical decoding keeps
module-foundation validation, identity-formation shape and theorem-citation
checks. The checker replays execution relies on (structural memberships,
external-completion providers and dynamic view results) run once in
`terminal_wire::sealing::prepare_received_execution` when the host authorizes a
received artifact, so inspection skips them and no executed received module
misses them.

Reference contracts and boundary normal qualifications no
longer repeat declaration-dependent entailment during execution: an admitted
program prepares `terminal_semantics::IntrinsicMemberships` once for its exact
declaration view, including layout and scalar-geometry replacements, and
dispatch answers from it with indexed type and domain lookups. Each
`WriteOnlyPrimitiveStore` likewise reads a plan of its destination's declared
root, scalar restriction and UTF-8 obligation, prepared once per program
instead of on every store. The stored value is still checked against that
restriction on every store. Owned-value claim checks read a `ClaimRoster`
prepared once per program instead of sorting and classifying every type on each
owned argument, and the path walks these checks share look declarations up by
binary search. Other dispatch paths still repeatedly scan type and place
tables. Closed checks of an actual value against its predicate are
separate from that static search and must survive the repair.

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

3. **Bound shared loading.** Enforce any encoded input size limit set by the
   receiving host in shared external loaders, including envelope paths.
   Keep length, count, nesting and string controls. Add an overall
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
- Emit a valid artifact within the caller's output quota. Load it under a host
  budget that admits it and verify a resource refusal under a smaller input cap.
  Shared loaders reject malformed and excessively nested inputs before
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

The existing **PROOF-KERNEL-CORE** work remains: finish explicit identity, and
source mutual, nested and derivation families (multiple and value indices
included). The kernel's K/UIP, identity eta, function extensionality and
equality reflection rejection controls exist. Open-carrier transport needs no
separate rule, since identity elimination already ranges over any carrier
variable. No storage receipt is added for polymorphic identity, because the
receiving kernel checks the citation itself. Retain its `--both` acceptance for
`samples/cli/facts/{closed_integer_bound,negated_mathematical_order,source_indexed_*,erased_reference_contract}`
and `proofs/quotient_*`. This docs decision completes none of those implementations.

## Separate tooling work

The [named stage-validator options](../../spec/build/command_line.md#independent-stage-validators)
remain under **RC-RELEASE-RECORD-AND-CLOSURE**. They exercise compiler stages and
report exact coverage. They supply neither producer trust nor proof of complete
source-to-executable correspondence. No assurance profile is introduced.
