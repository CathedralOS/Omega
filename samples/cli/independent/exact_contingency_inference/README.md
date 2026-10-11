# Exact contingency inference

A complete console application for synthetic 2×3 categorical count tables.
Two anonymous groups are classified into three anonymous outcomes. The declared
scientific model conditions on positive row and column margins and any forbidden
cells. On the remaining support, table mass is proportional to `1 / ∏ cell!`.
This is a conditional factorial model, with no asymptotic approximation or
claims about real populations. Structural zeros remove tables and renormalize.

The application enumerates feasible tables, computes an ordinary Pearson
association statistic relative to the unrestricted independence expectations
`row × column / N`, and emits two inclusive exact tails: mass no greater than
the observed table's mass, and association statistic no less than observed.
The Pearson reference is a ranking statistic even with structural zeros; it is
not fitted to the conditioned support and no chi-square calibration is used.
`association_ties` counts tables with exactly equal Pearson statistic.

Input is one record of exactly 17 ASCII digits, optionally terminated by LF:
`rrcccoooooozzzzzz`. The first two digits are row margins, the next three column
margins, then six observed cells in row-major order, then six zero flags.
Flags are 0 (allowed) or 1 (forbidden). All margins must be positive, match in
total and sum to at most eight. This bound makes complete enumeration and exact
integer output practical. The first record is processed; subsequent input is
outside this one-record interface. CRLF, spaces, short lines, non-digits, excess
record bytes, EOF without a complete record, and oversized totals are rejected.

Outputs contain exact numerators and denominators, with **zero tolerance**.
Weights are `N! / ∏ cell!`, integer multinomial counts. `partition` is their sum;
both tail numerators are divided by `tail_denominator`. The Pearson value is
`pearson_numerator / pearson_denominator`. Ratios are deliberately unreduced.
There is no imported numerical library. The private table, statistics, inference,
and I/O modules own the complete workflow.

The enumerator tries 81 first-row coordinate pairs and derives the other four
cells, then checks every margin and zero flag. The independent Python oracle
instead traverses all six-cell Cartesian products and calculates statistics and
masses with rational arithmetic. The frozen feasible-table lists independently
pin support, including impossible support. `data/oracle.json` fixes stdout,
status, hashes and tolerance before the first compiler execution.

Arithmetic uses declared `u64 in Wrapping` with external boundedness: a valid
cell is ≤8, every factorial product is ≤8!, each table weight is ≤40320, and the
partition is ≤81×40320. The product of all margins is ≤288; each squared residual
is ≤4096, so the score is ≤6×4096×288. Modular squaring recovers the ordinary
signed residual square. Runtime denominator guards precede division and an
invariant check rejects zero computed weight/partition/score denominator.

Exit statuses: 0 successful inference; 64 malformed record; 65 invalid margins
or zero flags; 66 no feasible table; 67 observed table violates margins or zeros;
70 arithmetic invariant failure. Impossible support takes precedence over an
invalid observation. Invalid observations never enter numerical calculations.

From the repository root, using the compiler built from that checkout and
`OMEGA_LIBRARY_ROOT` set to its `source/library`:

```sh
omega update --project samples/cli/independent/exact_contingency_inference
# Inspect the generated review; accept only the required documented std decisions.
omega update --resume --project samples/cli/independent/exact_contingency_inference
omega --check samples/cli/independent/exact_contingency_inference/main.omg
omega run --keep samples/cli/independent/exact_contingency_inference/main.omg < samples/cli/independent/exact_contingency_inference/data/sparse.stdin
omega run --both --keep samples/cli/independent/exact_contingency_inference/main.omg < samples/cli/independent/exact_contingency_inference/data/sparse.stdin
python3 samples/cli/independent/exact_contingency_inference/reference.py
```

`controls/invalid_contract/main.omg` is a separate complete source whose returned
record contradicts its `ensures result.value == 2` postcondition. Check it
separately; rejection must name that postcondition and the ensures rule.
Syntax, loading, trust, or provider rejection is a failed control.

Existing semantic overlap was reviewed before authoring: `arithmetic/dot_product`
and `collections/array_sum` only reduce arrays; `text/parse_number` and
`text/format_number` only parse/format; `interpreters/calculator_rpn` manages a
stack; `systems/framed_payload` computes a checksum. None enumerates a margin
fiber, conditions on structural zeros, computes factorial masses or ranks exact
tails. No contingency/Fisher/categorical/structural-zero sample was found.

The only external package is the stable bundled std console provider. Build
metadata binds the Linux x86-64 program-entry slot explicitly. Other targets
have not been claimed as validated. Generated products are under ignored
`build/`; evidence and final validation state are in `evidence/VALIDATION.md`.

The reserved `!` plus LF input runs `controls/runtime_oracle.omg`: seven valid
models exercise the actual analyzer and compare every numerical result against
the frozen independent oracle. It exits 0 and prints
`PASS frozen_numerical_control` only after every comparison succeeds. This
provides numerical execution checks under Terminal, whose `--both` CLI normally
compares exit statuses alone. The stdin-driven inference remains the delivered
application; this control is supplementary validation.
