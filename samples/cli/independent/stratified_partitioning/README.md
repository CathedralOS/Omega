# Stratified grouped train/test allocation (IND24)

A complete console application partitions a small research dataset while keeping
all records from each group in one split. All implementation is private to this
sample. It uses the existing standard Console provider and no numeric package.

Input is whitespace-separated decimal integers on stdin, terminated by EOF:

```
numerator denominator record_count
record_id group_id stratum
... exactly record_count triples ...
```

The test fraction is `numerator/denominator`, with `1 <= numerator < denominator
<= 9`. Supply 2–16 records with unique IDs 1–99, group IDs 1–8 and strata 1–3.
Groups may span strata. Empty strata are legal. The parser rejects non-digits,
negative signs, tokens above 999, more than 51 tokens, or input of 256 bytes or
more (including whitespace). Spaces, tabs, CR and LF are accepted. EOF is
required; this batch tool does not show an interactive pause.

Every nonempty proper subset of the present groups is eligible. For stratum s,
let N_s be its total records and T_s its test records. Minimize the lexicographic
triple:

1. `sum_s abs(denominator*T_s - numerator*N_s)`
2. `abs(denominator*test_count - numerator*record_count)`
3. the group mask `sum_{test groups g} 2^(g-1)`

This exact deterministic rule needs no random seed. The Omega implementation
visits all 255 nonzero eight-bit masks in increasing order and retains strict
improvements in the first two criteria. A mask with unused group bits cannot
beat its smaller equivalent. Results and assignments therefore remain unchanged
under record reordering. The bounded search is at most 255*16 row visits.
Saturating arithmetic has defined semantics; admitted inputs keep counts,
weights and objective values far below i32 limits, so no saturation occurs.

The report emits three-digit decimal values, sorted record IDs, group IDs,
train/test assignments, per-stratum totals and test counts, scaled absolute
errors, objective values, exact-balance feasibility, and measured pairwise
leakage. Divide scaled errors by the denominator to obtain record-count errors.
A nonzero optimal score proves exact proportional balance impossible under the
indivisible-group constraint. A singleton stratum can naturally make it
impossible. Empty train/test sets are excluded. A dataset with only one present
group exits 65. Malformed input exits 64. Successful allocation exits 0; an
internal conservation/leakage failure exits 66.

The audit materializes each assignment, recounts all strata and the test total,
and compares every same-group pair for differing assignments. Reporting consumes
these audited assignments. Validation compares the entire report, not a banner.

## Independent oracles

`reference.py` enumerates combinations of *present* group IDs, separately from
the Omega mask search. `data/oracles.json` freezes 70 cases before the first Omega
execution, including exact input/output SHA-256 values and integer tolerance 0.
The fixture files contain all expected outputs; validation never regenerates them.
The tiny cases exhaust every two-record layout over groups 1–2 and strata 1–3,
plus every three-record binary-stratum layout with distinct groups. Other cases
exercise mixed groups, missing group numbers, 16 records, 4/9 and 2/5 fractions,
reversed order, identical repeats, CR/tab whitespace, EOF without a final newline,
singleton strata, impossible exact balance,
empty strata, one group, and 15 malformed runtime inputs.

`data/supplemental-oracles.json` adds one high-bit variant, independently frozen
before its own first execution. Fifteen records in group 1 and one in group 8,
all in stratum 1 at fraction 1/9, force mask 128 and scaled error 7. This checks
the selected highest group bit and three-digit output. `supplemental-freeze.md`
records the hand calculation and hash. Validation runs all 71 cases while
preserving the original 70-case freeze.

`invalid/` is a separate complete intended-invalid application. Its seventeenth
array-slot write must reject at checking, independent of the runtime-invalid
inputs to the valid application. It uses `control.omg` so positive sample discovery
does not mistake the intended rejection for an ordinary executable sample.

## Existing sample comparison

`algorithms/dutch_flag` partitions three enum colors in place. It has no dataset,
strata, group integrity, proportional objective or global optimization.
`collections/inventory_system` searches and totals a fixed item table; it does
not allocate subsets or compare balance. `collections/run_length_encode`
provides byte-output usage but encodes adjacent runs, not train/test groups.
This sample adds grouped proportional allocation and an exact optimality report.

## Commands

From the repository root, use a compiler built from that revision. Explicitly
select the matching `source/library` when reusing a binary:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
OMEGA=/absolute/path/to/omega
SAMPLE=samples/cli/independent/stratified_partitioning
"$OMEGA" update --project "$SAMPLE" --target linux_x86_64
# Read and accept the generated review, then resume if requested.
"$OMEGA" update --resume --project "$SAMPLE"
"$OMEGA" --check --target linux_x86_64 "$SAMPLE/main.omg"
"$OMEGA" --target linux_x86_64 "$SAMPLE/main.omg"
"$OMEGA" run --both --keep "$SAMPLE/main.omg" < "$SAMPLE/data/balanced.stdin"
python3 "$SAMPLE/validate.py" --executable /absolute/path/to/published/executable
"$OMEGA" --check --target linux_x86_64 "$SAMPLE/invalid/control.omg"
```

`run --both` uses the current host and exercises native and Terminal interpretation where supported and
must report any declined comparison accurately. The current CLI compares exit statuses;
`validate.py` separately compares complete native output bytes. `run --target`
produces target artifacts without executing them, so omit that flag for host runs. Host-specific Windows, Linux
ARM64 and macOS ARM64 entry bindings are included, but only a tested host can
claim runtime acceptance. Generated products and validation records belong in
`build/`; they are not source dependencies. The handoff records actual binary,
revision, commands, source hashes and observed outcomes.
