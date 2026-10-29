# Record and owned-array predicate handback

A saved observation of an unwritten array element stays equal to 7 across a write to a disjoint element.

Parent: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, current record-field and owned-array predicate handback. The existing fact's **STATE-LOCAL-VALUE-FRONTIER** parent remains open. See the [nine-place production](../../predicate_handback/README.md) for acceptance and requirement mapping.

The holder starts at `[7, 1]`, with neighbors 11/13. Preserve independently copies its array, writes copied element zero to 4, and writes that value to original element one. Its Requires/Ensures remain `kept(&holder.items) == true`. Agree consumes the saved/current 7 fact before the explicit runtime check of current original element zero, original element one and neighbors. The twin changes only the final original destination from index 1 to 0.

Historical source 6a6c6ffa173d missed the explicit current-zero runtime canary. Its unchanged bytes and original R10 refusal records are preserved under P8 (`build/scalar-storage-versions-20261029/historical-field-6a6c6ffa173d/snapshot.json`). Original-source R1 b140 success was owner-reported. This refreshed pair adds only the runtime canary and is no longer byte-identical to that commit; none of its contracts or mutation operations changed.

The owner will rerun strengthened-field R10 baseline and require preserve's same own Ensures refusal. That baseline rerun and candidate execution/rejection qualification for the refreshed sources remain pending. Both parents retain their remaining acceptance.
