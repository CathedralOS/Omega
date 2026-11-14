# Scalar-recast parent restoration

A scalar store of zero through the u32 view restores Pair.start and its mixed parent predicate before returning the parent.

Owner: **CANONICALIZE-SCALAR-RANGE-CONTRACTS** in [TASKS.md](../../../../TASKS.md).
This row establishes checked normal-call restoration through an authenticated
scalar view, including a view forwarded through a nonmutating call.

`Pair` requires `start >= 0 && start <= end`. Each public `inspect` receives a
valid parent, forms a u32 view of its signed i32 `start`, and forwards that view
through `retain_view`. The ordinary `restore` helper stores all-one bits and
then the transported restoration scalar; its checked `ensures view == replacement`
supplies the new value. Handback must interpret that guarantee in the original
signed storage and prove the complete parent predicate at this call's return.
The formal's unrestricted u32 validity cannot supply the parent's predicate.
The later caller store `pair.end = 10` cannot repair an earlier handback.

`main` uses two parents with distinct initial starts (3 and 5), origins, markers
and neighboring hex canaries. It checks restored start 0/end 10 and those
canaries. The match place exercises both arms with distinct marker results;
the field carrier's neighbors remain observable too. These runtime guards
supply no proof to `inspect`'s earlier obligation.

| Place | Positive | Reject twin | Restoration scalar |
| --- | --- | --- | --- |
| Local | [source](local/main.omg) | [source](invalid_local/main.omg) | Local binding |
| Field | [source](field/main.omg) | [source](invalid_field/main.omg) | `Replacement.value` |
| Parameter | [source](parameter/main.omg) | [source](invalid_parameter/main.omg) | Exact entry contract |
| Call result | [source](call_result/main.omg) | [source](invalid_call_result/main.omg) | Identity result |
| Match join | [source](match_join/main.omg) | [source](invalid_match_join/main.omg) | Both identity/marker arms |
| Const name | [source](const_name/main.omg) | [source](invalid_const_name/main.omg) | `RESTORING` |
| Nested machine | [source](other_state/main.omg) | [source](invalid_other_state/main.omg) | Nested identity result |
| Another machine | [source](other_machine/main.omg) | [source](invalid_other_machine/main.omg) | `Other::identity` |
| Selected body | [source](target_body/main.omg) | [source](invalid_target_body/main.omg) | Selected implementation |

Each twin changes only zero to all-one restoration bits. The parameter twin
changes its matching exact contract and both inputs together. All explicit
entry contracts remain satisfied; the intended refusal is **cannot lend
restricted field `start`**, with **normal-return guarantees do not preserve the
field's standing predicate**. Unrelated formation refusals do not count.

Every positive declares exit 0 and all four hosted `ProgramEntry` bindings.
Exact sources, Builds and oracles are pinned under
`tests/omega/{pass,fail}/facts/recast_parent_restoration_through_<place>_*`.
Other-host execution remains unverified.

## Composition and authority controls

- [Argument snapshot](controls/argument_snapshot/main.omg) copies zero before
  a later argument changes its source local to all-one bits. A count and marker
  expose ordered, single evaluation. Its [twin](controls/argument_snapshot/invalid/main.omg)
  captures all-one bits before the later mutation to zero; the later value
  cannot repair the earlier actual.
- [Completion order](controls/completion_order/main.omg) nests two returning
  calls in one expression. The marker chain (17 then +6 to 23) and distinct
  returned values expose order and omitted effects. Its
  [twin](controls/completion_order/invalid/main.omg) breaks only the first
  restoration; a later restoring sibling cannot excuse it.
- [Forwarding marker](controls/forwarding_marker/main.omg) changes a disjoint
  marker while forwarding the formed view. Its
  [twin](controls/forwarding_marker/invalid/main.omg) breaks restoration.
  [Dirty forwarding](controls/dirty_forward/main.omg) calls the forwarding helper
  while the parent is already invalid. On integrated compiler
  `2713116252386e98ce5e87961aca764f8760b217`, dirty forwarding rejects the
  unproved `Pair` requirement at the call. The forwarding-marker twin rejects
  that requirement on return. These are the intended restoration refusals.
  The [hosted observations](https://github.com/CathedralOS/Omega/actions/runs/41756991925)
  retain the old oracle failure; the corrected expectations match their actual
  diagnostic messages. Earlier loan-only refusals keep their original source
  identity and do not stand in for these restoration observations.
- [Case backing](controls/case_backing/main.omg) and its
  [nonmutating form](controls/case_backing/nonmutating/main.omg) retain the
  selected `Envelope::Some` storage path. The
  [overlapping helper](controls/case_backing/invalid/main.omg) rejects immediately,
  despite a later restoring caller store.
- [Preserved end](controls/preserved_end/main.omg),
  [unrestored start](controls/unrestored/main.omg) and
  [changed end](controls/changed_end/main.omg) distinguish untouched sibling facts
  from missing restoration. Matching corpus pins retain each distinction.

The exploratory attempt to lend both coupled `start` and `end` exclusively is
an authority negative: a dependent loan pins its validity witnesses
([dependent values](../../../../wiki/spec/language/dependent_values.md)).
Physical disjointness cannot erase that restriction. No loan gate is relaxed.
Write-only equality probes refuse specification formation on baseline and
repair; they establish no new write-only guarantee API.

## Requirements and evidence

Construction establishes the initial predicate. Writes invalidate exact current
storage, while immutable argument snapshots keep their earlier meaning.
Normal-call transfer applies checked guarantees to all returned cells together,
with source/view representation translated at the authenticated boundary.
The complete parent is judged before later siblings, never assumed from its
own declaration. Source validity, access authority and loan lineage remain
separate obligations ([recasts](../../../../wiki/spec/layouts/recasts.md),
[fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md),
[loans](../../../../wiki/spec/terminal-psi/loans.md)).

The unchanged programs continue through Terminal Psi, ordinary call plans,
signed/unsigned shared geometry and native emission. Current observations cover
macOS ARM64; canaries detect wrong stores, origins, carrier neighbors and branch
selection. [PR #1386](https://github.com/CathedralOS/Omega/pull/1386) retains exact
producer profiles, frozen inputs, raw results and independent review. Earlier
productions and failures remain with [PR #1323](https://github.com/CathedralOS/Omega/pull/1323)
and [PR #1345](https://github.com/CathedralOS/Omega/pull/1345).

The exact optimized baseline `be5c01dd42f9038cf62412bd94a67a23b6658241`
(SHA-256 `ad370b5f5a8ae3ae45520cb2f9f0225a5388cc625d840a993e41bea14f680b53`)
wrongly refuses all nine positives. The repair executes all nine positives and
both snapshot/order controls with native/Terminal exits 0/0; their twins reject
the intended immediate handback.

The clean pre-integration compiler checkpoint
`c0be56a86d69ae34375a16c096f9099e7bce4b11` produces optimized runner
SHA-256 `a54a3666bc77398105ab50160295026c68102210a4a229546b26f514ccb8775d`.
Its 70-fixture source selection checks 34 positives, rejects 35 intended controls
and retains one attributed baseline lowering refusal. The 35-fixture execution
selection records 33 actual native/Terminal agreements, one compile-only result
and that same refusal (`computed borrow lost its established local`); its gate
exits 3. It is not an all-green result. The explicit zero-oracle names for the
two new controls were corrected and those four fixtures remeasured separately.
Actual integration observations are recorded separately in the PR evidence.

This closes this checked normal-call production of the row. Broader scalar-range
migration, other representation/origin requirements, common-field case rosters
and recursive/reference predicate inventories remain open under the parent task.
GUI qualification stays pending.


After the actual golden conflict and affected incoming custody/layout changes,
a fresh optimized integrated producer at `0a3a759c64e3ba71d5b04863d6e2e7a2fa9e8ff7`
(SHA-256 `6770f858f84ee26cc676345b5fdf9413036bfa5d52531b37620a3e06afd8ead3`) checks the focused
39 fixtures: 19 positives and 20 intended rejects. Its execution
selection records 18 actual native/Terminal exit-0 agreements and
one compile-only result; both focused gates exit 0. These include the complete
nine-place row, snapshot/order controls and selected affected custody/authority
consumers. Earlier 70-fixture observations and their baseline refusal remain
separate. Golden publication retains changed-source or changed-outcome rows;
all raw measured rows remain in the PR evidence.
