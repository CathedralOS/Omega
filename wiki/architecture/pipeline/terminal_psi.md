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

The Psi reference-interpreter entry and Omega abstract-operation entry accept
canonical semantic and proof sections plus an explicit admission profile,
decode and verify them, and only then construct resumable execution state or
realization requirements. No public in-memory module or checked-tree bypass
exists at either artifact boundary.

Parsing therefore belongs to Psi. “Omega files” is the language and product
branding; Psi is the frontend, semantic verifier input, and portable execution
representation.

## Why the bootstrap stages are not the cut

The older bootstrap lane does not provide a portable expression-lowering
boundary:

- `CheckedTrees` embeds `TypedTrees` plus checked fact tables;
- `StateGraphCode` copies the typed expression table, and operations and
  transitions retain `ExpressionHandle`;
- `ControlFlowCode` clones the same expression table and mostly remaps the
  graph topology and semantic arenas; and
- its abstract-operation construction and instruction selection inspect and
  substitute tree expressions directly.

`StateGraph` and `ControlFlowPlan` are therefore useful topology and evidence
scaffolds for slices not yet migrated, not self-contained executable
representations. Conversely,
`AbstractOperations` already owns runtime storage regions, calling-convention
classes, ABI aggregate distinctions, and other Omega realization concerns.
Removing those fields would not reveal a hidden portable IR.

`psi-checked-trees-to-terminal` now builds the supported portable slices, and
`omega-terminal-psi-to-abstract-operations` consumes verified terminal Psi.
The remaining migration extends that one boundary and retires corresponding
tree consumers. It does not serialize `StateGraph`, purify
`AbstractOperations`, or place a second similar block IR beside
`ControlFlowPlan`.

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

Unit structural declarations apply the same rule directly: every field row
retains authored relevance, and an erased row carries its exact normalized type
identity as an opaque semantic type rather than forcing proof data into the
executable structural-type graph. The codec and verifier reject mismatched
relevance/type rows. Omega skips erased rows before ABI classification, so the
terminal artifact preserves semantic identity without assigning proof evidence
an offset or transfer.

An entry claim may name either its complete structural parameter or a stable
record-field path below it. Each path segment uses the structural field's exact
canonical identity: `#<id>` for an authored numbered field and its spelling for
an unnumbered field. A projected claim is linear even when its containing
aggregate is affine. Paths traverse only relevant structural fields; case,
index, scalar, erased, unknown, duplicate, overlapping ancestor/descendant, and
noncanonical rows reject. Direct Unit calls require the caller and callee to
agree on the complete ordered claim-path set for each structural argument, and
content-entry bindings must name that same root and field path. The interpreter
and verifier transfer those exact claims together; neither treats aggregate
custody as a Boolean property of the containing parameter.

One affine record argument may therefore carry several disjoint linear sibling
claims. Source checking retains every sibling, terminal production assigns a
dense machine-local claim identity to each one, and calls transfer the complete
canonical set to the callee. A successful bodyless boundary invocation carries
the verifier-derived completion-receipt set for all live claims attached to each
exact argument position, rather than assuming one claim per linear parameter.
Missing, duplicated, reordered, or path-mismatched receipt rows reject before
execution. The interpreter commits their consumption only after the provider
effect succeeds; rejection records no receipt and leaves custody live.

A stable record claim path may cross nested relevant record fields. Each
segment is resolved against the structural type reached by the preceding
segment, and the complete path remains canonical identity across production,
encoding, direct Unit transfer, interpretation, and boundary settlement. An
unknown inner field rejects, a caller/callee truncation is a custody-set
mismatch, and an ancestor claim cannot coexist with one of its descendant
claims.

The straight-line Unit return slice carries explicit no-code cleanup for owned
affine structural parameters that have no claim rows. The checked plan derives
the list from state-exit permission events in reverse parameter declaration
order. Terminal verification independently reconstructs the exact live affine
frontier, and rejects missing, extra, reordered, unknown, or claim-bearing
discards. Interpretation charges the return edge before removing those places,
so sponsor exhaustion cannot perform cleanup early. This is only trivial
parameter disposal: affine locals, nominal cleanup machines, partial values,
other edge kinds, and the whole-edge conservation witness remain outside the
slice.

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
scalar arguments, carries exactly one caller obligation identity for each
published callee `requires` clause, and explicitly records the normalized
no-successor crash continuations that survive at that invocation. Validation
checks the complete signature, argument definedness and types, result type,
obligation arity, global obligation uniqueness, and crash-continuation
coverage. Verification substitutes the positional arguments into the callee
requirements and guarantees: requirements become caller proof obligations,
while verified guarantees enter the caller's normal-return semantic axioms.

The call verifier accepts exact unconditional and guarded routes from an
in-module callee. Terminal crash predicates retain canonical proposition terms,
not producer-authored identity bytes. The verifier substitutes every callee
parameter `ValueId` with the corresponding arbitrary caller-local argument,
reconstructs the surviving continuation set, and requires coverage by the
caller's published ceiling; an empty or untranslated set therefore cannot erase
a crash. Checked scalar contracts and body crash sites retain structured
predicate meaning through terminal lowering. Invocation-specific guarded call
rows now retain that same structure after substituting direct parameter and
caller-local scalar arguments. Checked scalar graphs also retain direct
call-valued bindings, their exact call coordinate, and positional scalar
argument plans. Source production composes the reachable in-module checked
scalar call closure, consumes each matching crash row, and emits `Call` with
parameter or computed direct-local substitutions intact. Calls stage
short-circuit scalar arguments left-to-right and Omega target lowering accepts
the resulting calls inside conditional control. A guarded staged call follows
the checked row's pinned target contract and substitutes its parameter-relative
routes with the exact terminal argument values; it never reverse-matches caller
expressions, which would be ambiguous for equal or overlapping arguments. Wider
aggregate/member predicates and imported crash capsules remain fail-closed.
Structural/content contracts reject because custody effects require their own
vertical slice rather than an ordinary scalar flag.

The interpreter uses owned call frames and charges the call before entering the
callee. Sponsor exhaustion in the callee resumes without replaying that paid
call. A callee crash escapes as the original no-successor crash site and uses
that callee edge's fuel charge; call composition records the surviving route
without fabricating or double-charging another executable crash. Validation
rejects recursive call graphs until terminal Psi can carry and verify the
required tail-position and ranking evidence. Fixed-fuel derivation includes
separate acyclic callee return/crash bounds: caller tails compose only with
normal returns, while callee crash paths terminate at their own edge. It retains
its own cycle rejection as defense in depth.

Omega selects each callee's native calling plan and evaluates arguments into
disjoint frame spills before filling their ABI homes. Assignment retains
explicit register or outgoing-stack destinations. Emission materializes the
complete outgoing area, including Microsoft x64 shadow space, preserves x86
call alignment and the AArch64 link register, and emits a typed internal-call
relocation tied to the exact Psi operation and callee. Conditional-control
emission preserves live entry registers across condition calls and rebases
relocations from independently encoded conditions and arms into final function
order.

An unconditional crash continuation requires no caller-side machine-code
branch: the verified internal call reaches the emitted callee crash leaf and
cannot return along that execution. Omega still resolves the typed call
relocation and preserves the callee leaf; it does not reinterpret a crash as a
scalar result.

### Value-less normal return slice

A terminal machine result is either `Scalar`, with one stable result
pseudo-value, or `Unit`, with no runtime value at all. `ReturnUnit` is a normal
exit, not a distinguished Boolean or integer: it creates no `ValueId`, result
structural place, or return-equality axiom. Contracts on a unit machine may
refer to its parameters but cannot name an absent result. The scalar `Call`
operation cannot target a unit machine; unit calls require their own complete
operation slice rather than an ignored result convention.

Canonical encoding, independent verification, interpretation, and fixed-fuel
derivation implement this distinction. A unit return charges exactly its one
terminal edge and resumes atomically after sponsor exhaustion. The checked-tree
producer and Omega native lowering remain explicitly scalar-only, so this is
artifact-core scaffolding rather than a source-visible unit-entry or Cathedral
hard-root claim. Attached roots, linear custody, provider/port effects, and
native unit realization remain gated on their complete vertical slices.

Normal scalar returns carry the exact canonical list of live unclaimed affine
parameters to discard; the list is empty when no cleanup is required.
Verification reconstructs that list in reverse parameter declaration order.
Interpretation charges the return edge and materializes the scalar result before
performing these no-code discards, so sponsor exhaustion cannot partially commit
the exit. Omega consumes the verified cleanup metadata without emitting a target
instruction. The current scalar source producer has primitive-only signatures
and therefore emits an empty list; mixed structural/scalar production remains a
separate vertical slice.

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

The verifier infers identity-preserving reshuffles. It validates canonical
partition-composition rows and replays their exact substitutions, but those
producer-carried rows are not semantic axioms by themselves. A following
vertical slice must bind each composition to the exact operation and authored
callee guarantee, then introduce the verifier-reconstructed theorem only on
that operation's successful path. Fingerprints identify canonical content for
reporting and caches; they never authorize a theorem. At a bodyless partial
boundary, Psi derives the kept content and residual and permits the provider to
admit only acceptance of custody for that exact residual—not the partition
arithmetic. External root correspondence and fresh issuance remain scoped
admitted hypotheses with provenance; downstream conservation remains derived.

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

For effectful Unit roots, the payload also retains the canonical function map,
each privileged port effect's exact service/operation/byte range, and each
bodyless settlement's exact admitted provider-execution binding and immediately
preceding effect realization. A settlement emits no duplicate hardware effect;
object and installation validation reject missing, reordered, byte-drifted, or
raw-number-only realizations. Production construction consumes the same
ledger-owned `ProviderExecution` values used by target lowering and requires
their closure to match the emitted settlements exactly; decoded payloads remain
non-authoritative audit projections. Both native lanes stage structural
parameters into aligned owned entry homes before effects or calls. AAPCS64 Unit
calls preserve `x30`, keep `sp` 16-byte aligned, marshal direct register and
stack fragments, and create the normalized caller copy for indirect by-value
aggregates before passing its address.

Native Unit artifacts and the canonical installation payload retain one
logical-fuel attribution row for every emitted operation and return edge:
exact current schedule, semantic site, units, operation ordinal,
function-relative byte offset, and byte count.
Metadata-only settlement rows deliberately have a zero-byte interval. This is
the provenance input to future sponsor-owned inserted metering, not evidence
that runtime charging already occurs and not a native instruction-cost model.

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
outcome-specific bound of each reached callee. A callee crash does not acquire
the cost of an unreachable caller tail; a caller segment ending after a call
uses only the callee's normal-return bound. Entry and segment certificates bind
the exact terminal identity, schedule, endpoints, and ceiling; validation
reconstructs those fields and the complete reachable segment partition. Ranked
tail calls, loops, and relevant-precondition refinements require later vertical
slices.

Omega may use a certificate only for the exact installed terminal bytes,
architecture, entry stub, and external-root context it names. Recomputable Psi
fuel evidence carries no provider receipt.

`omega inspect-terminal --machine <qualified>` verifies the selected terminal
closure and proof bundle, recomputes and validates its acyclic entry
certificate, and publishes the exact terminal identity, schedule, entry, and
ceiling. This is build-time semantic evidence, not installed-root evidence:
the native terminal Unit and branch-free scalar slices now retain exact
emitter evidence that object construction validates and replays to derive
local peaks plus caller-live stack at each typed internal-call relocation and
compose that acyclic closure below function entry. Conditional scalar control
flow is limited to one exact two-terminal-arm shape: a top-level Boolean
parameter or linear Boolean-expression condition with two direct linear
integer returns. The balanced expression prefix and both arms replay
independently and compose by maximum; exact x86 flag-preserving release and
AArch64 expression-branch encodings are validated. Typed scalar calls in the
prefix or either arm reuse the exact call evidence and closure composition.
Nesting, reconvergence, crashes in arms, division/remainder expressions,
external adapter/interrupt-arrival state, other terminal function forms,
provider admission, and exact installation binding are not yet part of that
theorem, so the
inspection surface still makes no installed-root WCSU claim.

## Implementation queue

[`TASKS.md`](../../../TASKS.md) owns remaining terminal-Psi work. Temporary
differential paths may coexist as test oracles while consumers move; they are
not alternate language versions or a permanent Omega-to-Psi path.
