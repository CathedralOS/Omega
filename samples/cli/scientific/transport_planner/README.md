# Independent transportation planner (IND01)

This complete Omega console application minimizes integer shipping cost for
three sources and three destinations. It owns its parser, storage, exact solver
and formatting. It depends only on the stable bundled std Console provider.

Supply is shipped exactly and every demand is met exactly. The input is one
line containing fifteen whitespace-separated decimal integers: three supplies,
three demands, then nine costs in row-major order. Margins are 0..9 and costs
are -99..99. Signs are accepted on negative values; plus signs are not accepted.
Input must complete below the 256-byte buffer capacity; EOF also completes a
line. Spaces, tabs, CR and LF are separators. This program processes one line
per invocation. Its output is the 3x3 shipment table and a signed four-digit
total cost. Exit 0 means optimal, 2 invalid input, 3 infeasible unequal totals,
and 4 an internal completeness failure.

The complete 3x3 bipartite graph has no forbidden arcs. For this model unequal
totals are the only infeasibility condition. No dummy destination or silent
supply disposal changes the user's problem.

Four cells determine all nine shipments. `planner/solver.omg` enumerates all
10,000 assignments to those cells, derives the others from conservation,
rejects negative derived cells and retains the least objective. The validated
input bounds put every feasible shipment in 0..9 and every objective in
-2673..2673, so declared Wrapping arithmetic never wraps. Ascending enumeration
and strict improvement give the lexicographically smallest optimal table.
No float tolerance is needed: tolerance is exactly zero.

Compared with `algorithms/maze_flood` (BFS distances), `bubble_sort` (ordering)
and `text/parse_number` (a literal two-digit conversion), this workflow reads
runtime signed data, optimizes a constrained allocation, produces the full
conserved plan and diagnoses infeasibility. It uses no active worker candidate,
shared numerical project, external optimizer, or generated semantic witness.

From the repository root, with a compiler built from this exact main base:

```sh
omega update --project samples/cli/independent/transport_planner --target linux_x86_64
# Read and accept the actual pending review, then omega update --resume.
omega --check --target linux_x86_64 samples/cli/independent/transport_planner/main.omg
omega run --both --keep samples/cli/independent/transport_planner/main.omg < samples/cli/independent/transport_planner/data/nontrivial.txt
omega --target linux_x86_64 samples/cli/independent/transport_planner/main.omg
python3 samples/cli/independent/transport_planner/validation/validate.py <emitted-executable> --report <report-path>
omega --check --target linux_x86_64 samples/cli/independent/transport_planner/intended_invalid/rejected.omg
```

The last command must reject the complete separate program's `costs[9]`
access to a nine-element array. It must not be substituted for invalid runtime
input checks, which execute the valid application.

`validation/oracles.json` was frozen before the first compiler invocation:
SHA-256 `2db28f76680e508ddbc9f1b31aba098671f9374708ba55c728b6ee0dd47b5f33`.
`validation/reference.py` recursively assigns each of the nine cells and is
independent of the Omega solver's four-cell parameterization. The manifest
contains eight named variants and all 1,539 balanced tiny 2x2 instances with
margins 0..2 and all four costs in {-1,0,2}, embedded in 3x3. Ten malformed
runtime fixtures cover EOF, missing/extra tokens, negative/large margins,
large costs, a bare sign, nonnumeric text, overflow-sized text and buffer fill.
The harness also checks EOF finalization and CRLF/tab separators. It checks
exact output/exit/stderr, conservation, nonnegativity, objective reconstruction
and the independently frozen optimal cost and tie-broken table. `freeze.py`
reproduces the manifest; do not rerun it to accommodate observed Omega output.

Named instances include a greedy trap, negative costs, ties, all-zero margins,
both directions of imbalance and the maximum magnitude objective. The
nontrivial fixture has supplies [4,3,5], demands [3,4,5], costs
[[8,6,10],[9,12,13],[14,9,16]], and minimum cost 119 with table
[[0,0,4],[3,0,0],[0,4,1]].

Generated compiler products and run reports belong under ignored `build/`.
Terminal stdout is checked independently by the validation-only Rust consumer
in `validation/terminal_probe`. It prepares the accepted project, produces one
native report, and executes that report's exact retained Terminal artifact with
private replay stdin and buffered stdout/stderr. It uses the stable compiler
runtime API and never calculates shipments on the host. `omega run --both`
checks exit agreement; this additional consumer checks exact Terminal output.
The portable Cargo templates materialize a private validation workspace under
ignored `build/`; it does not join the product's Rust workspace. Build and
execute it sequentially:

```sh
python3 samples/cli/independent/transport_planner/validation/build_terminal_probe.py
samples/cli/independent/transport_planner/build/terminal-probe-cargo/target/release/ind01-terminal-probe samples/cli/independent/transport_planner
```

See the delivered validation receipt for the exact compiler/source identities,
commands, native/Terminal observations and remaining limitations.

On this executor, `build_terminal_probe.py <verified-main-target>` also links the
same validation source to the recorded main build's existing Rust libraries.
Its exact rustc command and library hashes are saved in `build/terminal-probe-link.json`.
The validation consumer is checked separately; normal landing checks cover the
changed production compiler packages and CLI.
