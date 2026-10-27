# Retained borrowed fixed-array views

Parent: [NORMALIZED-ABI-LOWERING](../../../../../TASKS.md). This extends
[retained nested backing](../README.md) to an explicitly borrowed array.

A mutable foreign registration retains the original fixed-array view in Buffer until Pending completes; the restored middle Cell satisfies ResidentValue for observed_gap, while completion writes 73 and preserves both neighbors, marker 91 and the earlier copy 37.

`Cell.value` carries `ResidentValue`: `0..=2147483647`, excluding 42.
After completion, `observed_gap` requires that complete declared predicate and
subtracts the field from 2147483647; the caller checks the resulting 2147483574
alongside the original canaries. No caller guard re-establishes membership.
The compact [missing field predicate control](controls/missing_leaf_predicate/main.omg)
keeps an unqualified public Cell input and must reject the same consuming call.
The nine exclusive-loan twins remain separate guards on the retained window.

The compact [generic borrowed-root control](controls/generic_borrowed_root/main.omg)
passes `Wrapper<Cell>` through ordinary mutable calls, consumes the same predicate,
and checks the original backing after retiring its view. It protects the shared
call-publication path independently of foreign registration.

The logical-subject migration now covers `ResidentValue` in all nine places,
their pins and both focused controls: its three predicate references use `self`
without changing the bound, excluded value, runtime operations or loan twins.
At main `e92e8dfe666f5a858ce22b338075e5aee0b8167c`, the old binders stopped all
20 source fixtures at the retirement diagnostic, including the intended rejects.
With the renamed sources, that same optimized producer records ten checked
positives, ten intended rejects, ten corpus native/Terminal zero agreements and
ten public zero agreements on macOS ARM64. Public products pass canonical
Terminal inspection. Receipts `2c13682be12743269fdea422e93071a2`,
`ef91ddd173434e0bbafc1eda3f8959ec` and `c2115d65d72743708b2f0c7c44e69b62`
retain the producer, matching-host C fixture, replay locks and raw observations.
Other-host execution is not claimed by this migration. The broader
**DOMAIN-DEFAULT-PREDICATE-UNIFICATION** acceptance remains on the task board.

Optimized producer `1d92ab9b09e` executes all nine qualified view places and that
composition control with native/Terminal zero agreement on macOS ARM64. All ten
intended source rejects hold; the scoped corpus observes ten checked positives,
ten rejects and ten native/Terminal agreements. The 43 earlier retained-pointer
programs also agree. Independent source and runtime reviews, exact producers,
source snapshots and native/Psi products are retained under
`/Users/zcanann/Omega-build/qualified-borrowed-array-view-2026-10-27/`.
The failed first aggregate driver remains failed: it required package review for
an intentionally invalid loan read after all ten positives had completed. Only
the unrun source controls were executed separately.

The frozen `f5e853ea1f2` baseline source-checks all nine strengthened positives
but fails lowering with `structural result custody cannot manufacture an owned
reference`; the unqualified predecessor still executes successfully. A generic
borrowed-root regression exposed by the initial repair is fixed without weakening
its complete predicate or exact nominal carrier lookup. Baselines remain under
`/Users/zcanann/Omega-build/qualified-borrowed-array-view-probe-2026-10-27/`.
Linux and Windows runs for `1d92ab9b09e` remain pending. The older matching-host
records below establish only the unqualified predecessor, not this extension.

`Buffer<'cells>` contains `&'cells mut [Cell; 3]`. The C counterpart stores
`struct cell (*cells)[3]`: one explicit pointer to a fixed array, with no C
array decay or inferred extent. Registration writes 41 and retains the original
Buffer address; completion writes 73 through that same stored pointer.
After completion the program reads through Buffer, retires it, and independently
reads the original caller array. A private copied array cannot satisfy both
observations. Nonzero neighbors and marker detect wrong offsets; the earlier
scalar copy must retain 37.

The nine places carry the same Pending and retained loan:
[local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[nested machine](other_state/main.omg), [another machine](other_machine/main.omg),
and [target body](target_body/main.omg). Every positive binds all four hosted
ProgramEntry targets. The match calls both branches with distinct route tags
and checks the selected tag. The constant is an ordinary initial Cell array;
the mutable runtime array and Pending remain runtime occurrences.
Each `invalid/main.omg` adds exactly one read while Pending still holds the
exclusive loan, and must reject for that read. The complete row and twins are
pinned as `tests/omega/{pass,fail}/facts/foreign_borrowed_array_view_*`.

The [foreign-storage contract](../../../../../wiki/spec/build/foreign_storage.md#outbound-custody),
[explicit boundary references](../../../../../wiki/spec/build/boundary_shapes.md),
and [structural reference storage](../../../../../wiki/spec/terminal-psi/structural_access.md)
authorize this existing operation. Entry establishes Buffer::Owned and acquired
Capacity::Available; the checked registration publishes Pending::Retained.
Completion consumes that claim, restores the caller's loan window, and returns
the capacity for release. Neither geometry nor lexical lifetime equality proves
an opaque call preserved a pointer. Mutable reference outputs remain unknown
content for static alias invalidation; they never reacquire a declared owner.

Terminal captures a finite typed descriptor graph through live checked
projection, gives each original allocation one persistent native home, and asks
the isolated provider worker to link its own mapping addresses. Actual C pointer
outputs select a typed endpoint from the current captured graph; Core still
checks capture identity, access and every lifetime anchor before transactional
restoration. Unmodeled pointer changes refuse rather than leaving stale Core
references. The provider's admitted contract remains a trust premise.

The immutable optimized baseline at afb0c236efd wrongly refuses the original
local witness at registration for missing caller-storage authority. The source
repair captures descriptor-leaf origins at actual operand evaluation, without
weakening the whole-statement mutation fence; opaque returned references stay
unresolved content with conservative invalidation. Initial repaired native
execution returned zero while Terminal declined its missing reference codec.
The repaired row now has actual native and Terminal zero exits at all nine
places on macOS ARM64, with nine intended source rejections. Raw baselines,
producers, profiles and engine observations are retained in
`/Users/zcanann/Omega-build/retained-borrowed-array-view-2026-10-27/`.
Matching-host Windows x86-64 execution at `258918eb09a` also records nine
corpus and nine public CLI native/Terminal zero agreements, with nine intended
loan rejections ([run 40554860652](https://github.com/CathedralOS/Omega/actions/runs/40554860652)).
After exporting `nestlease_register_view` under `RETAIN_1.0`, Linux x86-64 and
Linux ARM64 at `3e1d8468dfb` record the same view-row outcomes
([x86-64](https://github.com/CathedralOS/Omega/actions/runs/40560630817),
[ARM64](https://github.com/CathedralOS/Omega/actions/runs/40560630630)).
Their producers, C libraries, native/Psi products and raw reports retain those
original source identities under
`/Users/zcanann/Omega-build/retained-view-elf-export-2026-10-27/` and
`/Users/zcanann/Omega-build/native-provider-test-codec-2026-10-27/`.
The broader runs remain red: the older array constant rejects with its declared
range diagnostic (now pinned by both public and corpus controls), and the Windows
mutable-borrow call-result case has an unresolved native timeout. Neither is
counted as successful whole-suite qualification.

This row establishes the finite borrowed fixed-array descriptor and its original
backing. Recursive/dynamic pointer graphs, newly introduced provider-owned
roots, opaque owned reference value/results, tagged carriers and nominal/routed
descendant memberships remain outside this evidence and keep the parent open.
