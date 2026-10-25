# Copied call operands keep their evaluation epoch

A copied first argument equal to 7 stays 7 when the second argument overwrites its former home with 9.

Parent: **STATE-LOCAL-VALUE-FRONTIER**; prerequisite evidence for **CHECKED-PACKAGE-REUSE**. The first argument completes as 7, then `overwrite` stores 9 through the original home. The caller must establish `receive`'s explicit `requires first == 7` from that completed copy. `receive` transports its declared premise through `seven`'s entry contract, establishes its result as 7, and permits the caller's exact subtraction `observed - 7`. A runtime guard does not establish that fact.

The nine places are [`local`](local/main.omg), [`field`](field/main.omg), [`parameter`](parameter/main.omg), [`call_result`](call_result/main.omg), [`match_join`](match_join/main.omg), [`const_name`](const_name/main.omg), [`other_state`](other_state/main.omg), [`other_machine`](other_machine/main.omg), [`target_body`](target_body/main.omg). Every place has an `invalid/main.omg` twin changing only the initial first operand from 7 to 9. It must refuse the same 7 obligation. Matching pins live under `tests/omega/{pass,fail}/facts/copied_call_argument_snapshot_through_*`.

The original baseline `55898d082fc8` reread the first operand's home after later operands ran. The producer already retained exact `FlowOperandFact` completion constraints, and call admission must consume that capture; live views instead consult their exact invocation-time referents. The by-value entry-contract correction in `9ac3908938b6` also requires each `receive` body to declare its premise: a caller's literal cannot justify an unrestricted callee body. Positive and false-input twins share that same contract; only the initial first operand differs. A conservative unknown-input fallback must never re-establish a fact from a predecessor subset.

## Copied descriptor transport

The selected value of a copied first argument remains 7 when the second argument
replaces its former descriptor home with a view of 9.

The same nine places have `<place>/descriptor/main.omg` witnesses and
`descriptor/invalid/main.omg` twins. The positives carry
`requires selected.target.value == 7` through the selected place into `seven`
and then the caller's exact subtraction. Twins change only the first backing
value from 7 to 9. Their pins are
`tests/omega/{pass,fail}/facts/copied_call_argument_snapshot_descriptor_through_*`.

A copied descriptor releases its former slot after evaluation while retaining
its selected backing until the enclosing invocation. Staged exclusions use the
actual captured descriptors and their immutable storage cuts, including a
backing installed by an earlier nested call. Mutating or consuming that backing
must reject even after another call replaces the original slot. Borrowed
arguments keep their source-home exclusion. Capture-time case evidence excludes
inactive payloads; current tags cannot erase an older active copy's loan.

Nested-reference contracts retain explicit referent crossings. An indirect read
never becomes an inline canonical field or a new proof assumption. Checked
source contracts remain the authority for source admission; this change does
not restore a second proof replay in ordinary compilation.

Related ownership controls under `tests/omega/{pass,fail}/borrows` pin retained
intermediate backing, consumed backing, borrowed source homes, independent input
permissions, and active/empty case snapshots. Explicit image provenance differs
from opaque static lifetime debt. An opaque-input call-result control also runs
through native and Terminal execution; it retains the exact supplier without
inventing an address. The image-backed descriptor control now also executes in both engines;
its former sequence-cleanup refusal is retained in the baseline evidence.

Host acceptance is `omega run --both <place>/descriptor/main.omg` with native
and Terminal exit 0, plus each twin rejecting `receive`'s 7 obligation.
Windows native execution is owner-deferred to a Windows host. Linux native
execution is unverified; declaring four hosted entries does not execute them.

## Copied byte-view descriptor transport

The selected value of a copied first argument remains 7 when the second argument
replaces its former descriptor home with a view of 9.

The nine `<place>/byte_view/main.omg` witnesses carry the pointer-and-length
descriptor in a shared byte-view field. The unchanged consumer `seven` and exact
subtraction require its captured extent of 7; `byte_view/invalid/main.omg` changes
only the first extent to 9. This is a distinct descriptor representation, not
another count or integer width. Matching positive and reject pins live under
`tests/omega/{pass,fail}/facts/copied_byte_view_argument_snapshot_through_*`.

Fresh optimized baseline `a92f16431042e8a54bc3f6d90a87f8602fa3aa18` accepted the
positive source obligations, but eight places refused normal-exit cleanup and
`const_name` refused its inline literal range. All nine twins rejected `receive`'s
7 obligation. The smaller image-backed descriptor control also refused cleanup.
A non-consuming observation must preserve custody: a move candidate roster does
not establish a transfer. The repair uses the selected source multiplicity before
retiring a claim or transferring storage, while retaining completion cuts.

Range formation retains its complete source argument, logical root, selected
path and requested access. A computed collection is evaluated once before its
endpoints; its new descriptor formation keeps the actual child backing and
access obligations. Record-field byte types and literal descriptors enter the
canonical catalog through their owning producers. An implicit nominal region
retains its exact supplying declaration rather than repeatedly creating an
unresolved result type. Native call-result custody counts owned descriptor slots;
reference-result provenance is checked separately. State transfers retain local
backing from complete shared-leaf lifetime profiles even after a forming call
access ends; shared type spelling alone cannot establish external backing.

Related `pass/borrows/runtime_*_window_*` controls exercise computed shared
record projections with actual byte reads, aliased record prefixes, collection
and endpoint evaluation order, and mutable-parent restoration after named and
returned windows. `runtime_returned_literal_window_exit` checks a helper-created
image range; `runtime_mixed_byte_view_reference_result_exit` checks an aggregate
with both an owned byte descriptor and a returned record reference.
`runtime_transitive_mixed_result_backing_exit` retains the intermediate record
and its own borrowed backing across the same state transfers. The mutable
shared-field twin rejects an attempted access escalation, and the mixed-result
backing-write control rejects mutation while its returned reference remains live. Nominal mismatch
controls must reject through ordinary call checking, without inventing a region
binding merely to settle inference.

Host acceptance is all nine byte-view positives with native and Terminal exit 0,
and all nine twins rejecting the same `receive` obligation. Windows native
execution is owner-deferred; Linux execution is unverified. Baseline and current
producer-bound observations remain distinct in the qualification record.

## Inline byte-view sources and untouched siblings

The selected value of a copied first argument remains 7 when the second argument
replaces its former descriptor home with a view of 9.

The nine `<place>/inline_byte_view/main.omg` witnesses pass the replacement
literal directly to `overwrite` and retain a second view field of extent 4.
Every caller checks that the first copied extent stays 7, the replaced field
becomes 9, and its sibling stays 4. The existing byte-view row retains the
named-source control. Each inline witness has an `invalid/main.omg` twin that
changes only the first extent to 9; the consumer still requires 7. Matching pins
are `tests/omega/{pass,fail}/facts/inline_byte_view_argument_snapshot_through_*`.

Optimized baseline `6ede39fcd256b210ac9a3be102ae61147ba9666b` refused a direct
literal with “call stores a borrowed value without a complete destination/backing
relation.” The same source passed when the literal was bound to a local; removing
the sibling did not remove the refusal. Stored-call substitution now reads the
source descriptor's producer relation rather than requiring a declaration home.
An image-backed descriptor has a complete origin without such a home. Carrier
identity stays separate from backing identity, including exclusive transfers;
unknown relations still reject. The sibling and detached copy retain their own
loans. This also exercises the larger aggregate call/return representation.

Acceptance is all nine inline positives running with native and Terminal exit 0,
all nine false-input twins rejecting `receive`'s unchanged obligation, and the
named-source row retaining its behavior. These observations are for macOS ARM64;
Windows native execution remains owner-deferred and Linux execution unverified.

## Computed byte-view sources

The selected value of a copied first argument remains 7 when the second argument
replaces its former descriptor home with a view of 9.

The nine `<place>/computed_byte_view/main.omg` witnesses keep that obligation and
its false-input twin. Each program exercises a source-free helper, a helper that changes another
descriptor and toggles a counter once, an owned carrier, a direct projected
view, a constructed carrier, and a returned view selected before a later write. The field place binds the computed result
before the call; the parameter place borrows the counter through a helper;
the call-result place returns a local array view and reads its distinct first
byte after installation. The sibling view stays at extent 4. Matching pins are
`tests/omega/{pass,fail}/facts/computed_byte_view_argument_snapshot_through_*`.

The initial computed controls refused on
`a59b4a6ce16cdba1316ae78d83a945a2eee46b23`. The expanded argument-replacement
controls also refused on `a8d49c50220db1d583ed399384e682246d82e03b`.
Stored-call origins now compose checked image/input result relations, and complete
content-write footprints preserve unrelated descriptor bindings. Reference-free
storage has an authenticated empty binding inventory; an unknown inventory is
still unknown. Owned record projections transport descriptor bindings separately
from pointee contents, retaining the completed copy's epoch. Reads through a
reference still observe live contents. The additional rejection controls
`computed_view_content_write_retires_old_scalar_fact` and
`computed_view_store_retains_local_array_backing` check that this preservation
grants neither stale scalar facts nor permission to mutate borrowed backing.

Host acceptance is nine native/Terminal exit-0 agreements and nine twins refusing
`receive`'s 7 obligation, together with both ownership controls. Windows execution
remains owner-deferred; Linux execution is unverified.

Stored calls now retain each operand completion inventory and the intermediate
frontiers of its child evaluations. Constructed source summaries conservatively
retain every possible backing across those cuts; completed Flow operands keep
the exact value predicates. Owned carrier copies and direct view values survive
a later argument replacing their old slot. A constructed carrier also retains an
earlier field's local-array backing after another field replaces its source slot.
The constructor control selects its array-backed child between two image-backed
slot versions, so entry/exit inventories alone cannot recover it. Constructor
and returned-view backing controls reject writes to their independently retained
arrays after the old source slots have been replaced.
State-edge and return-expression effects use the same chronological owner.
Borrow comparisons consume captured descriptor backing separately from its
former home; access authority remains with the original loan judgments. Terminal
argument preparation establishes descriptor values at their authored positions,
including views with existing or projected homes.

The computed row's replacement helper installs a descriptor through a borrowed
array element, copies it to a local, and returns that completed value. The local
place replaces the slot again before returning the saved descriptor. The field
place uses an intermediate constructed carrier; the other-machine place nests
the array in a borrowed record. Every place checks the untouched second element
and sibling field. The call-result place supplies local-array backing and checks
its distinct first byte after installation.

The supplier relation retains structured source coordinates independently of
conservative write footprints. An indexed home can coarsen the latter even when
its loaded descriptor resolves to an exact evaluated supplier. Descriptor capture
uses that supplier's checked path, type, access and full retention inventory;
footprint coarseness supplies neither an address nor a reason to discard this
association. Loan-owner path translation also looks through reference and
constraint shells at every selector. Ordinary loan judgments still authorize
access. The backing-write control replaces the array slot, then attempts to
mutate the saved descriptor's backing; it must reject independently of that slot.

Baseline `3ddc7d1455f` refuses all nine expanded positives for missing completed
descriptor backing at simultaneous call admission. Acceptance is native/Terminal
exit 0 for each positive and the unchanged `receive` refusal for every source-9
twin. Measurements are for macOS ARM64; other-host execution and general opaque
and dependent result composition remain open under the parent acceptance.

The computed row now passes its first copied carrier through a ranked identity
return. Its field place copies that carrier to an immutable local before the
recursive edge; its const-name place forms a literal window. The original
snapshot, bound, operation and source-9 twins are unchanged. Baseline `b50066b7530`
refused the recursive descriptor-length guarantees and completed backing join.
Finite result equations now distinguish descriptor leaves from paths growing
inside their pointees. Exact immutable input-slot correspondence establishes
returned length metadata without citing the recursive guarantee being checked.
Completed owned bindings retain inline descriptor cells across disjoint writes,
while typed projections hide old pointee contents. Local copies follow retained
completed values; contextual window operands supply their captured carrier for
selected builtin indexing. These relations grant no new access authority.
The changed-input, swapped-field and shortened-window controls under
`tests/omega/fail/contracts/cyclic_descriptor_result_*` must refuse the helper's
own length guarantee. Ranking and recursive premise checks remain independent.


## Public local dispatch results

The computed-byte-view row carries the same first-argument snapshot through
`Preserving::retain`, selected at runtime from two implementations. Its public
contract preserves both descriptor lengths. One implementation returns the
input; the other copies it through a local. Each place executes both selections
before a later argument overwrites the former descriptor home. The source-9 twin
still changes only the first extent and refuses `receive`'s unchanged 7 obligation.

At baseline `032e85a7991`, all nine positives wrongly refuse a staged backing
overlap. Result capture retained lifetime roots but lost the completed input
regions promised by the local dynamic slot. The repair retains those regions for
conservative exclusion and write invalidation. Slot admission independently checks
every implementation's returned origins against that public region contract;
this does not turn arbitrary provider lifetime relations into spatial evidence.
Region rows grant neither descriptor equality, pointee contents nor loan ancestry,
and remain separate from exact supplier rows used by permission construction.
Unknown candidates still withhold coverage. Immutable image returns remain legal
and retain the public input debt; their image backing grants no exclusive access.

Public result declarations and receiving calls also use the same materialized
carrier multiplicity. A source-copyable record can still own affine descriptor
retirement. The common structural-operand query includes direct, boundary and
dynamic calls, so transferred copies do not receive a second caller cleanup.

`controls/dynamic_window/main.omg` returns a shorter window from either of two
captured input regions, checks its actual first byte and consumes its declared
extent 7 directly. `controls/dynamic_image/main.omg` retains the legal immutable
image alternative. Their pins are
`pass/borrows/runtime_dyn_{result_region,image_result}_snapshot_exit`.
`fail/borrows/dyn_result_retains_actual_backing` and
`dyn_result_retains_all_public_regions` replace the former descriptor home before
attempting a write to actual or same-lifetime sibling backing; both must reject.

Acceptance is native/Terminal exit 0 for all nine computed places and both
controls, with all nine source-9 twins and backing controls rejecting. These runs
are for macOS ARM64. Other-host execution, general opaque/provider and dependent
results remain open.

### Returned window extents

Each computed byte-view place also creates its snapshot through
`initial_snapshot`, returning `source.bytes[2..2 + WINDOW_LENGTH]` from a
record field. The completed result establishes length 7 before another operand
replaces its former home with length 9. The consumer uses that retained length
directly; the sentinel byte 49 checks the nonzero offset at runtime. Its twin
changes the window bound to 9 and must reject `ranged_original.bytes.len == 7`,
independently of the earlier twin obligation. All nine positive and negative
places have matching fact pins.

The fresh baseline refuses the helper’s own result-length contract in all nine
places. The repair records builtin window length as a mathematical difference
of its captured endpoints, embeds finite scalar endpoints in the existing
mathematical judgment, and normalizes inclusive and omitted bounds there. It
does not replay expressions, grant custom range machines builtin geometry,
justify endpoint arithmetic, or establish storage access.

`controls/dynamic_nonzero_window` retains the original failing dynamic-call
witness; `controls/returned_window_extents` executes symbolic endpoints,
inclusive bounds, open suffixes and empty windows. Their pins are
`pass/slices/runtime_nonzero_window_result_snapshot_exit` and
`runtime_observed_window_extents_exit`. The reject pins
`fail/slices/returned_window_extent_mismatch`,
`returned_inclusive_window_extent_mismatch`,
`window_extent_does_not_establish_bounds` and
`window_extent_does_not_form_endpoint` preserve false-length, bounds and overflow
refusals. Acceptance is observed native/Terminal exit 0 on macOS ARM64 for both
controls and all nine places, plus fact-sensitive rejection of their twins.
Other-host execution remains unverified.

### Applied callable field regions

The computed byte-view row now composes the existing ranked/dynamic result’s
`bytes` projection with a separate `sibling` input through `Composing::compose`.
Both named implementations construct a `View` from two explicit references;
the second returns an immutable local copy. Every place retains this same
production, length 7, overwrite to 9 and consuming operation. The source-9
reject twins require the `Composing::compose` result to fail at `receive`, as
well as the existing returned-window obligation. Earlier producers and their
once-only replacement counter remain exercised inside the composed expression.

The fresh baseline rejects both conformance maps in all nine places. Preparing
a declaration field retains its lexical owner; scoped lifetime traversal must
use the active application frame for that schema, including its fresh invocation
regions. That frame now carries its region map together with Type substitutions.
A supplied operand outside the active frames still re-enters its captured scope.
The result relation remains the conservative union of every matching input;
field names and length guarantees cannot choose a referent. Nested-call result
discovery also uses the shared completed-carrier classifier, removing a dynamic-only
borrow-free restriction that had rejected a projection of the existing result.
Call custody and cleanup still belong to the common call planner.

`controls/dynamic_multi_input_window` is the original failing two-input dynamic
window program, pinned as
`pass/borrows/runtime_dyn_multi_input_window_snapshot_exit`.
`controls/nested_applied_regions` applies one nested field schema twice with
independent result lifetimes, pinned as
`pass/borrows/runtime_nested_callable_field_regions_exit`.
`fail/borrows/dyn_multi_input_result_retains_actual_backing` and
`dyn_multi_input_result_retains_other_region` reject writes to either retained
input while consuming just the result’s byte field. Swapped nested output
regions, ambiguous two-input result elision and a shared candidate for mutable
output reject in `nested_callable_field_regions_swapped`,
`callable_multi_input_elision_needs_relation` and
`callable_shared_candidate_cannot_supply_mutable_result`. Existing captured/fresh
scope controls remain part of focused validation. Runtime qualification is
macOS ARM64 native/Terminal agreement; other-host execution and the broader
opaque/provider/dependent result acceptance remain open.

### Static callable result contracts

Each computed byte-view place now returns its existing composed value through
`retain_callback<callback_keep>` before the later overwrite. The callback’s fresh
invocation contract declares both descriptor-length guarantees; the wrapper
proves those guarantees through that public signature. It does not inspect the
selected callback body. All earlier ranked/dynamic producers, separate input
regions, the length-7 consumer and the once-only replacement counter remain in
the same expression. The source-9 twins still reject at that consumer and at the
existing returned-window obligation.

The fresh baseline rejects both wrapper guarantees in all nine places. Shared
contract discovery now recognizes the static callable binder’s declared
signature. Completed locals in the straight-line proof path publish through the
same storage-binding operation as the general statement path. This preserves the
executed result’s identity after a conservative call frame retires earlier
storage observations; it does not revive an old initializer or pointee contents.
No special field, callback implementation or source arrangement supplies an
extra equation.

`controls/static_callback` preserves the original failing wrapper, pinned as
`pass/borrows/runtime_static_callback_snapshot_exit`.
`controls/writing_static_callback` retains a named callback result while the
callback toggles a mutable counter, pinned as
`pass/borrows/runtime_writing_static_callback_snapshot_exit`. The runtime oracle
checks that the callback ran once and that overwriting the original descriptor
left the result’s length at 7.

The reject controls are `fail/borrows/static_callback_requires_public_field_guarantee`
(a stronger selected implementation cannot replace a missing public clause),
`static_callback_body_must_establish_guarantee` (the body must prove its promise),
`static_callback_guarantee_needs_entry_premise` (a guarantee cannot establish its
own call’s precondition), and
`static_callback_result_does_not_restore_entry_contents` (a returned reference
does not restore facts about contents changed by the call). Qualification is
native/Terminal agreement on macOS ARM64; other-host execution remains open.

### Runtime-indexed byte-view results

The selected value of a copied first argument remains 7 when the second argument
replaces its former descriptor home with a view of 9.

Each of the nine places has an `indexed_byte_view/main.omg` witness and an
`indexed_byte_view/invalid/main.omg` twin. Both direct and public `dyn` producers
return `Counted<'storage, (source.bytes.len)>` from a local array-backed view.
The receiver accepts `Counted<'storage, 7>` and consumes its own default-domain
invariant through `seven`; it needs no redundant length precondition. The caller
subtracts 7 and checks that the former home became 9 while its sibling stayed 4.
Twins change only the initial array extent and contents to length 8. Pins are
`tests/omega/{pass,fail}/facts/indexed_byte_view_argument_snapshot_through_*_exit`.

Baseline `970629516baaa28053e7f4cdeb5eeebb0d6c3721` refuses the field-valued
application during parsing. The repair retains the expression until binder-owned
admission, uses the common arithmetic formation walk in its exact declaration
scope, and joins source Value binders to their represented descriptor fields.
Completed results retain index equations under their captured operands. Public
slot preparation remaps that entire value telescope; storage identity remains
separate from source index equality. The nine positives execute with native and
Terminal exit 0 on macOS ARM64, and all nine twins reject index equality.
Other-host execution remains unverified; Windows execution is owner-deferred.

Formation controls reject unused overflowing recipes, wrong/refined carriers,
a provider returning a different projection, forged descriptors and retagging an
already admitted value. A bounded unused recipe remains valid. Those controls
are under `tests/omega/{pass,fail}/generics/runtime_value_recipe_*`.

### Inferred runtime indices

The same nine places have `inferred_indexed_byte_view/main.omg` witnesses and
`inferred_indexed_byte_view/invalid/main.omg` twins, pinned under
`tests/omega/{pass,fail}/facts/inferred_indexed_byte_view_argument_snapshot_through_*_exit`.
Their consumer infers an ordinary machine Value argument from the completed
`Counted` operand. Both direct and public producers toggle a separate counter;
the caller checks each counter to detect repeated execution. The false-length
twins change only the initial array from seven to eight elements and reject
`Count == 7`.

The optimized baseline at `a600523a236621d61c5033f0bed399346a297f15` tries to
specialize the runtime index as a closed constant and imports the producer-local
`source` spelling. Inference now retains the exact call, operand ordinal and
represented descriptor. Source checking and executable retargeting consume the
same completed-value capture. The logical index remains a value, while the
shared executable instance specializes only its carrier. Each prepared cloned call
retains its own authenticated operand capture; hidden scalar arguments observe an
already completed value, preserving evaluation order and once-only effects.
All nine positives execute with native and Terminal exit 0 on macOS ARM64,
and their nine twins reject. `controls/cloned_inferred_capture` exercises a
generic caller, `controls/statement_inferred_capture` exercises statement calls,
and `controls/repeated_inferred_capture` compares two independently completed
indices. Its unequal-index twin rejects; equality uses the normal observer’s
established equations rather than assuming two distinct captures are unequal.
Other-host execution remains unverified.

The parent remains open. The post-write productions below establish earlier-operand
descriptor observations. General opaque-provider and cyclic result composition
remain required; this does not establish full RUNTIME-VALUE-GENERICS acceptance.

## Requirements and evidence

- Establishment: initial 7 and the completed operand capture establish the fact; checked call/arrival contracts carry it, never the later 9 or an unproved goal.
- Identity and lifetime: the copied first operand is detached; `overwrite` changes its original home, not that copy. Descriptor copies retain their exact backing permissions independently of that home; byte-field observations retain custody of their containing record.
- Composition: all nine places consume the same snapshot through `seven`, checked result equality and exact subtraction. Twins change the source to 9, not a caller guard.
- Soundness: ordinary exclusive access authorizes the overwrite; operand facts grant no loan or disposal authority. Unknown/missing captures participate conservatively.
- Checked-to-Terminal Psi and native realization: every positive binds all four hosted targets and declares exit 0. Acceptance requires observed native exit 0 and interpreter exit 0 through `omega run --both`; flags alone establish no other-host execution.
- Provider/layout/ABI: the descriptor rows pass shared-reference and byte-view records through ordinary field layout and call/return placements, including its call-result place. It does not establish every aggregate ABI or target lane.
- Instruction/frame/encoding/image realization: host execution exercises ordinary indirect loads, argument staging, frame homes, calls and direct Mach-O emission. No claim is made for unexecuted targets, concurrency, or unrelated instruction paths.
- Remaining scope: these witnesses establish scalar, shared-reference and shared byte-view operand snapshots. Inline image sources also preserve the untouched sibling through the ordinary stored-call relation; computed image/input results and binding-preserving effects are also covered; this does not establish every computed or opaque output relation. The rest of both parent outcomes, including general computed/cyclic composition, remains open.

Actual baseline diagnostics, producer/source/oracle identities, engine observations and reject reasons belong in the owning qualification/claim records. No unexecuted target is claimed.


## Earlier operands establish the descriptor selected by a later operand

The selected value of a copied first argument remains 7 when the second argument
replaces its former descriptor home with a view of 9.

The nine `<place>/post_write_indexed_byte_view/main.omg` programs first install
an array window into a descriptor whose old extent is 1, then capture a dependent
result, then replace the former descriptor with extent 9. Both direct and public
trait result producers toggle an independent counter exactly once. The receiving
`Counted<7>` invariant, `seven` contract and exact subtraction consume the captured
extent without a caller equality guard. Each twin changes only the installed
backing from 7 to 8. Matching pins are
`tests/omega/{pass,fail}/facts/post_write_indexed_byte_view_argument_snapshot_through_*_exit`.

Fresh optimized baseline `a69dd2d132b727bfbcb28f1ff4920e803a38eef0` refuses all
nine positives at runtime-index equality. Invocation write frames now consume the
completed operand origin roster, rather than reloading pre-operand descriptor
slots. Unknown origins remain unknown and only poison writes that need them.
Taking an owned address does not read a stale reference binding. Ordinary loan and
move checks still authorize the address. Fresh post-write descriptor observations
consume admitted call guarantees; erased integer arithmetic retains its meaning
when compared by the existing scoped integer engine. Inverse alias invalidation
remains conservative.

The same nine places also contain
[`public_post_write_indexed_byte_view/main.omg`](local/public_post_write_indexed_byte_view/main.omg)
with matching `invalid/main.omg` twins, pinned under
`tests/omega/{pass,fail}/facts/public_post_write_indexed_byte_view_argument_snapshot_through_*_exit`.
Here an ordinary public `Installing` requirement establishes the changed extent;
the named implementation deliberately renames its formals. Fresh optimized
baseline `4d987059e887d2712b1c7679f6577008f8f5e20d` rejects the valid public
postcondition and wrongly accepts a separate implementation promising the wrong
mutable formal. Law correspondence now preserves applied declaration identity,
selected operator meaning, and positional formal roles.

The public call retains its complete evaluated operands, including the selector's
empty data frontier. Checked stored-output summaries join every selectable body
without importing private predicates. The captured supplier roster preserves both
the replaced view and its unchanged image-backed sibling. Terminal descriptor
replay likewise joins current executable post-state effects while keeping result
guarantees requirement-owned. An opaque lifetime debt never establishes spatial
containment or access authority. Mixed selector/data records retain their real
borrows, and an alternative implementation that also changes the sibling executes
through the same route.

These productions discharge STATE-LOCAL-VALUE-FRONTIER's earlier-operand descriptor
observation requirement for direct and public mutators with direct/public dependent
producers. All nine public positives execute with native and Terminal exit 0;
their false-length twins reject the indexed consumer. Missing public guarantees,
wrong formal/field guarantees, and mutation through a live selector-record data
borrow also reject. The old-extent control rejects, and borrowing an owned home
already consumed by an earlier operand still rejects.

The type invariant establishes the extent, operand captures preserve its exact
value epoch and backing, and the untouched sibling remains extent 4. Native and
Terminal execution observe the same result and counters, exercising checked
lowering, provider settlement, layout and calling plans, register/frame realization
and final image emission. These are macOS ARM64 observations. Windows execution
is owner-deferred; Linux execution is unverified.
