# Retained borrowed fixed-array views

Parent: [NORMALIZED-ABI-LOWERING](../../../../../TASKS.md). This extends
[retained nested backing](../README.md) to an explicitly borrowed array.

A mutable foreign registration retains the original fixed-array view in Buffer until Pending completes; completion restores the original Cell::Issued memberships and ResidentValue for observed_gap, writes 73, and preserves both neighbors, marker 91 and the earlier copy 37.

`Cell.value` carries `ResidentValue`: `0..=2147483647`, excluding 42.
`mint_cell` establishes each exact `Cell::Issued`; the stored reference is
`&mut [Cell in Cell::Issued; 3]`. After completion, `observe_cell` consumes that
membership before any runtime guard, and `observed_gap` consumes the complete
field predicate. After retiring Buffer, another qualified observation reaches
the original caller array. The captured scalar must still equal 37 while
completion's observed value and original middle slot equal 73.

The C counterpart stores `struct cell (*cells)[3]`, an explicit pointer to a
fixed array. Registration writes 41 and retains the original Buffer address;
completion writes 73 through that pointer. Both neighbors (305419896 and
1515863100) and marker 91 must survive. A private copied array cannot satisfy
both the Buffer and original caller observations. The C source and existing
native binding declarations are unchanged. Those legacy declarations remain
migration inputs; this row does not establish Build-owned native binding syntax.

| Place | Positive | Live-loan reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [read before completion](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [read before completion](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [read before completion](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [read before completion](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [read before completion](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [read before completion](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [read before completion](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [read before completion](other_machine/invalid/main.omg) |
| Target body | [program](target_body/main.omg) | [read before completion](target_body/invalid/main.omg) |

Every positive binds ProgramEntry on all four hosted targets. The match place
executes both alternatives with distinct route tags. Const names seed ordinary
values; the array and Pending are runtime occurrences. Each twin adds exactly
one read while Pending retains the exclusive loan. Matching corpus sources and
oracles live under `tests/omega/{pass,fail}/facts/foreign_borrowed_array_view_*`.

The [missing-mint control](controls/unissued/main.omg) rejects absent original
issuer provenance. The [missing field predicate control](controls/missing_leaf_predicate/main.omg)
rejects the complete `ResidentValue` obligation. The existing
[generic borrowed-root composition](controls/generic_borrowed_root/main.omg)
protects ordinary mutable handback independently of foreign registration.
Live-loan twins alone do not establish issuer preservation: stale/mixed completion,
other live registrations and uncovered extra qualifications remain separate
regressions of the shared retention operation.

| Requirement layer | Witness and remaining scope |
| --- | --- |
| Establishment | Exact `mint_cell` results supply Issued; receiving declarations do not mint it. Normal completion must restore the declared duties from the original supplied membership. |
| Identity and lifetime | Original captured input, applied lifetime, consumed Pending holder and complete overlapping cohort must identify the same loan. Caller-array observation checks its original backing. |
| Composition | The same Pending crosses all nine places; branch alternatives must each carry justified supply. The generic borrowed-root control retains the predicate-only composition. |
| Soundness boundaries | Missing mint and live-loan controls reject. Additional input-only root qualifications and stale scalar facts cannot be restored from the declared child contract. |
| Psi 05–07 | Current completion facts must become loan-completion qualifications and Terminal access on the actual restored storage. Old scalar 37 remains distinct from new value 73. |
| Omega 00–02 | The C pointer-to-array ABI, exact record/array geometry, admitted provider and ordinary call plans remain unchanged; routed supply is semantic preservation, not C byte equality. |
| Omega 03–06 | Guards check original pointer writes, values surviving calls, selected routing and untouched neighbors. The retained public runs observe the original caller storage and all canaries after completion. |
| Omega 07–09 and emission | All nine authored zero exits require actual native/Terminal agreement on matching hosts. macOS public execution and independent native custody replay pass; matching Windows and Linux observations remain pending. |

The [foreign-storage contract](../../../../../wiki/spec/build/foreign_storage.md#outbound-custody),
[mutable declared-field duties](../../../../../wiki/spec/language/dependent_values.md#mutable-receivers-and-declared-field-domains)
and [structural access contract](../../../../../wiki/spec/terminal-psi/structural_access.md)
authorize preservation through the exact completed loan. The provider's admitted
behavior remains a trust premise; completion is not a new Issued establishment route.

At fixed published baseline `2ef45342f25`, the original local predicate-only
witness source-checks, but all nine strengthened routed positives refuse after
completion. The local case loses three Buffer child memberships and the qualified
Buffer/caller observations. All nine live-loan twins still reject for their
intended read. Baseline compiler, sources, diagnostics and independent preflight
are retained under `/Users/zcanann/Omega-build/retained-routed-handback-2026-10-28/`.
At optimized producer `89e749b84c9`, all 62 retained public native/Terminal
pairs pass on macOS with Terminal format 202, including this strengthened row.
Eleven direct invalid controls retain their intended refusal. The fixed-base
checking comparison recovers the nine strengthened positives and preserves the
other 78 outcomes. Its single stale compiler-review refusal and explicit
review-only retry remain in the evidence. The replacement/weaker-formal
controls preserve their intended pass/reject split, and the extra caller-root
qualification still refuses at native 0 / Terminal 201.

The independent native custody audit exposed a separate interference error:
a provider output’s validity anchor was mistaken for its pointee. The repair
authenticates the captured spatial ancestry before testing that anchor’s
interference; its lifetime debt remains live. The integrated producer also
executes all nine pinned positives and the generic borrowed-root control at
native 0 and Terminal 0 under that audit. Earlier `030fde8a16e` and
`796a65b12e4` observations retain their actual producer identities. Matching-host
Windows and Linux runs of the unchanged mechanism at `796a65b12e4` are in
progress; the integrated macOS row separately covers the combined wire format.
These results do not close the parent’s remaining codecs and ABI lanes.

Historical predicate-only acceptance remains separate: producer `1d92ab9b09e`
passed ten corpus and ten public native/Terminal pairs plus ten intended rejects
on macOS and matching [Linux x64](https://github.com/CathedralOS/Omega/actions/runs/40581713175),
[Linux arm64](https://github.com/CathedralOS/Omega/actions/runs/40581713402) and
[Windows x64](https://github.com/CathedralOS/Omega/actions/runs/40581713496).
Its immutable producers, raw source/engine observations and reviews remain in
`/Users/zcanann/Omega-build/qualified-borrowed-array-view-2026-10-27/`;
the earlier descriptor codec baseline and repair remain in
`/Users/zcanann/Omega-build/retained-borrowed-array-view-2026-10-27/`.
Those observations do not qualify the new routed requirement or close the parent.
