# Record and owned-array predicate handback

A saved observation of an unwritten array element stays equal to 7 across a write to a disjoint element.

Parent: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, current record-field and owned-array predicate handback. This is another production of the existing saved-observation fact, whose original parent remains **STATE-LOCAL-VALUE-FRONTIER**.

The holder starts with `[7, 1]`. `preserve` copies its array into independent owned storage, writes 4 to element zero of that copy, and writes the copied 4 to element one of the original. Its checked `kept` predicate must still hold on the current original record. `agree(saved, current)` consumes the unchanged 7 fact; field neighbors remain 11 and 13. The twin changes only the final destination from element one to element zero and owes the same preservation postcondition.

On fixed main `5134dc49b62ecfc216a0458094b6c8f6d6dd473a`, optimized R10 producer `ff7376f75f466e8b605149a58ede7e793bac852b` refuses the positive at `preserve`'s own postcondition. The twin refuses that same obligation. Frozen sources, executable/profile identity and original diagnostics are retained in `build/scalar-storage-versions-20261029/`. Neither is accepted repair evidence yet.

The repair must distinguish immutable captured contents, current storage homes and ordinary copy/reference binding. It must retain both existing stale-record and owned-copy rejection controls, all normal return paths, exact declaration paths and source access/custody admission. The parent and this production remain unfinished until all nine places run with native/Terminal agreement and their own reject twins are pinned.
