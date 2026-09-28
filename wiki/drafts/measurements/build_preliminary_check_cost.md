# What the duplicate preliminary whole-program check costs

**BUILD-EVALUATES-ONCE** asks to "eliminate the duplicate preliminary
whole-program check by moving its obligations into the owning stages".
Nothing recorded what that duplication costs. This does.

**This is motivation, not acceptance.** That item says plainly that "timing
rows alone are insufficient": its acceptance is counting one Build activation
and one final product check for a two-target request, with std's six rows
from one evaluation. Nothing here discharges that.

## Measured 2026-09-28 at `5f2031a538e`, macOS arm64

`omega --check --timings` on `samples/cli/basics/cli_mvp`, a sample that sums
a few integers and exits — so essentially all of this is fixed cost, not
program cost. Five runs, each into a fresh build directory:

| run | preliminary | settled | total | preliminary share |
| --- | --- | --- | --- | --- |
| 1 | 16.6s | 79.1s | 114.2s | 15% |
| 2 | 15.1s | 43.6s | 72.7s | 21% |
| 3 | 15.3s | 43.4s | 72.8s | 21% |
| 4 | **15.1s** | **42.5s** | **71.6s** | **21%** |
| 5 | 16.3s | 83.3s | 127.4s | 13% |

**Take the minimum row.** The host carried a load average above 10 from other
agents throughout, so totals are inflated; the least-contended run is the
closest to the true cost.

The preliminary check is **15.1–16.6s, stable to within 10% across every run**,
while the total swings 71.6s to 127.4s. The variance lives in the settled
check, not the preliminary one, which is what makes the preliminary figure
worth quoting at all.

## The structure behind the number

From the same rows, for one root with one dependency:

```
review_pass discovery  preliminary_checks=2  preliminary_micros=15.1s
                       settled_checks=2      settled_micros=42.5s
                       frontier_reused=0
review_pass bound      preliminary_checks=2  preliminary_micros=0.026s
                       settled_checks=2      settled_micros=0.052s
                       frontier_reused=2
```

The whole-program check runs **twice in the discovery pass** — once
preliminary, once settled. The bound pass repeats both and costs about 50
milliseconds, because it reuses the frontier. So the duplication that costs
anything is entirely within discovery.

`omega_language_std` is where it lands: 66.6s of a 74.7s compilation in the
first run. The sample's own package cost 2.3s.

Meanwhile every pipeline stage the timing block itemizes is trivial:

```
  0.818 ms  Stage 01: SourceFiles -> TokenStreams
  0.688 ms  Stage 02: TokenStreams -> SyntaxTrees
  3.679 ms  Step: pre-build check (declaration admission)
 25.093 ms  Step: build: AssembledSyntax -> CheckedTrees
 37.188 ms  Stage 05: TypedTrees -> CheckedTrees
165.717 ms  Step: settle: CheckedTrees -> SelectedExecution
```

Those sum to roughly 233 ms against a 71.6s run. **Reading the itemized stage
list alone would put the cost in entirely the wrong place**; it is in
re-checking the dependency closure, which only the `omega_review_timings`
rows expose.

## Why it is worth the item's time

About **21% of every compilation on this host** is the preliminary check, and
it is paid per compilation, not once: every sample check, every corpus
fixture, every agent's edit-check loop. The 71s floor recorded in
[sample_gate_macos_arm64.md](sample_gate_macos_arm64.md) — a trivial sample
costing over a minute — is mostly these two whole-program checks of the
standard library.

Removing the duplicate would not make the floor small; the settled check is
the larger half at ~42.5s. It would take roughly a fifth off it.
