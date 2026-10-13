# arith_ensures_selection

Ensures pinned to the selected arithmetic meaning: exact/wrapped/saturated u8 sums + chained wrap-subtract-shift `((0-2)>>1)=127` — each `as` recast picks its carrier's policy.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
