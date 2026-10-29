# Record and owned-array predicate handback

A saved observation of an unwritten array element stays equal to 7 across a write to a disjoint element.

Parent: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, current record-field and owned-array predicate handback. The existing fact's **STATE-LOCAL-VALUE-FRONTIER** parent remains open. See the [nine-place production](../../predicate_handback/README.md) for acceptance and requirement mapping.

The holder starts at `[7, 1]`, with neighbors 11/13. Preserve independently copies its array, writes copied element zero to 4, and writes that value to original element one. Its Requires/Ensures remain `kept(&holder.items) == true`. Agree consumes the saved/current 7 fact before the explicit runtime check of current original element zero, original element one and neighbors. The twin changes only the final original destination from index 1 to 0.

Historical source 6a6c6ffa173d missed the explicit current-zero runtime canary. Its unchanged bytes and original R10 refusal records are preserved under P8 (`build/scalar-storage-versions-20261029/historical-field-6a6c6ffa173d/snapshot.json`). Original-source R1 b140 success was owner-reported. This refreshed pair adds the runtime canary and agree's accurate output guarantee `result == observed`; it is no longer byte-identical to that commit. Preserve's Requires/Ensures, agree's requirement, and all mutation operations are unchanged.

A fresh optimized R10 replay of all nine final positive sources, including this field, still refuses preserve's own Ensures; hashes and raw diagnostics are retained in `build/scalar-storage-versions-20261029/final-source-baseline-observations.json`. Optimized R5 producer `383ffb829d07` freshly checks the positive, rejects the twin for preserve's own Ensures, and executes the positive and pin with native/Terminal exits 0/0 on macOS arm64. The nine-place production links the complete focused evidence. Both parents retain their remaining acceptance.

The integrated R6 producer `5c5c178f82c7` repeats these source and runtime oracles after main `66fc7ac3c97b`; the production README links the separately retained integration evidence.
