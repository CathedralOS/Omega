# Captured window descriptor retirement

Replacing the exact captured window descriptor releases its displaced backing while retaining replacement and disjoint sibling loans.

Parent: **BORROWED-STORAGE-RESTORATION** in [TASKS.md](../../../../TASKS.md).
The governing laws are [captured places and loans](../../../../wiki/spec/terminal-psi/loans.md),
[structural reference leaves](../../../../wiki/spec/terminal-psi/structural_access.md),
[assignment evaluation](../../../../wiki/spec/language/expressions.md#evaluation-schedule),
and [borrowed-storage restoration](../../../../wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration).

Two windows initially borrow distinct one-cell arrays. Replacing element zero's
slice descriptor installs a two-cell replacement. The caller then mutates the
displaced original backing to 9, observes its unchanged tag 17, reads replacement
values/tags 42/7 and 19/29, and verifies the untouched neighbor 31/41. The store
captures its destination before the RHS. Retirement ends the old descriptor's
loan; it neither disposes borrowed backing nor invents rollback.

The selector crosses all nine places: [local](local/main.omg),
[field](field/main.omg), [parameter](parameter/main.omg),
[call result](call_result/main.omg), [match join](match_join/main.omg),
[constant](const_name/main.omg), [nested machine](other_state/main.omg),
[another machine](other_machine/main.omg), and
[Build-selected body](target_body/main.omg). Every place keeps the sentence,
bound and operation above. Its `invalid/main.omg` twin changes only the store's
selector to the completed call returning one. That store cannot release the
original backing still held by window zero; the rejection must name its active
window loan. The nine positives and twins are pinned under
`tests/omega/{pass,fail}/facts/captured_window_slot_retirement_through_*`.

The frozen optimized `9d0ba50bdc865c2100f7b06763b6706b70bbdcbc`
producer wrongly refused all nine positives when `check` loaded
`windows[0].values` and `windows[1].values`: the selected descriptor lacked a
retained loan occurrence. An incoming collection's leaf family is a real
permission roster, but its enclosing pointer is not any selected child's
pointer. Selection now joins the exact current incoming slot from the completed
storage frontier to that roster. The family identity, captured descriptor-owner
selection and suffix below its loaded pointer remain separate.

Completed stores also retain their actual value producer. A replacement follows
its new value, while an earlier copied descriptor retains its older observation.
The current installed descriptor rejoins its own formation and complete input
alternatives before following construction inputs. Source retirement and native
custody read the same recorded installation home; a stable exact store can refine
a coarse dynamic owner, while unavailable and unknown stores remain conservative.
This supplies geometry and correspondence, not new access, bounds or lifetime.

The [composition witnesses](compositions/) exercise arbitrary incoming selectors,
distinct selected family permissions, nested record owners, a saved descriptor
across replacement, restoration followed by another replacement, and selected
or copied replacement values. Runtime checks distinguish replacement 42 from
neighbor 31, old descriptor 7 from replacement contents, and the restored
one-cell descriptor from its two-cell replacement. Always selecting element zero,
retargeting an earlier descriptor, choosing the wrong RHS arm, or omitting a
restore therefore traps. Controls retain the old saved descriptor, neighbor,
replacement and unknown-selector obligations, reject overlapping selected loans,
and require the incoming collection's independent extent proof. These programs
are pinned beside the nine-place fixtures.

The target-body place uses ordinary named implementations and explicit Build
provider rows. Distinct architecture tags make wrong provider selection observable
at runtime without exporting private implementation guarantees. Every positive
binds the four required hosted targets; this place also binds macOS x86-64.

Fresh optimized producer `c719bad8ddf365b4342b240f9e0b3c250515946c`
ran the nine original positives and seven composition positives with native 0
and Terminal 0 on macOS ARM64. The nine original twins and six composition
controls reject their authored obligations. A Terminal positive exits 0 and two
wrong-provider counterfactuals produce typed Trap. Focused pinned-fixture and
host-installation qualification accompanies the PR; other-host native execution
and the wider borrowed-storage restoration acceptance remain open.
