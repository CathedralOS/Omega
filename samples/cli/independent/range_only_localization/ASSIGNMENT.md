ADD09: independent range-only localization application.

Base: 9a0463cc42af5f7178372c169ae55e528fe43878, fetched origin/main.
Branch: add09-range. Worktree: .codex/worktrees/add09.
Scope: samples/cli/independent/range_only_localization exclusively.
No active-worker dependencies, shared numerics or worker candidates.
Acceptance: check, native and Terminal comparison, exact stdout for valid and
invalid runtime cases, separate complete intended-invalid source rejection.
Claim request ticket: 5d01a90e8f7e491ea38e60fc0a615485. Coordination timed out
and status also timed out; registration is unconfirmed, isolated work continues.

Source comparison on fetched main: `rg -l -i
'survey|traverse|nearest.neighbor|localization' samples` finds only incidental
probe prose (dual_accumulator_recursion, callback_contract_reach,
reach_through_machine_param). No admitted survey/nearest-neighbor application
is present in this checkout, and samples/cli/independent did not yet exist.
Existing maze_flood enumerates maze cells; bubble_sort reorders values;
parse_int parses a fixed text carrier; dual_float demonstrates scalar arithmetic;
float_builtins checks min/max/sqrt. None intersects bounded range constraints
on one static position or emits all feasible position/residual tuples.
The delivered workflow uses synthetic static geometry, without traverse state,
temporal estimation, nearest-neighbor selection, or averaging.

Additional source comparison: arithmetic/stats_compute computes mean, variance
and weighted sum of eight fixed scalar measurements; it does not estimate a
position from ranges. arithmetic/vector_distance evaluates one known 3-4-5
vector magnitude, without inverse geometry or feasible-set classification.
algorithms/maze_flood performs queue-backed BFS through walls; it does not test
Euclidean annuli. ADD09 supplies runtime observations, an exhaustive inverse
range-constraint search, full candidate residual rows and domain classifications.

Current frontier: admitted source check passes, native coverage declines the
fixed-array-to-slice loan on the root's guarded state edge (coordinate 755).
Application and oracles remain unchanged. Next: reproduce on fetched current
main 2f145503f2 using the complete minimal control and repair the general
loan/view presentation if still missing. Claims status remains unavailable.
