# Trophic energy reconciliation

NEW060 is a standalone Omega console application over synthetic scientific
sample records. It reconciles observed energy transfers; it does not predict
population growth, determine a food-web equilibrium, or optimize feeding.

Three nodes have external supply and maintenance. Up to six directed feeding
records carry source, consumer, raw intake, and conversion efficiency in integer
percent. Every edge debits raw energy from the source and credits converted
energy to the consumer. Conversion loss is raw minus converted. Duplicate edges
are independent observations; cycles are simultaneous recorded transfers and
create no external energy. Self edges are rejected as outside this sample model.

For each node, residual = external + assimilated - maintenance - outgoing.
Deficit = max(-residual, 0), surplus = max(residual, 0). A web is feasible exactly
when every deficit is zero. The diagnostic identity is
external - maintenance - conversion loss - sum(residual) = 0.
A zero diagnostic can coexist with infeasibility: conservation alone is not an
adequate supply budget.

Input is ASCII decimal tokens separated by spaces or LF. Supply, maintenance,
and intake are integers 0..1000; efficiency is 0..100; node IDs are 0..2.
A token has at most four digits. The input contains six tokens for the three
supply/maintenance pairs, one edge count 0..6, and four tokens per edge.
Extra tokens, missing tokens, negative values, malformed bytes and excess bounds
exit 64. EOF is required after optional separators. Input has finite records;
waiting for an interactive host is the ordinary Console blocking boundary.

Output uses signed seven-digit integers. Energy values are exact hundredths;
node IDs are ordinary integers. Each flow header is followed by source,
destination, raw, assimilated, and loss. Each node header is followed by ID,
intake, assimilated, outgoing, residual, deficit, surplus. The final web header
is followed by loss, summed deficit, and balance error. Exit 0 means feasible,
3 means infeasible, 70 means a broken balance identity. Accepted edges are
streamed before final EOF validation; a subsequent invalid token may leave a
partial flow report, but never a completed feasible result.

All arithmetic declares Saturating. Its saturation is unreachable in admitted
inputs: total raw or converted energy is at most 600000 hundredths; external
supply and maintenance totals are each at most 300000; residual absolute values
are below 700000. Intermediate diagnostics are below 2000000. These are far
inside i32. Private domain machinery has no numerics-library imports.

`data/reference.py` independently computes rational results with Python Fraction.
The checked-in `oracle.json` and byte-exact stdout files were frozen before the
first Omega check/native/Terminal execution. Tolerance is exactly zero; oracle
regeneration is an authoring operation, never part of validation. Fixtures cover
branched diets, a three-node cycle, an infeasible cycle, fractional conversion,
zero input and all six records at upper intake bounds, plus eleven invalid
runtime inputs. The separate `controls/invalid_bounds` project intentionally
indexes a three-element donor table at source 3; only a bounds rejection is valid.

Compared with `samples/cli/systems/bank_ledger`, this workflow has directed
inter-node transfers, conversion loss and independent per-node budgets rather
than one transaction sum. Compared with `collections/matrix_multiply`, it
reconciles sparse observations and rejects malformed runtime data rather than
multiplying constant dense matrices. No semantic food-web overlap was found
in the existing samples.

From the repository root, with a compiler built from the recorded base:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
omega update --project samples/cli/independent/trophic_energy_reconciliation --target linux_x86_64
# Inspect build/package-manager/review-linux_x86_64.txt if review is pending.
# Accept only reviewed decisions, then use update --project <directory> --resume.
python3 samples/cli/independent/trophic_energy_reconciliation/validate.py --omega /absolute/path/to/omega --route check
python3 samples/cli/independent/trophic_energy_reconciliation/validate.py --omega /absolute/path/to/omega --route native
python3 samples/cli/independent/trophic_energy_reconciliation/validate.py --omega /absolute/path/to/omega --route both
python3 samples/cli/independent/trophic_energy_reconciliation/validate.py --omega /absolute/path/to/omega --route invalid-control
```

Validation is serial. `run --both` exercises the produced Terminal artifact and
requires agreement; the documented CLI compares termination only and exports
native stdout. Thus byte-exact output is proved for native execution, while
Terminal evidence covers termination and successful interpretation, not a
separately exported Terminal stdout comparison. `build/validation/` retains
stdout, stderr, and results for each invocation. Package locks and generated
products remain local, never dependencies of this source candidate.
