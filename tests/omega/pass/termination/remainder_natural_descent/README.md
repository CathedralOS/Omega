# Completed remainder rank arrivals

This bounded repair supports `CORE-INTEGER-FLOAT-APIS`: the scientific consumers
reach the existing GCD in `source/library/numerics/arrays/shape_layout_contract.omg`.
The fixture copies that machine body and its `terminates by right ->
Nat::Descending` clause unchanged. Its runtime oracle checks common factors,
coprime inputs, a zero right operand, and the maximum unsigned dividend.

The obligation is `0 <= left % right < right` on the independently established
`right > 0` edge. The destination's second parameter receives that exact
completed remainder. Its source rank is the entry value of `right`; current
scalar capture and call-frame checks retain the existing assignment and effect
rules. Every complete recurrent cycle must still contain strict descent.

The [reject twin](../../../fail/termination/remainder_natural_descent/main.omg)
only exchanges the recursive actuals: `(left % right, right)` preserves the rank
and cannot prove termination. The [composition witness](../remainder_natural_descent_forwarding/main.omg)
uses different machine names and a private nested helper. Its forwarding edge
preserves the rank and its local remainder supplies the descending edge.
The [bounded witness](../remainder_natural_descent_bounded/main.omg) retains the
same recursive operation under an explicit `0..=48` rank range, exercising the
root-cycle arrival owner as well as joint natural ranking.

The frozen optimized integer compiler for `ead713c6a6166c0ac65f3b722dd75475ea8d5e86`
(`integer-integrated-r17-omega`, SHA-256
`20e9458ce518484e213fa0ae5644b3339fd0039cf114a3f6deb2e6cc3de62ecb`, opt-level 1)
rejects both the original positive and its twin with:

```text
no exact private-coordinate assignment proves every edge and complete cycle
```

Before the repair, rank arrivals projected completed operands into plain scoped
specification terms. That discarded the existing completed-remainder evidence
which local scalar capture already retains. Rank arrivals now use the same
completed-value owner. The arithmetic engine still establishes nonnegative
dividend and positive divisor independently before adding the remainder's
strict bound; no GCD spelling, operator rule, formation exemption, or termination
waiver is introduced.

The sidecar's fixed source base is `fdbafbb29344`. Frozen inputs, raw baseline
diagnostics, producer profiles, candidate source hashes and execution records
are under the worktree's ignored `build/gcd-termination/`. Parent integration
owns the float API migration, full scientific consumer acceptance, review and
landing. This fixture establishes the GCD ranking repair, not closure of the
parent capability or a nine-place fact row.

On macOS arm64, the repaired optimized compiler runs all three positives with
native exit 0 and Terminal exit 0; the nondecreasing twin retains its original
rejection. The direct inline positive is the demonstrated baseline defect.
Fresh historical projects confirm that the forwarding and bounded variants
already accept on the frozen compiler; they establish composition coverage.
The affected crate's Clippy check succeeds with preexisting warnings.

Ten existing focused fixtures retain identical outcomes and diagnostics against
the frozen compiler. That set includes four preexisting failures, not four new
regressions: `runtime_joint_rank_computed_copy_exit` rejects, while
`joint_machine_call_cycle_stalled`, `rank_range_subject_intervening_write`, and
`signed_rank_without_nonnegative_premise` reject with diagnostics that differ
from their existing expected fragments. These unrelated outcomes remain open.
