# Bounded crossover experiment design (ADD17)

A complete console application constructs treatment-by-period allocations for
one finite design request. Each subject receives all two or three treatments
exactly once. It admits exact balance in every period, a maximum count for every
directed adjacent treatment pair, and an optional prohibited carryover edge.
It prints the selected allocation plus period and carryover count certificates,
or exhausts the bounded design space and reports infeasibility.

Supply one LF-terminated `T,N,C,F` line, for example `3,6,2,0`. See
[data/oracle-contract.md](data/oracle-contract.md) for bounds and edge codes.
Exit 0 means certified allocation, 2 infeasible, 64 invalid input, 70 defect.
No pause or interactive menu is required. CRLF and EOF without LF are invalid.
This is a feasibility designer; no claim of randomization or clinical inference
is made. Subjects have no individual covariates or restrictions.

`main.omg` owns input and dispatch; `design/model.omg` owns private records and
permutations; `design/search.omg` exhaustively enumerates sorted sequence
multisets; `design/certificate.omg` recounts actual cells and admits constraints;
`design/output.omg` emits the allocation and both matrices. All storage and
algorithms are private. The sole dependency is the existing bundled standard
console/provider package. No numerics library or worker candidate is used.

Search completeness depends on exchangeable subjects: every labelled design can
be sorted without changing these count constraints. The six-subject,
three-treatment limit has 462 multiset candidates. Every generated candidate is
materialized and recounted. Successful results are recounted again before output.
The bounded counters use Wrapping i32 and cannot approach overflow under these
limits; all array accesses have explicit runtime range guards.

## Overlap review

On base `eac95f5bc8ef4123fd7815bafe188df2a6f71a07`, existing
`cli/algorithms/dutch_flag` partitions three colored items in place and checks
ordering; `cli/simulation/alarm_scheduler` processes scripted alarm events;
`cli/simulation/random_walk` evolves a position. None constructs balanced
experimental sequences or constrains directed carryover. The assigned admitted
resource scheduler, data partitioner, and first-wave randomization-test examples
are absent from this fetched main. Their stated goals concern makespan,
train/test membership, and inference from observations respectively. This sample
constructs an experimental design and its count certificates. No existing sample
is imported or repackaged.

## Independent frozen reference

`reference.py` enumerates labelled Cartesian products and independently counts
column frequencies and directed adjacent pairs. Its separate tiny raw-cell
crosscheck enumerates repeated-treatment rows too, confirming the permutation
restriction for 12 parameter sets. `data/expected.json` and 19 stdin fixtures
were frozen before any Omega execution; all output bytes and exits are exact
(integer tolerance zero). Its SHA-256 is
`6ce9539d7a9b9479641ee8a26ee414e15e650553fbcc512adea33bb01fda6ab5`.

The fixtures cover two-treatment replication, three-treatment Latin allocations,
pair-balanced six-subject design, a forbidden-edge variant, insufficient
carryover caps, impossible period balance, zero carryover cap, malformed bytes,
delimiters, unsupported values, oversized input and EOF. `validate.py` compares
actual native outputs and exits, then reconstructs count certificates from the
emitted allocations. It never regenerates the golden during validation.

## Commands from repository root

Use one compiler or Omega execution at a time. Set `OMEGA_LIBRARY_ROOT` to this
checkout's `source/library`. Substitute a current-main verified binary for
`$OMEGA`; retain its path, hash and source revision in validation evidence.

```sh
python3 samples/cli/independent/crossover_experiment_design/reference.py
$OMEGA update --project samples/cli/independent/crossover_experiment_design --target linux_x86_64
# Inspect the generated review and accept only the exact bundled std proposal;
# then use the normal saved-review --resume route.
$OMEGA update --resume --project samples/cli/independent/crossover_experiment_design
$OMEGA --check --target linux_x86_64 --build-dir samples/cli/independent/crossover_experiment_design/build/check samples/cli/independent/crossover_experiment_design/main.omg
$OMEGA --target linux_x86_64 --build-dir samples/cli/independent/crossover_experiment_design/build/native samples/cli/independent/crossover_experiment_design/main.omg
python3 samples/cli/independent/crossover_experiment_design/validate.py <emitted-executable> --omega "$OMEGA" --terminal
$OMEGA --check --target linux_x86_64 --build-dir samples/cli/independent/crossover_experiment_design/build/invalid samples/cli/independent/crossover_experiment_design/invalid/main.omg
```

The last command must reject the unchecked array-index obligation; package or
provider rejection does not count. `invalid/` is a separate complete intended-
invalid project and is deliberately excluded from positive sample acceptance.
Terminal comparison in the CLI compares termination only; exact stdout is
independently checked against the native output, and `DECLINED` is not agreement.
Retained output/logs belong in ignored `build/`; source/data never depend on them.
