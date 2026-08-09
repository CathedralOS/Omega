# Terminal Psi Architecture

[Pipeline](pipeline.md)

Status: target architecture settled 2026-08-02. This document describes the
current representation boundary. The semantic and evidence contract is owned by
[`canonical_ir_fuel_and_resource_provisioning.md`](../../design_briefs/canonical_ir_fuel_and_resource_provisioning.md).

This is a pre-release format. Producers and consumers move together; stale
artifacts reject. Git history records superseded vocabularies and implementation
checkpoints rather than this page accumulating a compatibility chronology.

## Boundary

Psi operates on Omega-branded source files and owns every target-neutral stage
through one canonical terminal representation. Omega consumes terminal Psi; it
does not feed source-shaped data back into Psi.

```text
Psi
    source files
    -> tokens -> syntax -> resolved -> typed -> checked
    -> lowered expressions, predicates, places, blocks, and edges
    -> terminal Psi

Omega
    terminal Psi
    -> abstract operations -> target operations
    -> assigned instructions -> bytes -> installed image
```

Parsing therefore belongs to Psi. “Omega files” is the language and product
branding; Psi is the frontend, semantic verifier input, and portable execution
representation.

## Why no existing stage is the cut

The current implementation has no expression-lowering pass before instruction
selection:

- `CheckedTrees` embeds `TypedTrees` plus checked fact tables;
- `StateGraphCode` copies the typed expression table, and operations and
  transitions retain `ExpressionHandle`;
- `ControlFlowCode` clones the same expression table and mostly remaps the
  graph topology and semantic arenas; and
- abstract-operation construction and instruction selection still inspect and
  substitute tree expressions directly.

`StateGraph` and `ControlFlowPlan` are therefore useful topology and evidence
scaffolds, not self-contained executable representations. Conversely,
`AbstractOperations` already owns runtime storage regions, calling-convention
classes, ABI aggregate distinctions, and other Omega realization concerns.
Removing those fields would not reveal a hidden portable IR.

The missing pass is the boundary: merge the useful state-graph/control-flow
shape and fill it with lowered semantic content. This is not serialization of
today's `StateGraph`, purification of `AbstractOperations`, or a second similar
block IR placed beside `ControlFlowPlan`.

## Terminal requirements

Terminal Psi is immutable and self-contained. It contains no arena handle that
requires `TypedTrees`, source syntax, the producer compiler, or instruction
selection to interpret its meaning. It contains:

- concrete machines and instantiated types;
- explicit typed blocks, block parameters, values, calls, transitions,
  continuations, and terminals;
- lowered predicates over the same stable value/place identities as execution;
- typed structural places, including ordinary and provider-backed roots plus
  field, dynamic-index, dereference, and range/subextent projection;
- explicit cleanup, transfer, conservation, invalidation, suspension, and
  boundary actions on edges;
- closed semantic operation variants, including scoped CPU/device ordering
  events; and
- fingerprinted contracts, obligation schemas, authorized admission sites,
  trust attribution, and work identities.

Nominal proposition declarations retain their binder telescopes,
fact-only/witness-bearing classification, and any normalized carrierless
evidence interface in this fingerprinted vocabulary. Changing that interface
is a semantic proof-API revision even though the proposition keeps its nominal
symbol. Transparent proposition definitions expand before terminal production,
have no independent semantic identity, and retain their source names only in
debug maps.

The source pipeline accepts the contextual
`proposition P(...) evidence Interface;` spelling for a witness-bearing
declaration. The retired `{ Interface; }` body rejects with migration guidance;
both spellings are therefore never competing routes to one terminal identity.

Witness-bearing facts additionally retain an evidence-term identity and a
separate derivation-provenance identity. Named `requires` inputs refer to exact
positional erased terms; named `ensures` outputs contribute public fields to a
machine-derived nominal package type that has no source name. Its runtime
projection is the ordinary result and its other fields erase. Outcome guards
control which package variant carries each field. Producer conformances remain
inside proof construction and do not enter proposition or package identity.

Relation applications retain their independently bound left and right carrier
index packs; no global carrier-parameter role is serialized. Selected
constructor lifts, dependency-ordered field relations, and every required
proposition-transport proof enter the semantic rows that justified a lifted
operation. Callable argument telescopes use positional identity, with source
parameter names confined to debug metadata.

An erased binding remains in typed semantic and proof rows with its
multiplicity, validity scope, conservation obligations, and provenance. It has
no executable storage place or cleanup action. Runtime layout and operation
encoding consume the erased-stripped form, while semantic fingerprints retain
the binding and its type.

Author-declared hardware geometry is semantic and may contain offsets, widths,
and alignment. Omega begins where the target chooses native layout, stack and
register placement, ABI classes, concrete storage regions, instructions, and
relocations.

## Psi operation definition

Every operation enters the vocabulary as one reviewed vertical slice:

```text
operation identity and canonical encoding
execution transition
generated obligations and authorized admissions
proof rule / logical interpretation
soundness proof of that rule against the transition
interpreter realization
Omega lowering requirement
fuel identity
```

Operations are statically distinct when execution semantics or generated
obligations differ. Obligation-affecting policy is a closed instruction variant,
not an ordinary value that requires constant folding before verification.
Additional sound proof lemmas may be published without changing operation or
program identity.

The proof kernel, proposition representation, total primitive judgments,
certificate envelope, and admission taxonomy land before an operation depends
on them. Concrete proposition and operation vocabularies are then co-designed
in vertical slices; the proof language is not speculated in isolation.

### Content-conservation proposition slice

The content slice extends structural-place terms with an entry/current version;
it does not add a general historical-expression modality. It carries the exact
owner-unique content-projection identity, canonical
`IntervalSet<CoordinateSpace>` and `CountedQuantity<Unit>` terms, variadic
partial `separate(...)`, containment and equality, and canonical interval-set
residual difference. Sealed claim-frontier rows record content introduced into
or transferred out of checked custody.

The verifier infers identity-preserving reshuffles. A primitive that changes a
partition carries an authored theorem, and checked wrappers compose those
theorems. At a bodyless partial boundary, Psi derives the kept content and
residual and permits the provider to admit only acceptance of custody for that
exact residual—not the partition arithmetic. External root correspondence and
fresh issuance remain scoped admitted hypotheses with provenance; downstream
conservation remains derived.

### Crash-control slice

Terminal Psi represents `Trap` and `Abort` as closed crash causes attached to
distinct no-successor terminators. A crash terminator is not an ordinary
terminal transition and does not encode abandonment by omitting a cleanup list.
It carries the path-conditioned site guard, covering published route buckets,
and the statically known local frontier as an explicit lower bound. The exact
dynamically abandoned frontier is not claimed to be edge-enumerable.

Published crash buckets are fingerprinted semantic content. Each bucket has
one cause and a canonical disjunction of route predicates over the same lowered
values and structural places as executable Psi. Buckets normalize by cause. An
unconditional clause contains the canonical `true` predicate.

The verifier independently reconstructs every crash site and checks:

```text
site_guard implies
    OR(covering_guard for the same cause)
```

Call composition substitutes arguments and caller path facts into published
routes. Disproving every route removes the corresponding crash edge from the
caller's semantic frontier. Evidence derived from a callee body is usable
only when that body is within the same fingerprinted verification unit.
Otherwise the verifier consumes the imported published ceiling and its
certificate.

The frontier lower bound is diagnostic and audit evidence only. It states which
tracked obligations are definitely live at the site; it cannot prove that
unlisted state or external effects remain valid, and no verifier may use it to
license survivors. Fault-tolerant restart requires separate closed-custody,
resource-recovery, external-reset, and target-isolation evidence.

The reference interpreter does not return a crash as data. Reaching a crash
terminator yields a distinct interpreter outcome carrying its cause and
semantic site identity. Build-time evaluation rejects any invocation with a
surviving crash route; a concrete invocation that disproves all routes remains
admissible. Native lowering may retain a physical check even when a caller has
proved its semantic edge unreachable, unless specialization makes erasure
valid.

These normalized obligations are semantic and fingerprinted. Their proof
derivations remain replaceable proof-bundle material.

## Verification boundary

The artifact verifier, proof kernel, and proof producer have distinct jobs:

```text
producer            emits canonical terminal Psi plus candidate evidence
artifact verifier   derives what that exact module is required to prove
proof kernel        checks derivations of those required propositions
```

The artifact verifier canonical-decodes terminal Psi, validates its structure,
derives structural obligations from every operation and edge, and retains the
fingerprinted author contracts. It matches evidence only after reconstructing
the complete obligation set. The proof bundle is not an obligation manifest and
cannot choose what is sufficient. Missing obligations, extra evidence, changed
propositions, wrong module/obligation identities, and unauthorized admission all
reject.

Every accepted fact is:

- re-decided by a specified total kernel judgment;
- discharged by a checked certificate, carried or reconstructed by a total
  certifying procedure; or
- admitted at a sealed site and accepted by the consuming profile.

Admission cannot replace a derivable obligation. Search that may time out or
return unknown must carry its certificate for portable verification. Primitive
trusted judgments are minimized and each joins the enumerable language
soundness audit.

The semantic module, proof bundle, installation record, and debug/source maps
remain separate. Proof improvements do not change semantic identity; provider
selection and attached evidence do change their own section and container
identities. One execution verifies and runs the compiler's current Psi
vocabulary.

`psi-terminal-verifier` is the current Rust implementation of the artifact-aware
judgment; `psi-proof-kernel` implements its current proof checks. That is an
implementation milestone, not the final trust placement. Before terminal-Psi
PCC becomes the deployment authority, the project must choose and implement one
of these auditable closures:

- a low-rung reference artifact verifier that reconstructs the same obligations;
- a Psi verifier that emits an obligation-reconstruction derivation accepted by
  the low-rung proof kernel; or
- an explicitly trusted Psi artifact verifier, named as such in the trust ledger.

A future Psi-hosted proof-kernel implementation may accelerate or independently
cross-check certificate validation. It does not by itself discharge the separate
obligation-reconstruction trust question.

## Canonical semantic bytes

`psi-terminal-codec` owns the canonical encoding of the supported in-memory
vocabulary. The current wire format begins with `PSITERM\0`, a little-endian
`u16` format marker, and a current-vocabulary marker. Counts are fixed-width
little-endian `u32`, stable identities are nonzero little-endian `u64`, integer
payloads occupy the full signed or unsigned 128-bit field, and every sum type
uses a closed one-byte tag. This intentionally favors one simple auditable
encoding over density.

Machines, blocks, and structural-place declarations are strictly ordered by
their stable identities; ensures are strictly ordered by obligation identity. Requirements and flattened
conjunction members are strictly ordered by their canonical encoded bytes,
duplicates are rejected, and symmetric equality operands use that same wire
ordering. Content equations order their symmetric sides canonically;
`separate(...)` is flat, sorted, duplicate-free, and exact projection/domain,
entry/current place, field, fixed-index, and sum-case identities are encoded. Nested
conjunctions, proposition nesting, recursive scalar terms, and content terms
deeper than 256 edges are rejected. Execution-significant vectors—parameters, operations, and
jump arguments—retain their declared order.

Decoding fails on stale format/vocabulary markers or unknown tags, zero identities, invalid booleans,
noncanonical ordering/forms, malformed or verifier-invalid modules, truncated
input, and trailing bytes. A successfully decoded module is re-encoded and the
bytes must match exactly; the decoder never normalizes an alternate encoding.
The semantic fingerprint is SHA-256 over a format-specific domain separator, the canonical
byte length, and those exact bytes. `TerminalPsiIdentity` contains only the
current-vocabulary marker and this fingerprint: proof bundles, installation
records, and debug maps are deliberately absent and remain replaceable.

The current vocabulary includes constants, Boolean and fixed-integer
operations, structural places and content propositions, identity reshuffles,
sum-case paths, authored partition composition, conditional and crash
terminators, nominal proposition identity, the address carrier, proof-gated
exact arithmetic, and guarded runtime arithmetic reconstruction. No part of
that list defines a compatibility sequence during pre-release development.
The arithmetic operations require two already defined operands of the exact
result integer type and have distinct canonical recursive proposition terms for
their exact logical results. Boolean equality requires two already defined
Boolean operands and reconstructs their exact equality result. Integer
equality requires two already defined values of one exact integer type and
reconstructs a Boolean result equating their representations. Integer ordering
has the same operand/result discipline and reconstructs the exact signedness-
aware relation. Bitwise operations require and return one exact integer type
and reconstruct the exact representation-level result. Integer widening
requires the target to contain the complete source range and reconstructs the
unchanged mathematical value at the result type. Validation and execution
accept only the current pre-release vocabulary. Terminal semantic changes
update the compiler, codec, verifier, interpreter, and lowerers in one cut.
Stale modules are rejected rather than migrated, and golden tests freeze only
the current canonical encoding and identity. Current fixtures cover integer
complement and widening, the address carrier, exact casts, shifts and
arithmetic, wrapping and saturating division/remainder, identity reshuffles,
sum-case paths, partition composition, entry claims, Boolean operations,
proposition vocabulary, integer equality and ordering, and bitwise operations.

The same codec gives proof bundles their own canonical `PSIPRF` bytes and golden
fingerprint. The current proof vocabulary covers every terminal proposition and
scalar term, including content conservation, structural places, Boolean and
integer operations, address identity, exact arithmetic, and all three arithmetic
policies. Evidence entries are strictly ordered by `ObligationId`; the closed
encoding covers kernel judgments, recursive proof trees, and exact admission
site/authority/evidence/profile identities. Unknown tags or stale format
markers, zero identities, alternate evidence ordering, truncation/trailing data,
malformed propositions, and excessive proof/proposition nesting reject.
Proof-tree propositions retain their exact rule
direction rather than being normalized as semantic contracts, because a proof
section is replaceable evidence and its cited axiom direction is significant.

`TerminalArtifactManifest` binds the canonical semantic and proof identities
plus optional installation and debug section hashes. Each role has a separate
SHA-256 domain, and absent differs from a present empty section. Replacing a
valid proof, installation record, or debug map changes that section and the
container identity while preserving `TerminalPsiIdentity`; validation
recomputes the complete manifest from attached bytes.

The typed installation payload in `omega-terminal-image-emission` begins with
`PSIINST\0` and
binds the terminal semantic identity, architecture, object format, pointer
size/alignment, PE subsystem when present, exact profile-decision identity,
strictly ordered selected-provider-plan identities, a domain-separated SHA-256
of the complete emitted image, and the compiler text-validation evidence. Its
decoder rejects stale markers or unknown tags, zero identities, invalid target facts,
alternate provider order, nonzero reserved fields, truncation, and trailing
bytes, then reproduces the canonical bytes. Validation recomputes the image
binding from the sealed `TerminalExecutableImage`. The scalar canaries carry an
empty provider set because they contain no calls or boundaries; later vertical
slices populate that set from actual selected plans. The record is manifest
metadata, not executable authority and not a replacement for the separate
`omega-executable-installation` admission/placement ladder. The typed debug map
in `psi-terminal-codec` is replaceable presentation
metadata bound to one exact semantic identity. The checked-source producer
populates retained declaration spans plus exact integer/Boolean-literal and
operator sites for terminal operations and their result values. Authored jump
edges use their exact transition-arrow sites; synthesized return edges retain
the exact returned-expression site.

## Logical fuel

`psi-terminal-fuel` owns the accounting identity independently from terminal
vocabulary identity. The current schedule charges one logical unit for every executed
operation in the current closed terminal vocabulary and one for every taken
terminal edge, including conditional successors and `Crash`. The cost table
matches the closed operation/terminator enums exhaustively, so a new vocabulary
variant cannot compile without making its schedule treatment explicit. A
schedule revision changes accounting identity, never terminal semantic bytes or
the program fingerprint.

The interpreter charges before executing each semantic site and returns a
deterministic `TerminalFuelUsage`: total units plus execution count and units
aggregated under stable `OperationId`/`EdgeId` attribution. Its sponsor-owned
meter may be unbounded or carry a finite allowance. Insufficient allowance is a
host result before the unpaid site, leaves usage unchanged, and is not visible
or catchable as a terminal-Psi machine result. The serialized real-source
canary costs four units—two constants and two edges—and retains the same
semantic identity before and after accounting. `TerminalExecution` retains the
exact block/operation cursor and values across that sponsor event; checked
replenishment resumes at the unpaid site without replaying or double-charging
earlier work, including in the serialized real-source/native canary. Build-time
migration, attributed response outcomes, and trusted native block metering
remain later IRFUEL slices.

A verified crash consumes its one edge unit before producing a distinct
terminal outcome. Repeated resume reports the same outcome without charging or
executing the edge again.

`psi-terminal-fixed-fuel` provides the first restricted checker over this same
schedule. It derives the maximum entry-to-terminal-exit cost over the verified
acyclic CFG with no additional precondition assumptions, memoizing shared tails and
taking the greater successor cost at a conditional rather than summing mutually
exclusive arms. The certificate keys the canonical terminal-Psi identity,
entry machine, schedule identity, and ceiling.
Validation recomputes every field from the verified decoded module; changing
program semantics invalidates an old certificate even when the numeric cost is
unchanged, and a verified but noncanonical module cannot acquire semantic
identity. The source canary's exact four-unit certificate equals measured
execution after source and producer state are discarded. Exact machine-local
block-to-edge segment certificates now reuse the same canonical identity and
schedule, include their selected jump, conditional, or return edge, and reject
an endpoint that is not reached before return. Every explicit edge is a
semantic safe point; the checker derives and validates the complete reachable
graph partition in canonical block/edge order so no branch segment can be
omitted or reordered. Crossing a conditional within one unselected segment
still fails closed. Loop outcomes, relevant-precondition subsets, and Cathedral
hard-root migration remain later slices.

Omega external-root composition now accepts those sealed entry and segment
certificates as a distinct local-evidence form beside admitted opaque-provider
summaries. It derives local units and schedule from the certificate, retains no
provider receipt for recomputable Psi evidence, and reports the terminal
semantic identity and exact entry/segment endpoint. A sealed Omega binding now
checks that terminal artifact text is exactly the relocation-free frozen
installed bytes, that architectures match, and that the selected entry stub
names the certified function offset. External-root installation rechecks the
whole-entry certificate against the exact root code context and stub; a
segment-only root fails closed. The real-source canary crosses the complete
generic installation ladder. Migrating the Cathedral hard-root graph remains.

## Remaining implementation

- Extend terminal Psi in obligation-complete vertical slices: semantics, reconstructed obligations, proof checking, canonical encoding, fuel, interpretation, and Omega lowering land together.
- Add general blocks, calls, aggregates, structural places, cleanup and transfer actions, boundary operations, loops, suspension, and scoped ordering without restoring source-tree dependencies below Psi.
- Complete sealed content-introduction and custody-exit frontiers and general conservation composition.
- Re-root remaining interpreter and native paths on decoded, verified terminal artifacts, then retire redundant checked-tree and legacy graph consumers.
- Extend fixed-fuel certificates to loops and migrate Cathedral hard roots; add native metering only when it preserves semantic-site attribution.
- Complete source provenance for newly admitted operations and edges.

Temporary differential paths may coexist while a consumer moves. They are test oracles, not alternate language versions or a permanent Omega-to-Psi path.
