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

The Omega abstract-operation and interpreter entries accept canonical semantic
and proof sections plus an explicit admission profile, decode and verify them,
and only then construct realization requirements or resumable execution state.
No public in-memory module or checked-tree bypass exists at either artifact
boundary.

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

Witness-bearing declarations use the contextual
`proposition P(...) evidence Interface;` form. The normalized evidence
interface enters terminal proposition identity.

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

### Direct scalar call slice

The current `Call` operation names one canonical callee, carries positional
scalar arguments, and carries exactly one caller obligation identity for each
published callee `requires` clause. Validation checks the complete signature,
argument definedness and types, result type, obligation arity, and global
obligation uniqueness. Verification substitutes the positional arguments into
the callee requirements and guarantees: requirements become caller proof
obligations, while verified guarantees enter the caller's semantic axioms.

This first call slice is deliberately crash-free and value-only. A callee with
published crash routes or structural/content contracts rejects because silently
discarding its crash continuation or custody effects would change the program.
Those shapes require their own control and structural vertical slices; they are
not flags on the scalar operation.

The interpreter uses owned call frames and charges the call before entering the
callee. Sponsor exhaustion in the callee resumes without replaying that paid
call. Validation rejects recursive call graphs until terminal Psi can carry and
verify the required tail-position and ranking evidence. Fixed-fuel derivation
includes complete acyclic callee bounds and retains its own cycle rejection as
defense in depth.

Omega selects each callee's native calling plan, evaluates arguments into
disjoint frame spills before filling ABI registers, preserves the AArch64 link
register, honors Microsoft x64 shadow space and x86 stack alignment, and emits a
typed internal-call relocation tied to the exact Psi operation and callee.
Stack-passed scalar arguments and calls inside conditional-control lowering are
remaining engineering coverage, not unresolved language design.

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
It carries a canonical site-guard set, covering published route buckets, and
the statically known local frontier as an explicit lower bound. The guard set
contains the exact incoming conjunction plus sound canonical consequences used
as route witnesses; retaining a consequence never erases the exact path
identity. The exact dynamically abandoned frontier is not claimed to be
edge-enumerable.

Published crash buckets are fingerprinted semantic content. Each bucket has
one cause and a canonical disjunction of route predicates over the same lowered
values and structural places as executable Psi. Buckets normalize by cause. An
unconditional clause contains the canonical `true` predicate.

The verifier checks each crash site against the canonical guard facts carried
by that site:

```text
the published route is Truth, or site_guard contains
    a canonical predicate from that same-cause route
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
admissible. Native lowering preserves every reachable no-successor crash leaf.
It may retain a physical check even when a caller has proved its semantic edge
unreachable, unless specialization makes erasure valid.

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

`psi-terminal-verifier` implements the artifact-aware judgment and
`psi-proof-kernel` checks its proofs. Before terminal-Psi PCC becomes deployment
authority, that verifier requires one auditable closure:

- a low-rung reference artifact verifier that reconstructs the same obligations;
- a Psi verifier that emits an obligation-reconstruction derivation accepted by
  the low-rung proof kernel; or
- an explicitly trusted Psi artifact verifier, named as such in the trust ledger.

A future Psi-hosted proof-kernel implementation may accelerate or independently
cross-check certificate validation. It does not by itself discharge the separate
obligation-reconstruction trust question.

## Canonical semantic bytes

`psi-terminal-codec` owns one canonical encoding of the supported in-memory
vocabulary. `PSITERM\0` bytes carry a format marker and the one current-vocabulary
marker. They use fixed-width little-endian counts, stable nonzero identities,
full-width integer payloads, and closed sum tags. The format favors auditability
over density.

Unordered semantic sets are strictly sorted by stable identity or canonical
bytes and reject duplicates; symmetric terms use the same canonical ordering.
Execution-significant sequences—parameters, operations, and jump arguments—
retain declaration order. Recursive terms have a fixed depth limit.

Decoding fails on stale markers, unknown tags, invalid identities or values,
noncanonical forms, verifier-invalid modules, truncation, or trailing bytes. A
successful decode re-encodes byte-for-byte; the decoder never normalizes an
alternate spelling. `TerminalPsiIdentity` binds a domain-separated hash of the
exact canonical semantic bytes and excludes replaceable proofs, installation
records, and debug maps.

Operation variants are closed, typed, and refer only to already defined values.
Each variant reconstructs its logical result and obligations under the
vertical-slice rule above. This pre-release project has no format migration
path: semantic changes move the compiler, codec, verifier, interpreter, and
lowerers together; stale modules reject. Golden tests freeze only the current
encoding and identity.

Proof bundles have separate canonical `PSIPRF` bytes and identity. They carry
one current proof-system marker; stale markers reject. Evidence is strictly
ordered by obligation identity and retains exact kernel rules, proof trees, and
admission identities. Proof propositions preserve rule direction because cited
axiom direction is significant even though the proof section is replaceable.

`TerminalArtifactManifest` binds semantic and proof identities plus optional
installation and debug hashes. Each role has a separate hash domain, and absent
differs from present-but-empty. Replacing a valid nonsemantic section preserves
`TerminalPsiIdentity` while changing its own section and container identities.

The canonical `PSIINST\0` installation payload binds semantic identity, target
facts, exact profile/provider decisions, the complete emitted-image hash, and
text-validation evidence. It is manifest metadata, not executable authority;
installation still consumes separate admission and placement authority. Debug
maps are replaceable presentation metadata bound to the exact semantic identity
and never participate in semantic meaning.

## Logical fuel

`psi-terminal-fuel` owns accounting identity independently from terminal
semantic identity. The schedule exhaustively assigns cost to every closed
operation and terminator variant, so extending the vocabulary requires an
explicit accounting decision. A schedule change never changes program identity.

The interpreter charges before each semantic site and reports deterministic
total and per-`OperationId`/`EdgeId` usage. Fuel is sponsor-owned: exhaustion
is not visible or catchable by the Psi machine. Resumption continues at the
unpaid site without replay or double charging; a completed crash edge is not
charged again.

`psi-terminal-fixed-fuel` derives certificates from verified terminal control.
For acyclic control and call graphs it computes the greatest entry-to-exit path,
taking the maximum rather than the sum at exclusive branches and including the
complete bound of each reached callee. Entry and segment
certificates bind the exact terminal identity, schedule, endpoints, and ceiling;
validation reconstructs those fields and the complete reachable segment
partition. Ranked tail calls, loops, and relevant-precondition refinements
require later vertical slices.

Omega may use a certificate only for the exact installed terminal bytes,
architecture, entry stub, and external-root context it names. Recomputable Psi
fuel evidence carries no provider receipt.

## Implementation queue

[`TASKS.md`](../../../TASKS.md) owns remaining terminal-Psi work. Temporary
differential paths may coexist as test oracles while consumers move; they are
not alternate language versions or a permanent Omega-to-Psi path.
