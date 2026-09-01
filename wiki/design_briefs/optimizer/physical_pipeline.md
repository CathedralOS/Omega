# Optimizer Physical Pipeline

This brief owns the lowering-to-publication path. The architecture entrance is
[optimizer_architecture.md](../optimizer_architecture.md).

## Selected lowering

Abstract-to-target lowering now enters through a 68-line settlement and
installation-evidence coordinator. It descends first by function result
family, then through explicit scalar setup, special-form, conditional, and
straight-line routes or structural direct-call and return routes. Unit,
boundary-settlement, cleanup, and structural-layout mechanics remain named
sibling responsibilities rather than hidden branches in one lowering file.

The adjacent sub-100-line translation-validation entrance is independent of
those producer routes. It first binds Psi identity, requested target, entry,
function count/order, machine, and attachment, then descends into exact family
replay. Its first thirty-nine families reconstruct parameterless straight-line
Unit return, one exact PortWrite followed by Unit return, one exact Unit call
followed by Unit return, one exact trivial affine local establishment followed
by Unit return and its discard cleanup, integer and Boolean
literal returns, scalar `Crash`, direct integer
and Boolean parameter returns, Boolean negation of a parameter, and equality of
two Boolean parameters, equality of two same-type integer parameters, or strict/inclusive
ordering of two same-type integer parameters, plus integer bitwise-not and
integer-widen or proof-bearing integer exact-cast of one parameter, and integer
bitwise-AND, bitwise-OR, bitwise-XOR, proof-bearing fixed-integer exact-add,
exact-subtract, exact-multiply, exact-divide, or exact-remainder, plus
wrapping-divide, wrapping-remainder, saturating-add, wrapping-add,
saturating-divide, saturating-remainder, saturating-subtract, wrapping-subtract,
wrapping-multiply, or saturating-multiply of two same-type integer parameters,
plus wrapping or proof-bearing exact shift-left and shift-right of an
independently typed integer value and count,
without calling
`lowering`, `KnownScalar`, or the scalar-return helper. The distinct parameter
families share governed source-envelope and whole-roster ABI replay rungs,
which independently apply the target's calling policy to prove every incoming
register or stack location.
The Unit-return leaf independently requires the exact claim-free, service-free,
single-block source shape and reconstructs the target's empty native call plan,
empty parameter and operation provenance, exact return edge, and empty cleanup
roster. Whole-plan admission separately binds every Unit body's canonical
structural-type roster to the source plan so a function-local family validator
cannot silently accept reordered, substituted, missing, or injected global
declarations.
The adjacent PortWrite leaf requires the exact parameterless, claim-free,
single-block `PortWrite; ReturnUnit` shape and a singleton published-service
ceiling. Independent replay retains the operation, service, port, byte,
provenance, empty native call plan, return edge, and cleanup roster on every
native target; optimized custody consumes the typed family receipt rather than
reclassifying the target body.
The Unit-call sibling admits a separate parameterless Unit-return callee and an
exact claim-free, structurally parameterless `CallUnit; ReturnUnit` caller.
Independent replay binds the callee, arbitrary exact requirement-obligation and
crash-continuation rosters, empty structural arguments and claim transfers,
empty native call plan, target provenance, cleanup, and return edge on every
native target. Exact operation rosters keep it disjoint from return-only and
PortWrite families.
The trivial-affine-local sibling requires one empty-record declaration at
ordinal zero, no construction, and the exact `DiscardRoot` cleanup before Unit
return. Its independent replay reconstructs the establishment, local/type
identity, native Unit call plan, return edge, cleanup, and provenance on every
native target; its two-operation roster keeps it disjoint from the other Unit
families.
Parameter replay descends through explicit direct, unary, arithmetic, bitwise,
and comparison rungs. The arithmetic model join owns only the common ordered
operand/result carrier and sends obligation-retaining policies through its named
`proof_bearing` leaf. Arithmetic catalog dispatch and independent source/target
reconstruction send the complete exact, wrapping, and saturating
division/remainder set through named `quotient` leaves; the arithmetic parents
retain the add/subtract/multiply families instead of accumulating one flat
switchboard. Those joins bind operand/result identity and exact
operation/edge provenance; Boolean equality, typed integer equality, and strict
or inclusive integer ordering retain ordered and identical operands through
recursive `ReturnBooleanExpression` receipts, while integer bitwise-not,
bitwise-AND, bitwise-OR, bitwise-XOR, exact-add, exact-subtract, exact-multiply,
exact-divide, exact-remainder, wrapping-divide, wrapping-remainder, wrapping-add,
saturating-add, saturating-divide, saturating-remainder, saturating-subtract,
wrapping-subtract, wrapping-multiply, and saturating-multiply retain exact-width
operands,
integer-widen retains distinct source/target types, and exact-cast additionally
retains its proof obligation through `ReturnIntegerExpression`.
The sibling shift rung owns distinct value/count types, values, parameter
indices, and ABI locations rather than forcing them through arithmetic's
same-type carrier. Both wrapping directions admit fixed or address64 carriers
independently for value and count and reduce signed negative counts with
Euclidean modulo by the value width. Wrapping right shift separately preserves
unsigned fixed/address zero-fill and signed fixed sign-fill. Neither
direction's specific family identity can be substituted with its sibling,
either proof-bearing exact shift, bitwise, or arithmetic expressions even when
an individual runtime value agrees.
Exact shift-right admits only fixed value/count carriers and retains the
operation's canonical `ExactShiftCount` obligation. That goal proves the
unmodified count is in `0 <= count < value_width`; it does not require discarded
bits to be zero. Signed values sign-fill and unsigned values zero-fill. Replay
rejects wrapping or left-shift substitution, address carriers, operand drift,
and independent obligation drift.
Exact shift-left uses the same fixed-carrier and independent-type custody, but
retains the stronger canonical `ExactShiftLeftRepresentable` obligation. Its
goal conjoins the unmodified count range with mathematical-result bounds for the
value carrier. Replay rejects wrapping or right-shift substitution, address
carriers, operand drift, and independent obligation drift.
Exact-add independently retains its range-obligation identity, rejects address
carriers, and rejects substitution with wrapping or saturating addition.
Saturating-add independently rejects both wrapping and proof-bearing exact-add
substitutions. Saturating-subtract likewise rejects wrapping and proof-bearing
exact subtraction without treating operand order as commutative.
Saturating-multiply independently rejects wrapping, proof-bearing exact,
addition, and subtraction policy substitution. Exact-subtract and
exact-multiply retain their exact range-obligation identities; exact-divide and
exact-remainder retain defined-division obligations covering a nonzero divisor
and a representable quotient. All four reject address carriers and
arithmetic-policy substitution and never treat ordered-operand custody as
commutative. Exact remainder follows truncation-toward-zero division and keeps
the dividend's sign; signed `MIN % -1` remains undefined because the
corresponding exact quotient is not representable. Wrapping divide and
wrapping remainder instead retain only nonzero-divisor obligations. The former
maps signed `MIN / -1` to `MIN`; the latter returns zero while otherwise keeping
the truncating remainder's dividend sign. Saturating divide also retains only a
nonzero-divisor obligation, but maps signed `MIN / -1` to `MAX`; saturating
remainder retains the same goal and returns zero for signed `MIN % -1`, while
otherwise keeping the dividend's sign. The optimized
custody tests construct canonical exact-cast representability, exact-subtract
range, exact-product range, exact-division-defined quotient and remainder
goals, and wrapping plus saturating quotient/remainder nonzero-divisor goals as
machine preconditions, then discharge them with real Terminal proof
certificates. Replay never substitutes fresh proof search for obligation
identity. Its adjacent ordered catalog is the sole enable/disable inventory.
Each descriptor joins one source classifier to one typed replay adapter; the
separate selection leaf makes zero matches explicitly uncovered, retains one
match on that function's roster row, and fails closed on ambiguity.
Error and receipt publication follow the same descent: their small entrances
map immediate, parameter, terminal, and whole-roster responsibilities. Named
`error/family` and `receipt/function_translation` leaves own the closed tagged
carriers; `receipt/function_translation/arithmetic` names their arithmetic
payload join without widening that carrier. The separate `receipt/family` leaf
owns the exact receipt-to-family projection, while `model/family` owns the
stable family IDs themselves.
The optimized target carrier retains this receipt. Its family-row roster is
deliberately partial and therefore cannot overstate validation of other target
operations while those rows are still being added.

Target legalization and instruction selection produce explicit selected forms
over virtual registers. Fixed operands are constraints; they do not preassign
the entire program. Selected rules may fold exact incoming immediates or choose
equivalent target forms, but must preserve operation, edge, trap, provenance,
and fuel identities.

Ordinary, Unit, structural-scalar, and structural-result calls retain their
ordered requirement-obligation and crash-continuation rosters from Terminal
projection through abstract, target, and temporary assigned operations. The
structural Unit selected route carries the same rows through legalized V10 and
selected V12 identities and independent replay. Machine emission consumes the
semantic carrier but does not duplicate it into runtime effects or machine
bytes; selected instruction identity and source-operation custody bind the last
semantic boundary.

The composed-Unit rollout canary carries one qualification-free owned linear
whole-root parameter through a Boolean three-block graph. Both successor edges
transfer distinct checked state-entry aliases; each bodyless attached-Unit leaf
consumes its alias with an exact completion receipt. Checked-to-Terminal
lowering independently replays those events and binds the aliases to one
Terminal claim before verifier and codec publication.

A disjoint composed leaf family admits parameterless internal Unit calls. Each
leaf rejoins the exact checked target state, contract fingerprint, service
reach, and retained ordinary Unit plan. Lowering emits repeated calls to one
deduplicated qualification-free empty-body target machine; it does not copy the
target into each branch or treat a missing transitive plan as an empty body.
That target closure now has a second exact rung: the shared leaf target may
contain one parameterless internal Unit call before its return. Independent
admission rejoins the nested flow coordinate, state, contract fingerprint, and
service reach, then retains and deduplicates the transitive empty target.
Lowering emits one root, one relay, and one empty target machine; a missing or
altered transitive plan rejects the whole route. The admission walk is
recursive rather than depth-coded: a depth-two relay canary produces one root,
two relays, and one empty target while retaining the same one-call node shape
and rejecting cycles through its active closure.

The first larger acyclic composed family prefixes that conditional frontier
with a finite ordered chain of exact scalar-only jumps. Every checked edge
records Boolean source and target parameter position zero and targets the next
state in source order. Lowering independently reconstructs one distinct block
parameter per non-entry control state, allocates root blocks dynamically, and
places any internal target closure after the complete root graph. One- and
two-prefix canaries cover boundary and internal-call leaves. Admission rejects
changed scalar maps, target states, attachments, contracts, custody, or leaf
effects rather than silently routing the graph through a shorter family.

A disjoint nested-control family retains a general finite acyclic Boolean
control graph. Controls may target any retained control or effect leaf and may
forward any exact ordered Boolean parameter map required by the target;
multiple edges may converge on one leaf. Producer and consumer independently
walk the graph by checked state identity, reject cycles or unreachable states,
and rejoin every scalar argument expression and structural-cleanup target.
Lowering derives block/value identities and block-parameter arity from the
admitted graph before publishing boundary leaves or one deduplicated internal
target closure. Right-deep depth-three and balanced three-control canaries
cover multi-value handoffs, argument-bearing true and false arms, and a shared
leaf emitted once. Scalar-map reordering, target corruption, and forged
convergence reject the route before verifier or codec publication.

The first effectful-control widening admits a finite ordered sequence of
qualification-free, parameterless internal Unit calls before either transition
in a control state. Calls remain checked state operations rather than synthetic
blocks; therefore their source coordinates shift the guard and successor
statement ordinals. Independent admission rejoins every call coordinate,
target state, contract, service reach, and finite target closure, then emission
places the calls in source order before the conditional terminator. A dedicated
`operations` rung on each side owns this sequence. Missing targets, coordinate
drift, operation reordering, and custody-bearing calls reject before
publication.

The same operation rung admits an exact parameterless boundary call with no
scalar or structural arguments. Producer assembly includes control states in
boundary/provider discovery; consumer admission rejoins the boundary call and
emission records its source-call occurrence while appending it to the existing
control block before the guard. Provider-backed control prefixes cross this
route by joining executable operations to flow calls at exact source
coordinates; named-transition call facts remain graph topology. Checked scalar
facts omit the implicit `self` parameter but key every explicit transition
argument by its raw target position, while executable plans use separate dense
source and target scalar indices. Producer and consumer independently rejoin
those coordinates, and missing scalar facts or provider requirements reject.

Selected-plan construction has one 52-line roster entrance over scalar, plain
Unit, and structural Unit results. Scalar construction reconstructs common
condition context and selects exactly one row from its adjacent seven-row
catalog. Immediate, entry-parameter, direct and widened exact-add/subtract, and
active-resident exact-add-chain leaves each return their whole virtual-register
and block body, eliminating the former duplicated source-family matches.
Structural selection separately joins ABI layout, optional whole-root call,
and return. Catalog omission rejects and ambiguity names both conflicting
families; neither path falls back to transitional assignment.

Independent selected-block validation enters through a 39-line join that
checks the three-block source roster, replays entry control, and then routes
the exact return family. It descends into immediate, parameter, exact-binary,
active-resident exact-add-chain, and shared instruction-projection leaves.
Those leaves reconstruct custody without calling construction helpers and
retain the existing mismatch precedence, instruction IDs, register schedule,
successor order, provenance, effects, and fuel rejection order.

The disjoint unoptimized ranked-`u32` lane now reaches ordinary object custody
without borrowing its machine-code producer as a validator. Each ISA owns an
opaque decoder result for its exact countdown body; the image boundary joins
that decoded layout to independent rank, fixed-fuel, ABI, affine-frontier,
cleanup, provenance, and nine-row fuel replay. The complete ranked record is
retained on the object function.

Semantic-code attribution is emitted and replayed with the ordinary function
body. It maps semantic operations and edges to exact byte intervals without
inserting instructions or rebasing control flow.
At executable-image publication, ranked replay checks the ordinary final text,
semantic proof, rank, ABI, frontier, and semantic-code attribution without a
parallel runtime image.

The mandatory lowering crate has two explicit entrances. `legalization/mod.rs`
joins canonical source projection to independent whole-plan replay;
`selection/mod.rs` joins selected-plan construction to independent validation.
Construction then has its own meaningful roster entrance and scalar catalog
rung; it is not a forwarding wall. Structural, scalar-function,
leaf-expression, constraint, identity, and fixture mechanics descend below
those joins. The crate-level `lib.rs` is only the responsibility map between
the two stages, not a hidden third coordinator.

Immediately below the legalization entrance, `catalog.rs` is the sole ordered
inventory for all thirteen current forms: eight scalar, one plain Unit, and four
structural Unit. Each row names its typed recipe, producer matcher kind, exact
source-shape constraints, non-authoritative structural cost, and independent
validator kind. `source/matchers/` walks that catalog to recognize a form;
`replay/validators/` reconstructs membership without calling producer code.
Removing a row disables the form, and missing or ambiguous recipe lookup fails
closed. The Unit recipe families are retained in the V10 legalized-plan
identity. Structural selected-form validation separately reconstructs ABI
layout and call constraints without importing selection construction helpers.

Scalar source-leaf construction enters through a sub-100-line `derive_leaf`
coordinator. It admits the common node and return envelope, visibly routes
immediate, entry-parameter, direct exact-binary, widened exact-binary, and
active-resident exact-add forms, and then seals shared return and fuel custody.
The exact-add rung orders the existing resident chain before direct binary
fallback and now admits one distinct bridge chain,
`r + (b + (r + (a + b)))`, whose independent replay keeps `b` live across the
first resident use. This recipe is a legalization-only prerequisite: no
selected-instruction or allocator-pressure authority follows until a separate
public selection family exists. Focused leaves below it retain the existing catalog-family order,
diagnostic precedence, operation roster, proof identity, and provenance order;
they do not merge producer mechanics with `replay/validators/`.

## Register allocation

Allocation computes selected-CFG liveness, live-range fragments,
interference, allowed views, ABI constraints, clobbers, and spill legality.
Home assignment, copy insertion, spilling, coalescing, and bounded
rematerialization are separately validated decisions.

Fixed-view-copy validation descends from one small independent join through
root and copy-constraint custody, work and budget replay, leaf-local or
shared-entry policy reconstruction, and exact application/comparison. Its
validated receipt therefore represents reconstructed copy insertion, not
producer self-attestation.

Its public artifact is V6. V4 remains decode-only and byte-pinned without a
structural roster; V5 retains the structural roster but decodes call proof and
crash rows as empty. V6 encodes those rows canonically through the shared
Terminal crash-route codec and also retains parameter-rooted projected
qualification rows. Those rows are bound by the selected and fixed-view
semantic identities and authenticated by the outer envelope; legacy decoders
reconstruct fields absent from their payload versions as empty.

The current transition-free, spill-free home stage is a deterministic
constraint-graph allocator. Distinct use/definition ties form quotient
vertices whose domains are the intersection of every member's legal views.
Interference contributes symmetric storage/write-footprint conflicts and
early-clobber rows contribute directional definition-write/use-storage
conflicts. Placement repeatedly chooses the vertex with the fewest currently
viable views, then greatest remaining constrained degree, earliest live point,
and lowest VReg leader; it chooses the lowest compatible view. Producer and
validator derive the domains, graph, and ordering separately. Exhaustion is a
typed pressure result for the separately governed spill/recovery work, not
permission to invent a home transition.

The first non-rematerializing recovery boundary is a target-neutral logical
spill-operation plan. Its tiny entrance joins validated selected instructions,
live ranges, allocation legality, and deterministic spill choice, then sends
the proposed plan through a separate replay rung. V1 admits only an
active-resident non-address unsigned-U64 instruction result in one block. It
records one logical storage identity, a store immediately before the incoming
definition, a reload immediately before the first strictly later flexible use,
and the complete ordered later-use rewrite suffix. The plan is versioned and
identity-bound, but creates no selected instruction or virtual register and
claims no physical slot, byte offset, alignment, frame, unwind, trap, address
stability, encoding, or publication authority. Incoming victims, entry
parameters, legalization temporaries, fixed or use-def suffixes, and
cross-block ranges fail with typed errors rather than silently receiving a
weaker recipe.

The next target-neutral boundary is validated stack-slot coloring. Its
26-line entrance consumes only the validated logical-spill carrier and joins a
canonical producer to an independent replay validator. V1 derives a closed
block-local lifetime from pressure through the first reload rewrite, sorts by
block/start/end/storage identity, and assigns the lowest available 8-byte slot;
touching endpoints conflict, while disjoint or different-block lifetimes may
reuse a slot. Offsets are relative to an abstract spill-area origin. The
versioned, identity-bound artifact grants no stack-pointer or frame-pointer
offset, stack direction, red-zone, shadow-space, frame, unwind, probing,
instruction, encoding, trap, or publication authority. The current logical
schema carries at most one spill action per function, so cross-target compiler
fixtures reach offset zero while internal interval tests pin the full first-fit
contract.

Abstract spill insertion is the next small join. It consumes the validated
logical operation and slot-coloring receipts and emits a deterministic
per-function schedule containing the abstract store, reload, and complete
later-use rewrite suffix. Its independent replay re-derives the ordering,
register-environment and allocator-availability roots, logical reload class,
and spill-area-relative geometry. It does not create an instruction, choose an
opcode or stack/frame address, or claim memory, trap, unwind, probing,
encoding, emission, or publication authority.

Reload-value home assignment then gives that logical reload a bounded physical
view without pretending the reload is already a real virtual register. V1
reconstructs the original linear-scan prefix, applies exactly the validated
single block-local spill, intersects the victim's legal views across the
reload lifetime, and selects the lowest view compatible with every coexisting
home. The producer uses a sorted linear schedule; independent replay uses a
point-indexed event timeline with explicit original and reload occupants and
separate work reconstruction. Reload pressure and later secondary pressure are
typed failures. Recursive spill recovery, synthetic-register realization,
memory effects, ISA lowering, and frame integration remain later boundaries.

Synthetic reload-value binding closes only the next namespace seam. Its V1
artifact assigns each validated logical reload/home pair an epoch-zero ordinal
in canonical function and logical-value order, retaining the insertion and
home identities plus lifetime, class, and chosen view. Production uses direct
function traversal; replay independently indexes logical reloads and sorts the
closed binding set. The synthetic identity is deliberately not a selected
`VirtualRegisterId`. Bounded recursive epochs/worklists, spill pseudos, real
instructions, memory effects, and all frame/publication work remain absent.

The bridge-chain recipe now has its own scalar-selection family and independent
selected replay. Its selected shape contains nine virtual registers and twelve
instructions. Under the public two-view availability policy, both x86-64 and
AArch64 traverse liveness, ranges, legality, spill choice, logical spill, slot
coloring, abstract insertion, and reload-home replay before reaching the exact
typed `ReloadPressure { function: 0, result: 0 }` branch. The fixture binds the
pressure point, incoming and victim values, store-before, reload-before, and
complete rewrite suffix without directly constructing any compiler-private
`Validated*` receipt. This establishes the deferred branch as reachable; it
does not itself publish an epoch-one seed. The adjacent bounded spill-recovery
worklist begins only at that independently reproduced failure and emits exactly
one `{epoch: 1, ordinal: 0}` item. V1 retains the source reload, machine, block,
half-open lifetime, reload class, complete canonical candidate domain, and
separately identity-bound trigger/worklist budgets. Production and validation
reconstruct all usage axes independently. The item does not choose a second
victim or assigned view, create a selected virtual register or rewrite, or
grant memory, frame, trap, unwind, encoding, emission, or publication
authority. Its next consumer must make the second spill choice and generalize
logical insertion/reanalysis under a new bounded contract.

Register units model aliasing between views. Flags/predicates, vector lanes,
special registers, ABI reservations, call clobbers, and stack/frame constraints
are explicit target facts. Modulo scratch-register assignment is not an
allocator.

The temporary empty-selection compatibility assignment is navigable without
pretending to be that allocator. Its plan coordinator validates the entry
roster, its function coordinator retains provenance, and one exhaustive
carrier router descends into cleanup, boundary, Unit, structural, scalar,
placement, control, and expression families. This preserves the explicit
replacement boundary while the selected physical conveyor gains full
operation coverage.

## Post-allocation machine stage

This stage consumes physical symbolic instructions, selected liveness, homes,
and the physical register model. Its catalog currently contains exact families
such as:

- AArch64 compare-zero plus branch-nonzero to `CBNZ`;
- AArch64 shortest MOVN-seeded i64 materialization; and
- x86-64 zero i64 materialization via `XOR r64, r64` when every canonical
  RFLAGS unit is dead-out; and
- x86-64 `0..=u32::MAX` i64 materialization via the canonical five- or
  six-byte `MOV r32, imm32` form. Its 32-bit write zero-extends the retained
  semantic 64-bit destination and preserves RFLAGS; and
- x86-64 i64 materialization whose exact bits round-trip through signed i32
  via the canonical seven-byte `REX.W + C7 /0 r64, imm32` form. Its 64-bit
  write sign-extends the immediate and preserves RFLAGS.

All produce variants of one validated post-allocation stage result. The result
contains the original source identity, exact rule identity, validated symbolic
plan, accounting, and custody receipt. Encoding dispatches on that typed plan;
the complete compiler route does not grow a new parallel carrier family for
each rule.

CBNZ is the first rule whose producer consumes a declarative symbolic
terminal-pair descriptor. That descriptor states the exact compare and branch
alternative keys; operand and allocatable `x0..x30` view shape; named `nzcv`
and `pc` unit sets; memory, stack, trap, and control effects; compare-to-branch
liveness continuity; and flags-dead-after eligibility. The shared matcher is
partitioned into instruction, register, and liveness leaves under one small
coordination entrance. The rule's independent validator deliberately retains
its separate replay logic. Other materialization rules remain rule-local
selection until a genuinely shared pattern vocabulary is demonstrated.

The shared matcher now has a second, still-bounded descriptor consumer. The
core-only AArch64 same-view-copy owner matches an exact terminal
`CopyI64; ReturnI64` pair, including cross-instruction virtual-register and
physical-view/storage relations, then joins its proposal to an independently
replayed symbolic disposition and canonical codec. It is deliberately absent
from the machine catalog and complete compiler route: the artifact grants no
encoding, layout, realization, build-selection, or publication authority.
Consequently this proves relation vocabulary and one exact copy-elision case,
not arbitrary peephole topology or general copy removal.

The adjacent `costs/` rung is deliberately non-authoritative. Its V1 model
binds the complete native target and model version into a stable identity, and
projects existing `MachineSizeKnowledge` without converting encoder-resolved
bounds into exact facts. Latency remains explicitly unavailable. The result is
safe input for future ranking and reports only: exact-rule eligibility,
independent replay, and semantic validation cannot consult it, and the source
organization audit enforces that dependency boundary.

Direct homes and homes after selected lowering enter one
`StagedPostAllocationMachineFunctionRelativeRealization`. CBNZ, MOVN,
XOR-zero, MOV-r32-imm32, and MOV-r64-imm32 therefore share the same encoding,
layout, exit, realization, and fragment source route. The former named CBNZ/MOVN
complete-route carriers have been removed; rule-specific values remain typed
leaves borrowed from the shared result.

Selected-lowering realization likewise enters fragment admission through one
`SelectedLowering` carrier whether or not a function-relative layout rule also
ran. Add/subtract folds do not acquire fragment route variants, and rel8 remains
an optional typed leaf of the selected-lowering realization rather than an
admission prerequisite. The fragment manifest records the generic phase source
kind while its selection and realization identities retain the exact rules.

The incoming-u12 add/subtract producer emits an immutable fold plan. Its
validator separately reconstructs source eligibility, register constraints,
the exact action roster, rewritten instructions, provenance and fuel custody,
dense identifiers, work usage, and transformed-plan identity. Validation does
not call the producer's transformation helpers; an architecture dependency
guard enforces that separation.

The adjacent machine catalog is also the architecture-admission point. CBNZ
and MOVN require AArch64; XOR-zero, MOV-r32-imm32, and MOV-r64-imm32 require
x86-64.
Each row joins the exact optimization name, required architecture, and closed
execution kind. The selected row survives physical composition intact; both
source lineages dispatch on its typed kind instead of reconstructing a second
name-to-implementation switch.
Function-relative rel8 relaxation declares x86-64 in its adjacent layout
catalog. Unsupported target selection is rejected with the exact optimization,
required architecture, and actual architecture before rule dispatch; custody
errors preserve this reason instead of converting it to a generic
phase-composition or root mismatch.
Linux, Windows, and UEFI x64 share x86-64 applicability, while Linux and macOS
Arm64 share AArch64 applicability. UEFI applicability does not grant its still
unimplemented publication authority.

## Encoding and layout

ISA crates own canonical form encoding/decoding and reconstructed effects. A
machine rule never hand-assembles bytes. Layout-independent encoding retains
row identity, decoded footprint, effects, provenance, and optimization
disposition.

The encoding entrance joins construction to a separate `validation/` rung.
That rung checks roots and normalized optimization custody, then descends
independently through ordinary rows, structural rows, and aggregate
counts/identity. Row validation consumes candidate bytes only through the
target-owned baseline, MOVN, XOR-zero, MOV-r32-imm32, MOV-r64-imm32, and
structural-call decoders; an
architecture guard forbids imports of producer row/structural encoders. CBNZ
dispositions are reconstructed from the typed optimization plan while its
unresolved branch remains explicit deferred control.

Function-relative layout resolves labels, branch extents, and exact byte
offsets. Its independent admission rung re-derives layout policy, canonical
block order, function/block spans, row offsets, structural call/return spans,
and aggregate identity from admitted pre-layout dispositions. Candidate x86-64
and AArch64 conditional branches are decoded with target-owned validators;
x86 displacement is relative to the branch end, while AArch64 displacement is
relative to the instruction address. Candidate evidence, fused CBNZ register
reads/effects, and structural unresolved-fixup custody must match that replay.
Layout rules such as x86 rel32-to-rel8 relaxation consume a complete baseline
layout and return a validated replacement. Baseline and selected byte counts
remain replayable.

## Exit and publication custody

The pipeline retains one chain:

```text
validated stage result
  -> selected-form encoding receipt
  -> resolved-layout receipt
  -> whole-function exit contract
  -> function-relative realization manifest
  -> fragment/text/object manifests
  -> optimized artifact manifest
  -> ordinary callable-entry manifest
```

Each boundary recomputes child identities and rejects detached, reordered,
truncated, trailing, or cross-source data. Generic artifact layers bind child
identities and do not need a new schema merely because a new exact rule exists.
The genericization changed the data carried at three serialized boundaries:
function-relative realization is v9, while fragment emission and fragment text
placement are v8. Their records retain the exact selected-lowering selection
or post-allocation optimization, not a broad optimization level.

Function-relative V9 persistence enters through a 78-line framing and final-
admission join. Canonical content encoding, ordered decoding, post-allocation
optimization tags and custody, target layout, rendering, errors, and cursor
mechanics descend into named leaves. The split preserves every byte and the
existing trailing-data, conflicting-transformation, then identity-mismatch
rejection order.

The Terminal-Psi-to-native stage now exposes its full physical composition as
small owning entrances. Source-entry settlement replays declaration and
calling-plan custody. Native realization then chooses the ordinary or exact
selected input, admits provider executions/installations, emits machine code,
and replays object/image assembly. The optimized ProgramStorage encoding and
wrapper-object boundaries remain separate joins: encoding projects and replays
the target template; object construction joins settlement, semantic contract,
composite object, manifest, and custody. Provider projection, machine routes,
artifact assembly, semantic replay, object validation, models, codecs, and
diagnostics descend into named leaves.

D32 requires this physical composition to issue one child receipt for every
settled boundary occurrence surviving the validated optimization projection.
The role-tagged `PhysicalChildParent` is either a reference to reconstructible
D29 operator-application coverage or a complete retained-and-replayed D41
boundary-trait settlement. Equal D29 applications may share one semantic
parent, but optimized-operation identities and physical children remain
distinct. Each child binds the domain-separated identities of its parent and
exact optimized operation before retaining selection, assignment, relocation,
and emitted-span custody. Artifact replay derives the survivor set and rejects
missing, duplicate, stale, substituted, padded, or role-swapped children; the
physical pipeline may omit only an occurrence whose verified optimization
proof establishes elimination.

For UEFI, the physical adapter contract is settled but not yet implemented in
this chain. A generated ABI shell invokes one checked bootstrap adapter;
physical-arrival and firmware-service postconditions supply its opaque
premises. The adapter establishes Loaded Image correspondence and independent
initial storage, proves resource composition, crosses
`ProgramStorageEntry::enter`, calls the semantic continuation, reclaims
returning-profile resources, and maps normal Unit return to success. Crash,
trap, and abort remain non-returning. Optimization selection still grants no
firmware image or publication authority until that adapter, native-image
validation, and selected-build publication join the same custody chain.

## Composition policy

During bring-up, phase compositions are deliberately narrow. A route accepts
only exact implemented sets and rejects the rest with a closed error. The
long-term coordinator executes each stage catalog over one typed stage carrier;
it must not encode an optimization name in top-level route variants.

The current policy is centralized at
`coordination/physical_pipeline/routes/composition/mod.rs`. Owning catalogs
first validate each phase selection and target predicate; this entrance alone
admits baseline, selected lowering, one allocation-recovery rule alone, one
post-allocation rule with optional selected lowering, function-relative layout
with optional selected lowering, and four exact cross-phase pairs:
active-resident immediate-U64 multi-use rematerialization followed by AArch64
MOVN, x86 XOR-zero, x86 MOV-r32-imm32, or x86 sign-extending MOV-r64-imm32
selection.
Multiple recovery or machine rules, every other recovery-machine pair, and
machine plus layout reject before route execution.
The canonical post-allocation catalog entry survives composition and its closed
execution kind selects the named leaf; lower leaves independently validate
transformation and custody.

## Required tests

- disabled selection preserves baseline bytes and identities;
- wrong target, form, allocation, or live-out facts reject;
- producer and independent validator disagree on corrupted plans;
- canonical encoders reject alternate or trailing byte forms;
- exact byte deltas and offsets replay after layout;
- every custody boundary rejects one-field identity corruption; and
- direct, selected-lowering-composed, and final artifact paths retain the same
  full selection identity.

The catalog matrix covers all 16 current exact names across all five native
target constructors: 66 admitted cells and 14 typed architecture rejections.
Target-independent Psi, selected-lowering, and allocation-recovery rules are
explicit declarations, not untested fallthrough behavior.

The adjacent composition matrix covers all 120 unordered exact-name pairs on
both x86-64 and AArch64. Its 240 cells contain 132 admitted routes, 56 typed
composition rejections, and 52 target rejections. Every cell also checks the
exact Psi pass projection and proves that overlaying the complete Psi suite
does not change the physical disposition; focused triple cases pin the two
selected-lowering rules with machine and layout routes.

Current XOR-zero coverage proves both direct and selected-lowering routes
and its active-resident zero-rematerialization composition through fragment,
object, and callable publication. The composed route consumes recovery's
recomputed liveness, so the rule's exact dead-RFLAGS predicate is checked after
the selected graph changes. Target-register-environment
coverage selects and corrupts the exact scalar-call ABI row for System V AMD64,
Microsoft x64, AAPCS64, and Darwin AAPCS64 across Linux x64, Windows x64, UEFI
x64, Linux Arm64, and macOS Arm64. It checks argument/result views, implicit
control and stack facts, every individual call clobber, preserved-unit
injection, platform ABI substitution, preservation-convention drift, and the
Microsoft structural-Unit call row. This does not claim general call-crossing
allocation coverage: general scalar calls are not yet represented in the
selected CFG.

Applied selected-lowering publication coverage crosses both exact incoming-u12
rules with every hosted target: Linux x64, Windows x64, Linux Arm64, and macOS
Arm64. Each row retains two literal-fold actions through encoding, fragment,
text, ELF/COFF/Mach-O object construction, and ordinary-callable admission;
repeated runs compare every serialized manifest, record, text span, and object
container. UEFI and unwind are not implied by this matrix. The former lacks
publication authority, while physical frames and unwind carriers remain P5
prerequisites.

Ranked countdown coverage proves ordinary executable-image and installation
custody on Linux x64 and Linux Arm64, including exact final bytes and semantic-
code attribution.

The ordinary empty-selection compiler route is deliberately outside this
optimizer custody chain. Its four-target byte and artifact-metadata baseline is
locked by the no-selection golden compiler test; only an explicit nonempty
selection constructs the optimizer-side prephysical carrier.

Allocation recovery has one final function-relative carrier. Its closed source
taxonomy has `FixedViewCopies` and `ActiveResidentRematerialization` leaves;
each leaf retains its exact upstream receipt and post-allocation transformation
identity, while the carrier owns the common machine plan, selected-form
encoding, resolved layout, whole-function exit contract, and realization
manifest. Recovery alone returns the physical pipeline's `AllocationRecovery`
variant, and fragment admission consumes one `AllocationRecoveryV1` source
kind. The admitted active-resident-plus-materialization pairs instead move the
same source taxonomy into the generic post-allocation realization's
`AfterAllocationRecovery` leaf. That join independently replays recovery,
machine plan, exact materialization, encoding, layout, and exit custody and
publishes the generic post-allocation source kind. Neither recovery rule owns a
parallel publication vertical. Fragment and fragment-text manifests use schema
v9 because the source-kind tag denotes these generic carriers.
