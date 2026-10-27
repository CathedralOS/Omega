# Retained borrowed fixed-array views

Parent: [NORMALIZED-ABI-LOWERING](../../../../../TASKS.md). This extends
[retained nested backing](../README.md) to an explicitly borrowed array.

A mutable foreign registration retains the original fixed-array view in Buffer until Pending completes; completion writes the middle caller cell to 73 while both neighbors, marker 91 and the earlier copy 37 remain unchanged.

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
Other-host execution remains to be qualified by the retained foreign gate.

This row establishes the finite borrowed fixed-array descriptor and its original
backing. Recursive/dynamic pointer graphs, newly introduced provider-owned
roots, opaque owned reference value/results, tagged carriers and independently
qualified descendants remain outside this evidence and keep the parent open.
