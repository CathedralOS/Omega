# Pareto experiment selection (IND11)

A complete stdin-to-report Omega application with private bounded storage, integer
input admission, constraint filtering, strict Pareto dominance, stable identities,
and certificate generation. It has no dependencies on active workers or a
numerics library. The only package dependency is the existing standard console.

The research cards in `data/catalog.csv` are explicitly synthetic, not empirical
measurements or recommendations. Cost uses research credits, duration uses days,
gain uses information points and risk uses an index. The catalog connects stable
row and source IDs to experiment names and immutable synthesis-card provenance.
Arbitrary new IDs and source IDs can be supplied without changing Omega source.

Input starts with `count budget deadline risk_limit`, followed by `count` rows
of `id source cost days gain risk`. Whitespace separates tokens. Decimal tokens
contain one to three digits, with values 0–999; counts are 0–12, identifiers and
source identifiers are positive and identifiers are unique. Budget, deadline and
row duration are positive. Risk and its limit are 0–100. Zero cost and gain are
valid. EOF must follow the declared rows except for whitespace. Invalid input
exits 2 and emits `ERROR|invalid-input` without a partial report. Inputs are finite
streams; console waiting and failure retain the ordinary provider behavior.

An experiment is feasible when `cost <= budget`, `days <= deadline` and
`risk <= risk_limit`. Selection minimizes cost and duration and maximizes gain.
A feasible row dominates another if it is no worse in all three objectives and
strictly better in at least one. Risk is a constraint, not a fourth objective.
Equal vectors remain separate alternatives, including different sources or risk
values. This chooses a set of individually feasible experiments; it does not
solve a combined portfolio budget or invent a scalar weighting.

The report emits every row in increasing stable identifier order. It preserves
its source identifier, original one-based input position, and all four measures.
`SELECTED|000` certifies exhaustive absence of a feasible strict dominator.
`DOMINATED|id` names the smallest feasible dominating ID, which need not itself
belong to the front. `INFEASIBLE|mask` identifies every violated constraint:
cost=1, duration=2, risk=4, combined by addition. The loops examine all admitted
rows; ties, constraint failures and row ordering do not shortcut this definition.
Storage is twelve fixed records; selection costs O(n²). Canonical report ordering
also costs O(n²) and never changes provenance. Exact integer arithmetic and exact
stdout bytes require **zero numerical tolerance**.

`domain/model.omg` owns data, `input/numbers.omg` owns byte parsing,
`domain/selection.omg` owns feasibility and dominance, `report/render.omg` owns
canonical rendering, and `main.omg` owns admission and visible dispatch.

## Independent expectations

`expected/FROZEN.json` records input and expected-output SHA-256 values frozen
before the first Omega application execution. `reference.py` checks those hashes
and enumerates fronts independently by mapping `(cost, days, gain)` to the
all-minimized vector `(cost, days, -gain)`. It never authors or runs Omega source.
The baseline front is **102,103,110,111**; reversing rows preserves this front
while changing the retained input-position provenance. IDs 110 and 111 tie.
The tighter front is **101,105**; with all constraints relaxed it is
**102,103,106,107,108,110,111**. The zero-risk variant has an empty front.
Separate fixtures cover no rows, one row, equality boundaries, selected ties,
zero cost/gain, maximum 999 values, CRLF/tabs and EOF without a final newline.
Runtime-invalid fixtures challenge count, missing fields, overflow, digit count,
lexical form, negative values, zero identities/provenance/duration, duplicate IDs,
risk range and trailing garbage.

`invalid_source/` is a separate complete intended-invalid source project. It
attempts exact unsigned `0 - 1`; validation requires its proof rejection,
independently of all runtime-invalid tests.

## Commands

From the repository root, use a compiler built from that checkout and select
its library explicitly. Package review is the ordinary documented update route:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
omega update --project samples/cli/independent/pareto_experiment_selection --target linux_x86_64
# Read the pending review, accept the reviewed decisions, then:
omega update --resume --project samples/cli/independent/pareto_experiment_selection
omega --check samples/cli/independent/pareto_experiment_selection/main.omg
omega run --keep samples/cli/independent/pareto_experiment_selection/main.omg < samples/cli/independent/pareto_experiment_selection/data/baseline.stdin
omega run --both --keep samples/cli/independent/pareto_experiment_selection/main.omg < samples/cli/independent/pareto_experiment_selection/data/reordered.stdin
python3 samples/cli/independent/pareto_experiment_selection/reference.py
python3 samples/cli/independent/pareto_experiment_selection/validate.py --omega /absolute/path/to/omega
omega --check samples/cli/independent/pareto_experiment_selection/invalid_source/main.omg
```

The final command must reject the exact unsigned-underflow control. Generated
compiler artifacts, logs and accepted lock/review files stay in ignored local
`build/` or local package state. Build binds the ordinary Linux, macOS and Windows
host entry slots; the validation evidence records the actually executed target.
`run --both` compares host termination against Terminal execution. The private
`validation/` harness additionally checks Terminal stdout and stderr directly,
through the current main compiler's public package and runtime facades. It prepares
the normal package project, compiles once, retains the produced Terminal artifact
and its admitted provisions, and sequentially replays all 28 input fixtures.
It supplies only finite input and private containing-domain exit delegation;
there is no source interpreter, guessed provider or generated expected guest code.
Build and run it from the repository root:

```sh
mbx build --release --manifest-path samples/cli/independent/pareto_experiment_selection/validation/Cargo.toml --target-dir samples/cli/independent/pareto_experiment_selection/build/rust-validation
samples/cli/independent/pareto_experiment_selection/build/rust-validation/release/ind11-terminal-validation --evidence "$PWD/samples/cli/independent/pareto_experiment_selection"
```

Use `cargo` if `mbx` is unavailable. Run one Rust build and one Omega execution
at a time. The harness's explicit target is `linux_x86_64` and its sponsor fuel
is 10,000,000 per input. Exhaustion, declined imports, crashes or partial output
fail validation. Native output is separately checked through `validate.py --native`
using the executable produced by the ordinary Omega compile command.

Existing `insertion_sort`/`bubble_sort` only sort fixed scalar arrays;
`binary_search_viz` searches an ordered scalar collection; `bank_ledger` accumulates
transactions; `maze_flood` computes reachability. Those workflows provide no
constraint-aware multi-objective front, strict tie handling, per-rejection witness,
or row provenance. A source search for Pareto/dominance/experiment/multi-objective
found no existing substantive overlap before authoring.
