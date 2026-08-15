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

## Checked-adapter provider installation

A static bodyless boundary call keeps its boundary-machine ID in terminal Psi;
it is not rewritten to a chosen implementation. For the currently admitted
zero-argument Unit slice, Psi serializes every exact checked satisfier as an
ordinary terminal machine plus a canonical conformance row. The row binds the
boundary requirement identity, nominal provider identity, canonical adapter
identity, artifact-local machine ID, Unit signature, and checked service
refinement. Structural parameters, domain requirements, stateful provider
values, and completion receipts remain outside this slice.

Provider selection is not terminal-Psi semantic identity. Omega consumes its
retained `SelectedProviderPlanFacts`, resolves each selected `CheckedAdapter`
by exact overload, provider type, and adapter identity against the verified
catalog, and asks Psi to admit only those terminal IDs for the exact artifact.
The Psi interpreter follows a cataloged boundary only through that explicit
private-field installation; absence fails closed instead of falling through to
an external effect handler.

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
The current producer serializes forwarded terms as dense module-local
identities over the exact proposition application and a structured canonical
carrierless interface; the verifier requires each witness application to carry
that interface and each term row to agree with it. A forwarded output
contributes only its source vocabulary identity. Canonical positional rows for
the selected terminal machine's named `requires` and `ensures` lanes reference
those exact IDs, and forwarding places the same ID at both endpoints. The
verifier requires known machine/term IDs, dense positions per lane kind, and no
orphan term rows. A fresh ensured term is accepted only when the proof bundle
contains one canonical provenance row keyed to that exact term. The row has its
own proof identity and retains the selected conformance, evidence trait, and
complete normalized realization rows without source handles. Missing, unused,
malformed, reordered, or interface-mismatched provenance rejects. The row
changes the proof fingerprint, not terminal semantic identity, runtime, or
fuel. Each ensured lane also retains its public generated-package field name
beside the exact `EvidenceTermId`; required lanes have no output field, `value`
remains reserved for the ordinary runtime result, and missing or duplicate
names reject. The retained carrierless interface includes its complete direct
and inherited requirement surface, including each declaring trait's normalized
argument pack. A proof-static projection carries the canonical evidence-term
ID plus the exact declaring-trait application and requirement-overload
identity. Forwarding is canonicalized before applications are serialized, so
input and output aliases project the same opaque identity while separate terms
remain distinct. The verifier requires the term and exact row to exist in the
retained interface; diagnostic display spelling is never an identity oracle.
The immediate generated-package rung adds a dense invocation table retaining
the canonical caller and ordinal, normalized callee-machine identity, and the
complete nonempty output set in callee-lane order. Each output binds its public
field and dense position, one callee declaration, and one distinct caller-local
term with the same proposition and interface. Repeated calls share callee terms
and producers while minting fresh caller terms. Source coordinates, source
field order, and diagnostic paths erase. A proof-only row adds no runtime
operation or fuel. A scalar-runtime row separately retains its scalar result
type and the exact canonical ordinary `Call` operation ID and callee. The
verifier requires that operation to occur in the declared caller, produce the
declared scalar type, and call the linked callee; a missing, spurious, unknown,
wrong-kind, wrong-caller, or mismatched-callee link rejects. The proof row adds
no operation or fuel beyond that ordinary call. Retained/general projection and
guarded complete-package forms are design-blocked on `OWNER_QUESTIONS.md` Q14.
Generic package application is blocked on Q12, and explicit discard is blocked
on Q13.

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

An entry claim may name its complete structural parameter or a typed path below
it. Record segments use the field's exact canonical identity: `#<id>` for an
authored numbered field and its spelling for an unnumbered field. Literal array
segments carry their canonical zero-based index and resolve only through a
nonempty literal-length fixed-array shape. A projected claim is linear even
when its containing aggregate is affine. Paths traverse only relevant
structural fields or in-range fixed indexes; cases, dynamic indexes, scalar or
erased leaves, unknown segments, duplicates, overlapping ancestor/descendant
rows, and noncanonical order reject. Direct Unit calls require the caller and callee to
agree on the complete ordered claim-path set for each structural argument, and
content-entry bindings must name that same root and typed path. The interpreter
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

The admitted result-bearing slice returns one primitive scalar from a bodyless
boundary while consuming one or more whole structural roots. Its call result,
boundary signature, arguments, and exact receipts survive canonical encoding
and independent verification. Interpretation checks the provider's returned
scalar before committing either custody or receipts, so a rejected call can be
retried against the unchanged frontier. Omega preserves that result in its
abstract plan. An admitted x86-64 `u8` port-read provider lowers to a sealed
instruction interval and returns the byte through the scalar ABI; its provider
identity, whole-root arguments, receipts, and exact bytes survive object,
image, and installation validation. Other result shapes and targets, plus
projected and content-bearing result calls, fail closed.

A stable record claim path may cross nested relevant record fields. Each
segment is resolved against the structural type reached by the preceding
segment, and the complete path remains canonical identity across production,
encoding, direct Unit transfer, interpretation, and boundary settlement. An
unknown inner field rejects, a caller/callee truncation is a custody-set
mismatch, and an ancestor claim cannot coexist with one of its descendant
claims.

The indexed source slice accepts one nonempty literal fixed array of linear
structural elements with the complete dense sibling claim set. One literal
element may pass either to a bodyless Unit boundary or through an ordinary Unit
call whose caller and callee each have exactly one structural parameter and no
scalar parameters. The callee accepts one unqualified whole-root claim and no
contract clause over that parameter. Verification rebases the selected claim;
interpretation retains every sibling until its own successful settlement.
Omega realizes the internal call on all five targets and carries its exact
path, type, layout, copy bytes, and claim transfer through installation.
Nested/dynamic indexes, wider signatures, projected contracts, content-bearing
partitions, partial returns, and aggregate construction remain fenced.

The claim-free partial-cleanup slice accepts one affine transparent record. A
finite nonempty set of pairwise prefix-disjoint, nonempty all-field paths may
move through source-ordered one-parameter ordinary Unit calls, provided at
least one residual subtree remains. The Unit return then names every maximal
live residual subtree by exact root, canonical field path, and subtree type in
recursive reverse declaration order and never discards a partially moved
ancestor whole. The verifier independently proves that the moved and residual
paths are disjoint and exhaust the root. Interpretation
charges the return edge before disposing the residual paths, so fuel exhaustion cannot
clean early. Omega carries every path and type through all five target
pipelines, object/image validation, and canonical installation records while
emitting no cleanup instruction or runtime bitmap. Claims, content, contracts,
nominal `drop`, and arrays/cases remain fenced.

The straight-line Unit return slice carries explicit no-code cleanup for owned
affine structural parameters that have no claim rows. The checked plan derives
the list from state-exit permission events in reverse parameter declaration
order. Terminal verification independently reconstructs the exact live affine
frontier, and rejects missing, extra, reordered, unknown, or claim-bearing
discards. Interpretation charges the return edge before removing those places,
so sponsor exhaustion cannot perform cleanup early. A one-state Unit/effect
body may also begin with a finite source-ordered run of immutable, unqualified,
empty-record affine locals. Each has an explicit fuel-charged establishment;
the return discards locals in reverse order before eligible parameters. Their
typed custody crosses Omega's five native artifact pipelines without runtime
bytes. Nonempty, mutable, qualified, content-bearing, nominal-cleanup, and
post-effect locals remain fenced.

The nominal-cleanup slice accepts one root-only, one-state Unit machine with a
finite nonempty list of claim-free, unqualified affine parameters whose records
are empty or contain only relevant Terminal-supported Boolean/integer fields, plus
their exact attached `T::drop(&mut self)` machines. One cleanup may be empty or
contain a finite source-ordered list of ordinary zero-argument calls to mutually
distinct exact-empty attached helpers. Multiple cleanups run in reverse parameter
declaration order; every body may use that executable form, including a shared
cleanup target or helper. Repeated use of the same cleanup machine remains legal
because each action names a distinct place. The
return carries the ordered whole-place/type/machine list. Verification
reconstructs its exact deduplicated machine closure; interpretation charges the
caller edge once and executes each cleanup sequentially; fixed fuel counts
every invocation, including a repeated target. Omega carries all return
cleanup kinds in one ordered action stream through abstract, target, assigned,
machine, object, image, and installation artifacts. Empty drops emit no native
call; an executable body emits a call owned by the exact edge/action ordinal
before teardown, with source-ordered operation-owned helper calls.
For an empty cleanup body or the bounded receiver-independent helper-call body,
a finite canonical set of direct Boolean-field clauses in either polarity is
accepted when the caller's supported
Boolean fact set contains every corresponding requirement on each owned root.
Caller-only facts remain in the entry contract. Terminal Psi retains a
target-local proof-only receiver, shared by actions using the same target, and
one positional edge obligation per action and cleanup clause. It substitutes
that receiver with the owned cleanup place during
independent verification, and binds each proof to its matching caller
assumption rather than assuming identical set positions. The cleanup target
remains operationally zero-argument. The verified Psi-to-Omega boundary
removes those proof-site identities; every downstream Omega validator rejects
their reintroduction. Missing caller evidence rejects during checked
production. Wider predicates, bodies that can inspect or change receiver facts,
nested/erased receivers, claims, qualifications, locals, and non-root edges
remain fenced.

An unconditional jump and each ordered conditional successor may carry an
independent canonical reverse-declaration subset of the same eligible
parameters. Verification removes exactly those places from the corresponding
successor frontier; interpretation charges the selected edge and materializes
its scalar arguments before committing the no-code disposal. The primitive-only
scalar source producer emits canonical empty lists. Checked facts now retain an
exact source-state, transition-statement, and target-state row for each
supported structural jump or conditional arm, together with the
reverse-declaration positions of its claim-free affine parameter discards.
States needing affine-local, projected, nominal, or claim-bearing cleanup fail
closed without a partial row. The first structural-control producer consumes a
composed checked plan for attached, multi-state, Unit-returning machines whose
states contain only claim-free affine structural parameters and either return
naturally, contain one unconditional ordinary local jump, or select two ordered
ordinary successors from one retained Boolean scalar input. At most two states
may select successors, so an unconditional prefix and one nested decision are
accepted while a third conditional state remains fenced. Whole-parameter
arguments provide exact type-preserving transfer maps; each map and its exact
cleanup row must independently partition the source frontier. Production
resolves checked parameter positions against the source-handle-free state
signatures. One acyclic two-predecessor join may reconverge when both paths
reconstruct the identical ordered structural frontier. Scalar arguments remain
ordinary typed edge bindings and need not be the same values. A divergent
custody map, second join, third predecessor, or cycle rejects. Unconditional
jumps and conditional arms may additionally pass
direct primitive scalar inputs into typed successor block parameters; the edge
materializes those arguments before committing its structural cleanup.
Production emits the resulting jump/conditional/return blocks and rejects stale
scalar or structural signatures, arm order, or cleanup. This slice admits only reachable,
acyclic custody lineages whose surviving place order remains canonical. Wider
joins, cycles, reordering, computed guards or successor values, locals, and richer
cleanup continue to fail closed. The terminal verifier remains responsible for
reconstructing every emitted cleanup frontier and scalar edge binding.

The first nonempty scalar-return source path composes the same cleanup evidence
with an attached, one-state signature containing only claim-free affine
structural parameters. Its scalar work is an ordered prefix of immutable
primitive locals followed by one return expression. Every initializer and the
return use checked scalar expressions: explicitly landed integer literals,
terminal integer operations and casts, Boolean constants, negation, equality,
comparisons, and references to already materialized scalar locals. Initializers
are branch-free except for the repeated Boolean continuations below. State
parameters are partitioned explicitly: primitive inputs receive dense scalar
positions plus retained authored positions, while affine custody retains its
separate structural positions. Their authored-position maps must be disjoint and
complete. Locals follow the scalar inputs in the value namespace. The checked
row carries that partition, the exact structural signature, local types and
statement coordinates, scalar result carrier, return coordinate, and
reverse-declaration cleanup positions. Production revalidates the partition and
the dense scalar/local namespace, materializes expressions in order,
reconstructs any exact-operation proofs before the return edge, and resolves
cleanup positions to structural places. A final short-circuit Boolean return is
expanded into explicit decision blocks: internal conditional edges preserve the
unchanged structural frontier, and every terminal value leaf carries the same
checked complete cleanup list. The verifier reconstructs that requirement on
each path. Any finite sequence of short-circuit Boolean locals is also accepted
within an otherwise branch-free primitive binding sequence: prefix values
dominate the first decision tree, each tree's leaves jump without cleanup to one
typed Boolean convergence parameter, and branch-free work in that continuation
may lead to the next local tree or to the return expression. That final return
may itself be a short-circuit Boolean tree; every one of its value leaves then
performs the same complete cleanup.
Calls, mutable or non-scalar locals, contracts beyond the bounded premises
described below, claims, effects, and multi-state control remain outside this
source slice;
structural custody is never represented as a scalar parameter.
One narrower nominal branch admits a finite nonempty list of direct affine
structural parameters that may mix no-code and nominal roots, a finite set of
direct primitive scalar inputs interleaved at authored parameter positions,
and no authored contract beyond a combination of the direct-Boolean contextual
subset and direct unsigned scalar-parameter upper bounds described below,
plus a finite source-ordered prefix of immutable branch-free primitive locals
and either one branch-free scalar result or a finite Boolean continuation chain
that begins with a finite `&&`/`||` decision tree of arbitrary nesting. Every
later local in that chain is branch-free or another finite nested decision
tree over the inputs and available locals, and it uses its immediate Boolean
predecessor at least once; the return directly names the final local. Checked
plans retain the complete authored parameter partition; terminal Psi gives scalar
values and structural places independent dense namespaces. Terminal production
materializes the input-dependent local and result operations in
source order, then executes the complete cleanup stream in reverse authored
root order. No-code roots retain their exact position without invoking a
machine; nominal targets may be distinct or shared, and each drop may be empty
or contain the bounded source-ordered zero-argument helper-call body accepted
by the Unit nominal slice. For the finite Boolean form, terminal production
retains a branch-only decision tree with distinct return edges and attaches the
same complete cleanup stream to every leaf. Terminal production retains the
cleanup targets and helpers in the same closed module. Contextual cleanup requirements are accepted
for a finite mixed root list in the same direct-Boolean subset as Unit cleanup.
Checked production binds every target premise to the exact nominal caller root
and retains supported caller-only facts on no-code roots; terminal Psi carries
canonical caller requirements, proof-only receivers, and distinct action
obligations. Omega consumes those facts only after verification and projects
the proof metadata away before target lowering. Native lowering preserves the
computed ABI result and, on AArch64, the return link across executable cleanup
calls in an exact lifetime frame; object construction validates the frame,
stores, loads, calls, and stack ceiling from emitted bytes. The finite Boolean
form instead retains one edge-specific cleanup interval per surviving native
leaf and validates the result and return-link lifetime independently on every
native path. Terminal production decides every short-circuit local once per
stage, substitutes each resulting value leaf into the continuation, and
source-distributes branch-free work and later decision stages without a
convergence block. One bounded exception accepts a finite `!`/`&&`/`||` tree
over a finite nonempty set of runtime Boolean parameters and constants. Boolean
equality with a constant normalizes to the same identity/negation leaves. Every
typed value leaf jumps to one terminal-Psi Boolean parameter and one shared
cleanup return. Omega retains the source-ordered decisions, an unconditional
join branch from every non-final leaf, and final-leaf fallthrough into one
physical cleanup tail on every target; object construction reconstructs the
decision regions, decodes every join, and replays the shared tail before image
and installation custody. That exception also admits one canonical direct
relevant Boolean field identity from one claim-free affine nominal-cleanup root,
combined with those parameters and constants. At least one Boolean parameter
must remain in the tree so native expression scratch cannot overwrite the
structural source. Terminal Psi names the exact source place and field ID;
verification reconstructs that field from the entry type, and interpretation/
native lowering read the exact structural ABI field without treating opaque
identity as layout. Machine-code evidence binds every such read to its exact
native interval. Object replay independently resolves the claimed source home,
reconstructs the canonical Boolean field and ABI offset from the retained
structural type closure, replays the live stack depth, and requires the
architecture-specific load and normalization bytes before image and
installation custody. Separately, direct integer comparisons whose
operands are scalar parameters or landed constants, optionally beneath up to
two total bitwise-not, binary bitwise, wrapping shift/arithmetic, saturating
arithmetic, or integer-widening shells, or one exact fixed-width narrowing,
same- or cross-sign, under retained direct parameter range `requires`, or exact
fixed-width addition with a landed operand, subtraction with a landed
subtrahend, or multiplication with a landed factor under retained matching
direct parameter bounds, runtime unsigned multiplication under retained
`1 <= right` and `left <= MAX / right` bounds, or runtime signed multiplication
under a retained positive or at-most-`-2` factor bound and both carrier-tight
quotient bounds, one runtime unsigned addition under the retained canonical
`left <= MAX - right` bound, one runtime signed addition under matching sign and
`MAX - right` or `MIN - right` bounds, runtime subtraction unsigned under a
retained direct subtrahend-to-minuend bound or signed under matching sign and
`MIN + right` or `MAX + right` bounds, one exact
right shift under a direct upper count bound for unsigned counts or direct
zero-lower and upper bounds for signed counts, one exact left shift by a landed
count or runtime count under the verifier-required direct value bounds and, for
runtime counts, direct count upper bounds plus a direct zero lower bound when
that count is signed, or exact division/remainder by a landed nonzero unsigned
constant, a landed signed constant other than `0` or `-1`, a runtime unsigned
divisor under a direct positive lower bound, or a
runtime signed divisor under a direct positive lower
bound, `divisor <= -2` upper bound, or joint `divisor <= -1` and
`MIN + 1 <= dividend` bounds, may form decision leaves. Psi retains every exact
operation; one proof-bearing exact operation may also appear as the innermost
operation beneath up to two bitwise-not, integer-widening, or proof-free binary
shells, and distinct binary subtrees may each contain one independently proved
exact leaf. A finite same-carrier exact-add chain may instead have a direct
machine-parameter root when every non-chain sibling is a landed literal
constant. A finite same-carrier exact-subtract chain may likewise have a direct
machine-parameter root, but only the left operand continues the chain and every
right operand is a landed literal constant; reversed subtraction is not a
chain. A finite same-carrier chain may mix exact addition and subtraction when
both operation kinds occur. It continues only through each left operand from a
direct machine parameter, every right operand is a landed literal of that same
carrier, and the verifier combines additions and mathematical negations of
subtrahends in the same checked sign/magnitude offset accumulator. Every prefix
reconstructs its carrier-tight direct-root bound independently; a later
cancellation does not authorize an unsafe earlier link. A finite same-carrier
exact-multiply chain may also continue only through its left operand from a
direct machine parameter. Every right operand must be
an explicitly landed literal of that same carrier and nonnegative; zero and one
are admitted, while signed negative factors are not. A finite same-carrier chain
may also mix exact divide and remainder,
continue only through its left operand from a direct machine parameter, and use
only landed nonzero unsigned divisors or landed signed divisors other than `0`
and `-1`. For addition, subtraction, their mixed chain, and multiplication, the
verifier walks only prior left-to-right definitions with a shrinking prefix.
Addition/subtraction combine constants or mathematical negations of subtrahends
as a checked sign and magnitude and reject accumulator overflow or a magnitude
beyond the carrier span. Multiplication combines only same-carrier nonnegative right factors in a
checked `u128` accumulator. Cumulative factor zero or one is total; a larger
unsigned factor reconstructs `root <= MAX / factor`, and a larger signed factor
reconstructs both `MIN / factor <= root` and `root <= MAX / factor`. Every
earlier multiply retains its own independently checked obligation, so a later
zero never authorizes an unsafe earlier link. One unified affine-chain family
admits a finite left-associated same-carrier chain containing both at least one
exact add/subtract and at least one exact multiply. It starts at one direct
machine parameter; every right sibling is an independently landed same-carrier
literal, and multiply factors are nonnegative. For each shrinking prefix the
verifier walks ordered definitions and replays `A * root + B`: addition and
subtraction adjust the checked signed offset `B`, multiplication checked-scales
both the nonnegative coefficient `A` and `B`. If `A > 0`, the verifier maps the
carrier interval back with mathematical ceiling/floor division and intersects
it with the root carrier. If `A == 0`, the current obligation is true exactly
when `B` is carrier-representable. Every earlier prefix remains independently
proved, so later zero factors or cancellation supply no authority. Homogeneous
chains continue to use their narrower existing families. A finite
same-value-carrier exact-right-shift
chain may also continue only through its left operand from a direct machine
parameter. Every right operand must be a landed literal in one of the current
fixed native signed or unsigned integer count carriers and independently satisfy
`0 <= count < value width`; count carriers may differ between links. Each
divide/remainder or right-shift obligation reconstructs independently from its
own safe landed divisor or count, so no producer-definition traversal supplies
authority. A finite same-value-carrier exact-left-shift chain may also continue
only through its left operand from a direct machine parameter. Each right
operand must likewise be an independently landed in-range fixed native integer
count, and count carriers may differ. The verifier follows only prior
left-to-right definitions with a shrinking prefix, accumulates counts in a
checked `u128`, and reconstructs every link from the cumulative count: zero is
total; `0 < cumulative < width` requires `root <= MAX >> cumulative` for
unsigned roots and `MIN >> cumulative <= root <= MAX >> cumulative` for signed
roots; cumulative counts at least the width require the root to equal zero.
One mixed shift family admits any finite left-associated same-carrier chain
containing both exact-left and exact-right shifts from a direct machine
parameter. Every count is an independently landed legal fixed-native literal;
count carriers may differ. Each right-shift proof remains only its own legal
count proposition. For each left prefix the verifier starts with that
operation's carrier-tight safe input interval and walks every prior canonical
mixed-shift definition backward: a prior left shift maps `[a,b]` to
`[ceil(a/2^k), floor(b/2^k)]`, while a prior arithmetic or zero-fill right shift
maps it to `[a*2^k, (b+1)*2^k-1]`; each step intersects the value carrier.
Empty intervals reject, full intervals are true, and surviving intervals become
canonical direct-root bounds. Every operation keeps distinct evidence, so a
later right shift cannot erase an unsafe earlier left prefix. Homogeneous shift
families remain on their existing paths. Runtime, computed,
negative, out-of-range, address, or non-native counts, mixed value carriers,
local, block, computed, or nested-cast roots, intervening shells or operations,
right-associated shapes, malformed, reordered, cyclic, redirected, or stale
definitions, interval overflow, and stale or missing evidence remain fenced.
The same mixed-only chain may feed one validator-legal partial fixed-native
exact cast. The cast proof starts from the intersection of the target and
source carriers, then walks every canonical mixed-shift definition backward
with the same inverse-left and inverse-right transfers. It emits only the
surviving canonical direct-root interval. A mathematically empty preimage is
canonical falsehood; checked interval-arithmetic failure is no admission.
Every shift-prefix proof and the cast proof remain distinct, and homogeneous
shift-to-cast chains continue to use their existing narrower paths.
Conversely, one validator-legal partial fixed-native cast of a direct machine
parameter may root the same mixed-only finite chain in the target carrier. For
each left prefix the verifier walks the shrinking canonical definitions back to
the cast, applies the same inverse-left and inverse-right transfers, intersects
the surviving target interval with the source carrier, and emits only canonical
source-root bounds. The direct cast proof, every left-prefix proof, and every
right count proof remain independently mandatory. Mathematical emptiness is
canonical falsehood; checked transfer failure admits no family. Homogeneous
post-cast shift chains stay on their narrower existing paths.
One unified cross-family composition admits a finite nonempty left-associated
same-carrier arithmetic prefix over exact add, subtract, and nonnegative
multiply by landed same-carrier literals, followed by a finite nonempty shift
suffix with independently landed heterogeneous legal counts and at least one
exact-left shift. For every left prefix the verifier first maps its safe input
interval backward through the prior canonical left/right shift definitions,
then composes the arithmetic definitions as checked `A * root + B` and maps the
surviving interval back to the direct machine parameter. `A == 0` decides only
that left-prefix proposition from `B`; every arithmetic-prefix proof and every
shift proof remains independently mandatory. Mathematical emptiness is
canonical falsehood, while checked affine or interval transfer failure admits
no family. Right-only suffixes, runtime or computed siblings/counts, negative
factors, non-left-associated shapes, casts, shells, and non-parameter roots
remain fenced.
The converse composition admits a finite nonempty left-associated exact-shift
prefix followed by a finite nonempty same-carrier add/subtract/nonnegative-
multiply literal suffix. Every count remains independently landed and every
left-shift overflow and arithmetic-prefix obligation remains distinct. For each
arithmetic prefix the verifier composes checked `A * shifted_root + B`, maps the
carrier backward through that affine form, then replays the complete ordered
shift prefix to the direct machine parameter. `A == 0` decides only the current
arithmetic proposition after the complete canonical root shape is validated;
it cannot erase any earlier proof. Mathematical emptiness is falsehood, while
checked affine or interval transfer failure admits no family. Runtime or
computed siblings/counts, negative factors, reversed or right-associated
shapes, casts, shells, and non-parameter roots remain fenced.
Terminal retains every operation and obligation, and every
operation's evidence is checked independently. Two computed operands,
nonconstant siblings, runtime or computed multiply factors or shift counts,
signed negative multiply factors, right-associated or reversed shapes, local or
block-parameter roots, exact operations outside the admitted chain family, and
other proof-bearing compositions remain fenced. For addition, subtraction,
their mixed offset chain, multiplication, the mixed affine chain, and left
shift, missing, reordered, reversed, redirected, cyclic, or stale definitions
reject. The affine family additionally rejects coefficient or offset
composition overflow. For every family, stale
operation/factor/divisor/count evidence and missing evidence reject. Multiply
and left shift additionally reject cumulative arithmetic
overflow. One
separate computed-cast exception accepts a direct
fixed-integer parameter
widened through any finite chain of valid fixed-carrier widenings and then
exactly narrowed back to its original carrier. Terminal retains every ordered
`IntegerWiden` and the `IntegerExactCast`. The verifier walks only prior
left-to-right value definitions, reduces the available definition prefix at
every step, checks every adjacent carrier and strict widening, and requires the
origin value to be a machine signature parameter of the narrowing's target
carrier. The walk is bounded by the finite prior-axiom count; missing,
reordered, reversed, cyclic, mistyped, or redirected definitions reject the old
self-proof. Local or block-parameter roots and otherwise computed exact casts
remain fenced. A second computed-cast exception accepts
one partial exact conversion whose operand is a finite nonempty left-associated
same-carrier exact-add/subtract chain. The chain uses a current fixed-native
source carrier, starts at one direct machine parameter, and has one independently
landed same-carrier literal on every right edge. The target is another current
fixed-native carrier; same-width and cross-sign partial conversions are
included. The verifier follows only ordered shrinking-prefix definitions,
accumulates additions and mathematical negations of subtrahends in the checked
sign/magnitude offset, and reconstructs the cast as the target interval shifted
back by that offset and intersected with the source carrier. Vacuous sides are
omitted, an empty intersection is false, and one or two surviving bounds are
canonical source-carrier propositions. Every arithmetic prefix retains its own
obligation and evidence, so cancellation or a cast-safe final interval cannot
erase an earlier unsafe operation. Computed or unlanded siblings, literal-left
addition, reversed subtraction, right-associated shapes, local or block roots,
mixed-carrier or non-native chains, other proof-bearing operations, additional
casts, missing or noncanonical definitions, accumulator overflow, and stale or
missing evidence remain fenced. A third computed-cast exception accepts one
validator-legal partial fixed-native exact cast whose operand is a finite
nonempty left-associated same-source-carrier exact-multiply chain. The chain
starts at one direct machine parameter and every right operand is an
independently landed nonnegative source-carrier literal. Every multiply prefix
retains its ordinary independent obligation and evidence. For the cast, the
verifier follows only prior canonical shrinking-prefix definitions, accumulates
the factors in a checked `u128`, maps the target range back through the
cumulative product, and intersects it with the source carrier. Product zero
makes only the cast obligation true. Product one uses the ordinary target/source
intersection. A larger product reconstructs `[0, MAX / product]` for an unsigned
target or `[ceil(MIN / product), floor(MAX / product)]` for a signed target
before the source-carrier intersection; vacuous sides are omitted and an empty
intersection is false. Literal-left or right-associated shapes, runtime,
computed, negative, or mistyped factors, mixed carriers, local or block roots,
intervening operations or casts, non-native or invalid casts, malformed or stale
definitions, cumulative-product overflow, and stale or missing evidence remain
fenced. A later zero cannot erase an earlier multiply proof. A fourth
computed-cast exception accepts one validator-legal partial fixed-native exact
cast whose operand is a finite nonempty left-associated same-source-carrier
exact-left-shift chain rooted at one direct machine parameter. Every right
operand is an independently landed legal fixed-native count, and count carriers
may differ. The verifier follows only prior canonical shrinking-prefix
definitions, checked-adds the counts, maps the target interval right by the
cumulative count, and intersects it with the source carrier without importing
any shift-prefix evidence. Count zero uses the ordinary target/source
intersection. A positive count below the source width reconstructs
`[0, MAX >> count]` for an unsigned target or
`[ceil(MIN / 2^count), floor(MAX / 2^count)]` for a signed target before the
source intersection. At or above the source width, the cast alone is true
because any successfully produced exact source result is zero; every shift
prefix still retains its independent carrier-safety or zero-root proof.
Runtime, computed, negative, out-of-range, address, or non-native counts,
right-associated shapes, mixed value carriers, local or block roots,
intervening operations or casts, non-native or invalid casts, malformed or
stale definitions, cumulative-count overflow, and stale or missing evidence
remain fenced. A fifth computed-cast exception accepts the corresponding finite nonempty
same-source-carrier exact-right-shift chain. Counts, root, definition walk, and
fences match the pre-cast left-shift family, but every shift-prefix obligation
remains only its independent legal-count proof. For cumulative count `C` below
the source width, with `Q = 2^C`, the cast maps target interval `[L, U]` back to
`[L*Q, (U+1)*Q-1]` and intersects the result with the source carrier. At or
above source width an unsigned source yields zero; a signed source yields
`-1` or `0`, so the cast is true for a signed target and requires `0 <= root`
for an unsigned target. No shift proof is imported into the cast reconstruction.
A further computed-cast family accepts a finite nonempty left-associated
same-source-carrier exact-divide/remainder chain rooted at one direct machine
parameter when its result is carrier-total for the partial cast. Every right
sibling is an independently landed same-carrier safe divisor. The verifier
walks only prior canonical shrinking-prefix definitions, then replays them
inner-to-outer from the full source-carrier interval: toward-zero division maps
endpoints monotonically (reversing them for a negative divisor), while
remainder uses the dividend-sign interval hull clipped by `abs(divisor) - 1`.
The family is retained only when the final hull lies wholly inside the target
carrier. No guard-sensitive or nonconvex preimage, operation proof, or evidence
is imported into the cast; every divide/remainder prefix and the cast retain
independent evidence. Noncontained hulls, zero, signed `-1`, runtime, computed,
or mistyped divisors, literal-left or right-associated shapes, mixed carriers,
local or block roots, intervening operations or casts, non-native, identity,
widening, or invalid casts, malformed, stale, or out-of-order definitions,
interval arithmetic failure, and stale or missing evidence remain fenced.
A further computed-cast exception accepts the unified finite left-associated
same-source-carrier mixed affine chain described above when it contains both an
exact add/subtract and an exact multiply. The cast is validator-legal and
partial, the root is one direct machine parameter, every right sibling is an
independently landed same-carrier literal, and multiply factors are
nonnegative. The verifier follows only prior canonical shrinking-prefix
definitions and replays the full operand as `A * root + B` with checked
coefficient and offset composition. For `A > 0`, it maps the target carrier
back to `[ceil((TARGET_MIN-B)/A), floor((TARGET_MAX-B)/A)]` and intersects that
interval with the source carrier. For `A == 0`, only the cast is true exactly
when `B` is target-representable. No arithmetic-prefix proof is imported into
cast reconstruction, so later zero or cancellation cannot erase an earlier
obligation. Homogeneous chains remain on their narrower computed-cast paths.
Literal-left, reversed, or right-associated shapes, runtime, computed,
negative, or mistyped factors/siblings, mixed or non-native carriers, local or
block roots, intervening shells, operations, or casts, invalid or widening
casts, malformed or stale definitions, coefficient/offset overflow, and stale
or missing evidence remain fenced.
Conversely, one
validator-legal partial fixed-native exact cast of a direct machine parameter
may root a finite
nonempty left-associated same-target-carrier exact-add/subtract chain. The cast
result is the innermost left operand, and every right operand is an
independently landed target-carrier literal. The cast retains its ordinary
direct source-to-target representability obligation. For every arithmetic
prefix, the verifier walks only prior canonical shrinking-prefix definitions
through the chain to the cast, accumulates additions and mathematical negations
of subtrahends with checked sign/magnitude arithmetic, shifts the target
interval back by that cumulative offset, and intersects it with the source
carrier. The cast and every arithmetic prefix retain distinct obligations and
evidence, so later cancellation cannot erase earlier safety. Literal-left or
reversed arithmetic, runtime or computed siblings, right-associated shapes,
local or block roots, intervening shells, additional casts or other
proof-bearing operations, non-native or mismatched carriers, missing,
reordered, reversed, redirected, cyclic, or stale definitions, cumulative
offset overflow, and stale or missing evidence remain fenced.
A direct validator-legal partial fixed-native exact cast may likewise root a
finite nonempty left-associated same-target-carrier exact-multiply chain. Every
right operand is an independently landed nonnegative target-carrier literal;
signed negative factors remain outside this family. The cast independently
proves direct representability. For each multiply prefix, the verifier walks
only prior canonical shrinking-prefix definitions to that cast and accumulates
the literal factors with checked arithmetic. Cumulative product zero or one
makes only the current multiply prefix true. A larger product divides the
target interval back toward the direct source root—`[0, MAX / product]` for an
unsigned target, or `[ceil(MIN / product), floor(MAX / product)]` for a signed
target—and intersects that interval with the source carrier. Vacuous sides are
omitted and an empty intersection is false. The cast and every prefix retain
distinct obligations and evidence, so a later zero factor cannot erase an
earlier unsafe multiply. Literal-left or right-associated shapes, runtime,
computed, negative, or mistyped factors, mixed carriers, local or block roots,
intervening operations or casts, non-native or invalid casts, malformed or
stale definitions, cumulative-product overflow, and stale or missing evidence
remain fenced.
A direct validator-legal partial fixed-native exact cast may instead root the
unified finite nonempty left-associated same-target-carrier affine chain when
both an exact add/subtract offset and an exact multiply occur. Every right
sibling is an independently landed target-carrier literal, and multiply
factors are nonnegative. The cast retains its independent direct
representability proof. For every arithmetic prefix, the verifier follows only
prior canonical shrinking-prefix definitions to the cast and composes the
checked affine form `A * source + B`. Positive `A` maps the target interval
back through ceiling/floor division and intersects it with the source carrier;
`A == 0` makes only the current prefix true or false from target
representability of `B`. No cast or earlier arithmetic evidence is imported,
so later zero factors or cancellation cannot erase an earlier obligation.
Homogeneous offset and multiply chains remain on their narrower paths.
Literal-left, reversed, or right-associated shapes, runtime, computed,
negative, or mistyped siblings, mixed carriers, local or block roots,
intervening operations or casts, non-native or invalid casts, malformed or
stale definitions, coefficient/offset overflow, and stale or missing evidence
remain fenced.
A direct validator-legal partial fixed-native exact cast may also root a finite
nonempty left-associated same-value-carrier exact-left-shift chain. Every right
operand is an independently landed fixed-native signed or unsigned count,
count carriers may differ between links, and each count independently satisfies
`0 <= count < value width`. The cast independently proves direct
representability. For each shift prefix, the verifier walks only prior
canonical shrinking-prefix definitions to that cast and accumulates counts in
a checked `u128`. Cumulative count zero makes only the current shift prefix
true. A positive cumulative count below the value width shifts the target
interval right by that count—`[0, MAX >> count]` for an unsigned target or
`[MIN >> count, MAX >> count]` for a signed target—and intersects it with the
source carrier. A cumulative count at least the width intersects the zero-only
target interval with the source carrier. Vacuous sides are omitted and an empty
intersection is false. The cast and every prefix retain distinct obligations
and evidence. Runtime, computed, negative, out-of-range, address, or non-native
counts, right-associated or reversed shapes, mixed value carriers, local or
block roots, intervening operations or casts, non-native or invalid casts,
malformed or stale definitions, cumulative-count overflow, and stale or
missing evidence remain fenced.
A direct validator-legal partial fixed-native exact cast may also root the
corresponding finite nonempty left-associated same-value-carrier
exact-right-shift chain. Counts are independently landed legal fixed-native
literals and their carriers may differ. The cast independently proves direct
representability; every shift prefix independently reconstructs `Truth` from
its own `0 <= count < width` fact. Unlike left shift, no cumulative count,
value-definition traversal, source interval, cast evidence, or earlier shift
proof is needed. Runtime, computed, negative, out-of-range, address, or
non-native counts, right-associated or reversed shapes, mixed value carriers,
local or block roots, intervening operations or casts, non-native or invalid
casts, malformed or stale definitions, and stale or missing evidence remain
fenced.
A direct validator-legal partial fixed-native exact cast may also root a finite
nonempty left-associated same-target-carrier chain containing exact divide and
remainder in any order. Every right sibling is an independently landed
same-carrier safe divisor: nonzero for unsigned carriers, and neither zero nor
`-1` for signed carriers. The cast keeps its independent direct
representability proof, while every divide/remainder prefix independently
reconstructs `Truth` from only its own safe divisor. No cast evidence, prior
operation proof, value-definition walk, quotient/remainder algebra, or
cumulative state is imported. Runtime, computed, zero, signed `-1`, or mistyped
divisors, literal-left, reversed, or right-associated shapes, mixed, address,
or non-native carriers, local or block roots, intervening operations or casts,
invalid casts, malformed or stale definitions, and stale or missing evidence
remain fenced.
The direct-root and post-cast divide/remainder families admit one unified
runtime-divisor widening when at least one right sibling is a direct
same-carrier machine parameter. The direct-root form remains a nested chain of
at least two operations; the post-cast form remains nonempty. Every other right
sibling is either another direct same-carrier parameter or a landed safe
literal. Each runtime divisor independently requires `1 <= divisor` or, for a
signed negative divisor, `divisor <= -2`. Only the first direct-root operation
may instead use the joint `divisor <= -1` and `MIN + 1 <= dividend` form, and
only when the verifier independently reconstructs that direct dividend bound.
Computed and post-cast dividends cannot borrow that authority. The cast and
every operation retain distinct evidence; no quotient/remainder value
definition or earlier proof is imported. Literal-only chains keep their
existing paths. Zero, signed `-1`, local, block, computed, mistyped, or
wrong-carrier divisors, missing divisor guards, computed or local roots,
literal-left, reversed, or right-associated shapes, intervening shells,
operations, or casts, invalid casts, malformed definitions, and stale or
missing evidence remain fenced.
All native targets join those leaves into the same cleanup tail. Nested paths,
field-only trees, a second field identity, erased or non-Boolean fields, nested
or partial integer computation, member/comparison mixtures, calls, effects,
nested nominal ownership, other projections, and wider cleanup shapes still
fail closed.

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
expressions, which would be ambiguous for equal or overlapping arguments. A
nonempty path to a relevant Boolean field of a record parameter retains every
canonical structural-field identity and rebases across structural Unit calls.
For a field- or literal-fixed-index-projected structural argument, the caller's
canonical argument path is prepended to the callee's parameter-relative Boolean
path. Canonical predicate segments distinguish verifier-owned field identities
from exact array indices. The verifier independently traverses both declared
record and fixed-array paths, requires in-bounds indices, structural
intermediates, and a Boolean leaf, and rejects absent, erased, truncated,
mistyped, out-of-bounds, or redirected paths. Built-in Boolean equality,
inequality, negation, and conjunction may compose multiple relevant member
paths and literals; every nested path is independently traversed and rebased.
Equality, inequality, and ordered comparisons also accept same-typed relevant
fixed-integer member paths; terminal terms retain both the canonical path and
the exact integer type, and the verifier checks that annotation against the
declared leaf. Built-in fixed-integer `&`, `|`, `^`, and `~` compose the same
typed member terms without an arithmetic proof obligation; overloaded forms and
the distinct address carrier remain outside this bounded structural slice.
Whole-root and all-field-projected structural calls reconstruct those predicates
across the callee boundary by prepending the caller's canonical argument path to
every callee-relative integer-member path, including operands nested beneath
bitwise terms. The verifier repeats that substitution independently and rejects
a redirected continuation even when the redirected path reaches another valid
same-typed leaf. A built-in Boolean
disjunction retains two distinct canonically ordered proposition branches; each
branch may contain the same accepted Boolean or integer-member predicate forms.
Production and independent verification recursively rebase every branch across
whole-root and all-field-projected calls. Both codecs retain the proposition;
the semantic codec rejects nested, duplicate, or noncanonically ordered
disjunction rows. Whole-record equality does not add an opaque aggregate term:
for two same-typed `Equatable` parameters, checked production retains the
language-defined inline field expansion. A finite nonempty tree containing only
relevant Boolean and fixed-integer leaves becomes one flat canonical conjunction
of typed member equalities. Each leaf keeps its left and right parameter root;
call verification independently substitutes both roots and rejects redirecting
either operand even when the replacement path is otherwise valid and
same-typed. Text, float, sum/case, erased-field, empty-record, and written
`equals` bodies remain outside this bounded terminal slice. Arithmetic over
same-typed relevant fixed-integer members accepts Exact addition, subtraction,
and multiplication: each member or fixed-integer-literal operand retains its
exact checked carrier, nested operations remain typed `ExactIntegerAdd`,
`ExactIntegerSubtract`, or `ExactIntegerMultiply` terms, and whole-root or
all-field-projected calls rebase every member leaf recursively. The verifier
independently repeats that substitution and validates every declared leaf and
arithmetic-node type; both codecs preserve the nested term. Policy-selected
fixed-integer members also accept the total Wrapping and Saturating forms of
addition, subtraction, and multiplication. The terminal term retains the exact
selected behavior, and projected calls, codecs, verification, fixed fuel, and
interpretation preserve it without an overflow obligation. Wrapping left and
right shifts are likewise retained as total structural terms: the value's
carrier and the independently typed integer count remain distinct, and the
language-defined Euclidean count reduction survives projected calls, codecs,
verification, fixed fuel, and interpretation without a count obligation. Exact
right shifts accept a self-proving in-range literal count or a complete retained
package proving a runtime count nonnegative and below the shifted carrier width.
Exact left shifts require the same count evidence plus carrier-tight value bounds
at the greatest possible count; a zero count or a compile-known value that shifts
safely is self-proving. The producer canonically orders the complete requirement
package, and projected calls rebase one exact obligation per requirement.
Independent verification reconstructs the count and overflow checks and rejects
missing or weakened evidence. Trapping arithmetic remains fail-closed here.
Exact division and remainder accept a
same-carrier literal divisor only when it is nonzero and cannot trigger signed
`MIN / -1` overflow. Wrapping and Saturating division and remainder accept any
same-carrier nonzero literal, including signed `-1`: their selected policy
defines the `MIN / -1` result, while division by zero remains illegal.
A whole-root structural Unit closure may instead name a runtime integer-member
divisor. For Exact operations, each machine's complete bounded `requires`
package must prove one of the verifier-owned totality shapes: `1 <= divisor`,
`divisor <= -2`, or the joint signed bounds `divisor <= -1` and
`MIN + 1 <= dividend`. For Wrapping or Saturating operations the corresponding
package need only prove the divisor nonzero through `1 <= divisor`,
`divisor <= -2`, or `divisor <= -1`. Checked plans retain those packages without
source handles and terminal Psi publishes the exact requirements. Every direct
or all-field-projected structural call carries one exact obligation per callee
requirement; the producer rebases the target place through the caller's
canonical field prefix, cites the matching caller assumption, and emits a
replaceable certificate. Independent verification reconstructs that prefix,
repeats the rebasing, and checks the assumption index before codec or
interpretation. Removing evidence or weakening or redirecting a bound rejects.
Case-payload paths and imported crash capsules remain fail-closed.
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

### Normal result slices

A terminal machine result is `Scalar`, with one stable result pseudo-value;
`Structural`, with an exact type, multiplicity, qualification set, and result
place; or `Unit`, with no runtime value. `ReturnUnit` is a normal exit, not a
distinguished Boolean or integer: it creates no `ValueId`, result structural
place, or return-equality axiom. Contracts on a unit machine may refer to its
parameters but cannot name an absent result. Scalar and unit calls remain
distinct complete operation slices rather than ignored-result conventions.

Canonical encoding, independent verification, interpretation, and fixed-fuel
derivation implement this distinction. A plain unit return charges exactly its
one terminal edge and resumes atomically after sponsor exhaustion. Checked-tree
production and Omega native lowering now cover the bounded Unit, structural
custody, effect, and cleanup slices described above; unsupported Unit shapes
remain fail-closed rather than falling back to checked or source trees.

The first structural-result artifact slice is root-only whole-parameter
passthrough. `ReturnStructural` names the live source place and exact ordered
claim set transferred to the declared result place. Verification requires a
matching linear signature and whole-root entry/content binding, rejects result
places on scalar or Unit machines, and reconstructs content-identity facts only
at the validated return edge. Interpretation preserves the opaque value,
qualifications, and claim identities, charging fuel before custody or cleanup
commits; canonical encoding and fixed-fuel derivation cover the same edge.
The exact checked-source slice accepts one attached, one-state passthrough of a
whole linear parameter with matching qualifications and one whole-root claim.
It may additionally carry a finite tail of unqualified, claim-free affine
structural parameters, whose places are discarded after result materialization
in canonical reverse parameter order. It
may also establish a finite consecutive prefix of immutable, unqualified
empty-record affine locals before the return. Terminal Psi declares each local
without a source handle, charges each explicit establishment operation, and
cleans them in reverse declaration order before the optional affine parameter.
Declaration ordinals must be dense and establishment order exact. Nominal
cleanup, nonempty/partial locals, authored contracts, projections, and wider
cleanup/control shapes fail closed.
Omega realizes that exact slice through its target calling policy when the value
has one direct eight-byte integer fragment. The source and result placements,
typed local establishment, Psi edge, claim set, exact affine cleanup, and fuel
attribution survive target assignment, machine emission, object/image
construction, and canonical installation. Direct register and stack parameter
homes are retained exactly. The locals are not ABI parameters;
claim identity and trivial cleanup are zero-runtime semantic metadata rather
than extra ABI words or cleanup instructions. Wider or
indirect values, projections, structural calls, and broader control remain
fenced before partial lowering.

Normal scalar returns carry one exact ordered affine cleanup-action stream; the
stream is empty when no cleanup is required. Actions distinguish whole-root
no-code disposal, typed residual disposal, and executable nominal cleanup.
Verification reconstructs the complete live frontier and reverse declaration
order, and independently validates every nominal target and obligation.
Interpretation charges the return edge and materializes the scalar result before
committing actions, then runs nominal bodies resumably, so sponsor exhaustion
cannot replay a completed action or partially commit the exit. Fixed fuel
composes every nominal invocation. Omega preserves the same action order and
call ownership through target assignment, all five machine emitters, object and
image custody, and canonical installation; no-code actions emit no target
instruction. Current source production covers the wider trivial-discard scalar
slice plus the finite mixed no-code/nominal, branch-free-input/local branch described
above, including direct-Boolean contextual cleanup across mixed roots.

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
the native terminal Unit and scalar slices retain exact emitter evidence that
object construction replays into local peaks, caller-live bytes at typed calls,
and an acyclic closure demand. Accountable acyclic scalar conditionals use one
depth-independent carrier: physically ordered decisions, a true-before-false
DFS return/crash bitmap, and one ordered x86 division-diamond ledger. Object
construction reconstructs every exact prefix and leaf, validates each branch
and terminal crash encoding, partitions division diamonds by region, and takes
the maximum across exclusive paths. AArch64 and branch-free x86 expressions
reuse linear replay; signed x86 wrapping/saturating division retains its exact
special/ordinary diamonds. The same facts survive typed call arguments,
relocations, object/image validation, installation serialization, and installed
closure recomposition.

Boolean parameter/expression conditionals retain the same tree and call-stack
evidence. If terminal lowering source-distributes one semantic Psi convergence
call into several leaves, the object boundary permits its repeated operation
owner only when every physical pair has conflicting outcomes at a validated
decision. Calls sharing an executable path still reject. This proves the
source-distributed tree, not an actual reconvergent native join. Separately, the
finite runtime-Boolean-parameter tree slice retains one terminal-Psi join and
object replay validates its ordered native decisions, non-final-leaf
unconditional join branches, final-leaf fallthrough, and single cleanup tail on
every target. General shared native control-flow joins remain outside the
theorem. Affine cleanup admits the finite
branch-only trees described above, with one distinct cleanup-bearing return
edge per surviving leaf. The shared form also accepts Boolean equality against
a constant: Psi normalizes that leaf to identity or negation before emitting
the existing convergence carrier, so no comparison operation crosses the
terminal boundary. It additionally accepts one direct relevant Boolean field
identity on one claim-free affine nominal-cleanup root; the terminal operation,
verifier, interpreter, fuel model, codec, and every native target retain the
exact source place and canonical field ID. At least one Boolean parameter keeps
that source outside native expression scratch. The bounded direct-integer-
comparison form specified in the nominal-cleanup section above retains the same
exact Psi operations, contract premises, and certificates through this
verified, interpreted, and native shared join. The operation list and accepted
proof shapes are stated once there; this native accounting section adds no
second vocabulary.
Field-only trees,
nested or multiple member identities, wider or partial integer computation,
member/comparison mixtures, external
adapter/interrupt-arrival state, and other terminal function forms remain
outside the shared-join theorem, so the inspection surface makes no
installed-root WCSU claim.

## Implementation queue

[`TASKS.md`](../../../TASKS.md) owns remaining terminal-Psi work. Temporary
differential paths may coexist as test oracles while consumers move; they are
not alternate language versions or a permanent Omega-to-Psi path.
