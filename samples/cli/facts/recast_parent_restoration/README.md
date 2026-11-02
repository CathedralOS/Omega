# Scalar-recast parent restoration

A scalar store of zero through the u32 view restores Pair.start and its mixed parent predicate before returning the parent.

Owner: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, specifically the scalar-recast
source-domain restoration gap in [TASKS.md](../../../../TASKS.md).
[Invariant windows](../../../../wiki/spec/language/dependent_values.md#invariant-windows)
require proof at return/termination; [exclusive recasts](../../../../wiki/spec/layouts/recasts.md#deferred-establishment-on-exclusive-storage-casts)
retain the original source's validity and handback obligations.
[Loan lineage](../../../../wiki/spec/terminal-psi/loans.md) connects the view to
its actual backing. A plain `u32` view's validity cannot establish the mixed
`Pair` predicate.

Each public `inspect` receives `pair: &mut Pair` and forms its own
`let view: &mut u32 = &mut pair.start as &mut u32`. It calls ordinary `restore`, whose checked `ensures view == replacement`
retains the transported scalar. The helper first stores `0xFFFFFFFFu32`
(signed -1), then that restoration scalar through the same view.
The complete parent predicate already holds when the helper is called.
Its unchanged end retains the old predicate's consequences; the new start must
come from the checked guarantee interpreted in the original signed storage.
The later `pair.end = 10` cannot justify this earlier call handback, and there
is no direct restoring store to `pair.start`. Restoration-value helpers finish before the window opens and promise ordinary
`ensures result == value`.
Each place now also forwards the already formed reference through
`retain_view<'storage>`, which returns it unchanged before the corrupting store.
The helper's plain `u32` formal does not promise the signed parent's predicate.
Its complete noninterfering frame preserves the caller's exact current facts;
the returned reference must retain the source origin for the later stores.

`main` starts two valid parents at start 3 and 5, end 10, then checks restored
start 0/end 10, both neighboring hex canaries, markers and origin
canaries (the field place records its carrier neighbors in those markers). The match arms return the same zero but write markers 11 and 13;
both arms execute, and a wrong selection traps. The field carrier has its own
hex neighbors, copied to parent markers and checked by `main`. These final guards test runtime
behavior; they supply no proof to `inspect`'s earlier handback obligation.

| Place | Positive | Reject twin | Restoration scalar |
| --- | --- | --- | --- |
| local | [source](local/main.omg) | [source](local/invalid/main.omg) | Local binding |
| field | [source](field/main.omg) | [source](field/invalid/main.omg) | `Replacement.value` |
| parameter | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) | Explicit `replacement: u32` with exact entry contract |
| call result | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) | Ordinary identity result |
| match join | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) | Two identity/marker arms |
| const name | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) | `RESTORING` |
| nested machine | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) | Lexically nested identity result |
| another machine | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) | `Other::identity` result |
| selected body | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) | Build-selected `SelectedRestoration` identity result |

Each twin changes only the restoration scalar from `0u32` to `0xFFFFFFFFu32`.
The parameter twin also changes its exact input contract and both call inputs
together, so no input requirement is intentionally violated. Identity contracts,
parent construction, first corrupting store and runtime oracles stay unchanged.
The current production's intended refusal is **`cannot lend restricted field `start``**
with **`normal-return guarantees do not preserve the field's standing predicate`**
at the helper's normal-return handback. A formation failure or an unrelated input-contract refusal
does not establish this obligation.

All positives declare `Expected exit: 0` and pin native/interpreter exit 0.
Every program has all four hosted `ProgramEntry` bindings; the selected-body
Build also selects the ordinary named implementation on all four targets.
Exact source, Build and oracle copies are pinned at
`tests/omega/pass/facts/recast_parent_restoration_through_<place>_exit/` and
`tests/omega/fail/facts/recast_parent_restoration_through_<place>_rejected/`.
Sample twins use `expected.txt` for their intended-invalid discovery/oracle.

## Requirements and evidence

1. **Establishment:** valid parent construction and a proved zero establish the
   repaired signed field and `start >= 0 && start <= end`; the temporary all-one
   bits open debt. Confirm the positive closes it and the twin cannot.
2. **Identity and lifetime:** the locally formed recast must retain `pair.start`
   as its exact storage origin, invalidate that field's old facts, and owe the
   containing parent's mixed predicate when the borrow returns.
3. **Composition:** field, exact parameter, identity results, both match arms,
   const, nested machine and selected provider must carry the same scalar fact
   into the store. No caller guard substitutes for that proof.
4. **Soundness boundaries:** unrestricted `u32` validity grants no signed-parent
   predicate. Both twin inputs satisfy all explicit entry contracts; rejection
   must be the unclosed parent domain, with authority and custody preserved.
5. **Psi 06–08:** qualify source checking, alias-store and loan restoration
   lowering, and actual Terminal Psi observations for all nine positives. The
   parent readback after `inspect` makes restoration observable; no affine
   cleanup or ownership-hole mechanism is claimed by this scalar row.
6. **Omega 00–02:** qualify four-target Build settlement, selected-body binding,
   signed/unsigned 32-bit shared storage geometry, field carrier layout and
   ordinary scalar/aggregate call plans. Bindings alone prove no execution.
7. **Omega 03–06:** qualify store width and address, register/frame homes and
   live values. Parent and field-carrier hex neighbors, distinct initial
   values/origins and both match markers are runtime canaries.
8. **Omega 07–09/emission:** qualify actual native images and Terminal agreement
   at exit 0 through `omega run --both`; record producer revision, executable
   digest/profile, target, command and observations. Actual macOS ARM64 results
   are recorded below; other-host execution remains unverified.

## Qualification

The optimized fixed-base producer `326591276188e2fcc3ca124ad47be809d7dba4ec`,
SHA-256 `dc6d0658694f118dcbaeb930cbf602b46b480c0f46b8a65a824500c96d30f74d`,
rejects all nine positive places at the parent default-domain obligation. It
also wrongly accepts the unrestored scalar-view control before any later direct
field write exposes the missing source valuation. The restored control accepts.
These are actual Source observations, distinct from native execution.

The optimized repair producer, SHA-256
`a10f855a139286aa0e672d567d5d48eeb3160631cb689f431c3545c3a24e27cb`,
uses that fixed base plus compiler patch
`c4b1bcc36858f3c0c1ef62b32a494d8a1a93dd8a745d4b6c87f9d34cb7a2bc3c`.
All nine canonical positives execute with native/Terminal exits 0/0 on
`macos_arm64`; all nine twins reject the intended parent obligation. Sources
and oracles remain unchanged during each capture.

Three focused controls retain the original failure and preservation boundaries:
[preserved end](controls/preserved_end/main.omg) restores only through the view,
[unrestored](controls/unrestored/main.omg) omits that restoration without a later
direct field write, and [changed end](controls/changed_end/main.omg) replaces the
sibling so its entry fact cannot justify handback. Matching corpus pins protect
these distinctions. The PR and claim retain exact source hashes, producer
receipts, raw observations, and independent review.
Other-host native execution, broader scalar-range migration and GUI qualification
remain open under the owning task.

The strengthened row transports the same restoring scalar into a formed view
that returns through an ordinary nonmutating invocation. The
[forwarding-marker composition](controls/forwarding_marker/main.omg) also writes
19 to a disjoint marker during that invocation; its twin changes only the
restoration scalar. The [dirty-forward control](controls/dirty_forward/main.omg)
opens the parent window before borrowing/calling: an empty frame cannot
establish a predicate that is already false.

On fixed base `446a86f964ce9c6f9a19317aa08e454ae7b4bd01`, optimized CLI SHA-256
`e8a618c30e2beb09604ee7663e27bffb343c3782f56931e7f772560160707c4e`
wrongly refuses all nine strengthened positives at `retain_view`'s call. All
nine twins also report that unrelated handback refusal alongside the intended
parent debt. The marker composition wrongly refuses too; stored-view Source
contrasts already distinguish valid restoration from its invalid twin.

The repair shares the declaration-owned predicate read set between field
independence and mutation-frame consumers. It proves the complete current
predicate at the pre-statement prefix only when the complete invocation frame
preserves its inline scalar dependencies. Borrowed/recursive read footprints
remain unproved without their own backing inventory; unknown frames supply no
preservation. Changed storage still owes checked normal-return evidence.
Post-store checks use that same captured-predicate operation at the completed
store prefix. A scalar interval envelope cannot substitute for a coupled data
predicate.

Optimized CLI SHA-256
`d116c7f72037033678dfcad7e12441a2823a0294d55b94196964587ee57e3718`
(base plus compiler patch retained in the PR evidence) executes all nine new
canonical positives with native/Terminal exits 0/0; all nine twins now reject
only the intended unclosed parent domain. These observations retain their
actual producer identity. The focused final-source corpus and independent
review are recorded separately in the owning PR.

General changed-source representation guarantees, common-field case-predicate
rosters, recursive/read-through-reference dependencies, other formed-view
productions and other-host execution remain open with the parent task.
GUI qualification remains pending.

Final-source checkpoint `4a65db8ea7a55553ea8e77d11678d6c129bd5335` builds
optimized corpus producer SHA-256
`e9c31754f6e35f95da96a1b7469e41e669b96de2da5f24a42df50b05679e4517`.
Its focused check leg accepts 26 positives and rejects 28 intended controls;
one additional positive has the attributed baseline lowering refusal.
The execution leg observes 25 native/Terminal agreements, including all nine
strengthened places and forwarding-marker composition. One additional fixture
is compile-only. The existing `runtime_scalar_field_local_loan_handback_exit`
control refuses Terminal production with `computed borrow lost its established
local`; the exact-source baseline CLI reproduces that refusal before execution.
The original failed gate, its new measured row and the baseline replay remain
retained. This is not a claim that the whole execution selection passed.


## Case-bearing backing coordinates

The [case-backed marker control](controls/case_backing/main.omg) and its
[nonmutating sibling](controls/case_backing/nonmutating/main.omg) wrap the same
`Pair` in `Envelope::Some`. Explicit incoming facts establish the current parent
predicate independently of the later stores. The helpers preserve its scalar
read set, and both parents' neighbors and markers remain observable.

The [overlapping-call control](controls/case_backing/invalid/main.omg) changes
only the helper by inserting an all-one scalar store. It must refuse the scalar
handback immediately, even though a later caller store restores zero. Canonical
case selectors distinguish semantic paths; write frames project them into
physical storage, where different case names cannot prove disjointness.
The retained R2 CLI wrongly source-checks all three controls. R4's coordinate
repair checks the two valid controls and rejects the overlapping helper for its
intended scalar-handback obligation.

Initial native production then refused the two valid controls at the native
reference source's duplicate path whitelist. Terminal's canonical typed
projection geometry already represents case-qualified storage; source lookup
must use that owner while retaining access and active-case obligations.
The frozen failed attempts remain in the evidence record. The optimized R5 CLI
then executes both valid case-backed controls with native/Terminal exits 0/0,
rejects the overlapping helper at handback, and repeats all nine original
places/twins plus the marker and dirty-forward controls successfully. Its actual
producer is `d1bc08fc53210f0fddffec513b887a9d3686ca44` plus compiler patch
`453b9476fa0e86ebb9d7f31f5288688e157d164570c22949e62df71be5ec9daa`,
executable SHA-256
`734b7cf202544897579aa655310bd9dc21248b06d2fdb7bad675d69efb7bc2fb`.


Final-code corpus qualification uses clean checkpoint
`8fce926f97351853c0165602a991352592ef43e6`, optimized runner SHA-256
`0a6461f6c5460dd3513139405eeb30a6a4627190acba54fd0f9668d82925ea9c`.
Its 66-fixture check records 32 checked positives, 33 intended rejects, and
one attributed baseline lowering refusal. The execution selection records
31 native/Terminal agreements, one compile-only result, and that same refused
local-loan fixture. The execution gate exits 3 for the retained baseline failure;
no execution or repaired acceptance is claimed for it.


## Checked restoring-call production

The primary nine places now consume their unchanged restoration scalar through
`restore(view: &mut u32, replacement: u32) ensures view == replacement`.
Both stores remain inside that helper; the caller's fact, bounds, two distinct
parents, markers, neighbor canaries and exit oracle are unchanged. Twins change
only the restoration fact and must refuse the original parent's immediate
handback. The source field's i32 meaning cannot come from the formal u32 type.

The optimized exact baseline `be5c01dd42f9038cf62412bd94a67a23b6658241`,
SHA-256 `ad370b5f5a8ae3ae45520cb2f9f0225a5388cc625d840a993e41bea14f680b53`,
wrongly refuses all nine positives at handback and state exit. The first repair
executes all nine with native/Terminal exits 0/0 and rejects their nine twins at
the intended handback. Its raw R1 build receipt's tracked patch is accompanied
by the retained added-module overlay; final-source qualification and independent
review remain required before publication. No later revision is labelled as that
actual R1 producer.

Normal invocation subjects now preserve the authenticated scalar representation
and exact backing. Incoming values use source-to-view interpretation; returned
cells use view-to-source interpretation. The existing call-completion owner
keeps immutable argument snapshots, stages all sibling outputs together, and
retires affected storage before installing new contents. Full parent handback
reads that exact completion, before later siblings. It no longer reconstructs
coupled predicates from independent intervals or a formal's carrier range.


The [argument-snapshot control](controls/argument_snapshot/main.omg) captures zero
for the restoring actual, then a later argument changes its source local to
all-one bits. The helper must consume the earlier immutable snapshot. Its twin
starts with all-one bits and changes the later local to zero: the later value
cannot repair the captured actual. A counter and caller-visible marker expose
omitted or repeated evaluation in both engines.

The [completion-order control](controls/completion_order/main.omg) restores zero
in the first nested call and again in a later sibling. Its twin makes only the
first restoration invalid; the later sibling cannot excuse that earlier handback.
Distinct returned values and marker writes expose reversed/omitted calls. Each
control has an exact reject twin and matching corpus pins in this same fact.

A separate exploratory attempt borrowed both `start` and its coupled `end` as
exclusive scalar inputs. It was correctly refused after the parent proof:
the live dependent loan pins its witness (`dependent_values.md`, invariant
windows). Physical disjointness does not authorize those competing loans; no
loan-compatibility rule is relaxed. This is retained negative evidence, not a
valid production or a passing runtime claim. Write-only specification equality
probes likewise refuse formation on both baseline and repair; they establish
no new write-only guarantee API. Existing write-only/read-authority controls
remain in the focused regression selection.
