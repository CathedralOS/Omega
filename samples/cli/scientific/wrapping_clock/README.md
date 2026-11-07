# Wrapping acquisition clock (IND17)

A complete stdin-to-report application that reconstructs a bounded acquisition
counter and fits its clock to host observations. All implementation is private to
this directory; the only package dependency is the stable console and core
conversion surface. No active-worker code or numerics package is imported.

Existing `elapsed_timer` measures a live host sleep, `random_walk` uses pacing,
and checksum samples exercise modular arithmetic without reconstructing epochs.
This workflow instead decides uniqueness from an external interval, validates
arrival order, retains dropped-reading evidence, and fits reconstructed times.

## Input and assumptions

Whitespace-separated nonnegative decimal integers (ASCII space, tab, LF or CR):

```
count modulus lower upper absolute_anchor
sequence raw_counter host_time
... exactly count rows ...
```

`count` is 2..16; modulus 2..65536; 1 <= lower <= upper <=100000.
Sequence and host values are <=100000. Raw counters are <modulus. Numbers have
at most 19 digits and are <=9223372036854775807. EOF terminates the input.
The acquisition span must be <=100000 ticks. Host time strictly increases;
sequence identifiers strictly increase, and skipped identifiers denote drops.
Duplicate or reordered readings reject rather than sorting or silently dropping.

Unique absolute reconstruction requires all of these external assumptions:

* The counter increases one tick at a time modulo the declared modulus, without
  resets, rate reversals or a changed modulus during the acquisition.
* The first absolute timestamp is known and congruent to the first raw reading.
  Without this anchor, only relative times can be reconstructed.
* Rows arrive in true acquisition order. Sequence identifiers reliably identify
  that order and do not wrap within this data set. Host observations describe
  each acquisition instant; host ordering alone does not establish tick bounds.
* The inclusive lower/upper interval is independently valid for **each pair of
  retained adjacent readings**, including across dropped readings. The fit is
  never used to manufacture those bounds or resolve an ambiguity.
* Exactly one positive delta congruent to each raw difference lies in that
  interval. A narrow interval can identify several complete wraps; a broad one
  can remain ambiguous even when the residue increases. Equal residues do not
  imply zero elapsed time. This workflow deliberately excludes simultaneous
  acquisitions with zero elapsed ticks.
* Absolute timestamps fit in signed 64-bit storage. A valid final timestamp at
  the maximum is accepted; exceeding it rejects before addition.

A false external assumption can yield a unique but incorrect answer. The program
validates the declared model, not the provenance of acquisition equipment.

## Model and report

Output includes every sequence/raw/absolute timestamp/relative elapsed/host row,
then wrap crossings and dropped sequence count. The host clock is fit with an
anchor-constrained least-squares line: host[0] + elapsed * N/D, where
N=sum(elapsed*host_delta) and D=sum(elapsed^2). D>0 follows from positive deltas
and at least two rows. The rate is host units per device tick; nominal rate is
one host unit per tick. Drift is (N-D)*1000000/D ppm. Maximum absolute fitting
residual is reported as a numerator over D. Ratios are intentionally unreduced.
The model fixes the first host observation as an exact intercept; it does not
estimate an independent intercept or imply that a noisy first observation is
perfect. This assumption is part of the chosen diagnostic fit.

Validated magnitudes bound sums by 1.6e11, drift numerators by 1.6e17 and residual
products by 3.2e16. Private arithmetic uses explicit Wrapping carriers while
runtime guards ensure every accepted mathematical result fits. Absolute
addition uses MAX-elapsed before addition; decimal parsing checks the cutoff
before multiplying. The intended-invalid control separately demonstrates that
unbounded Exact signed addition must not be accepted.

Exit 0 reports a valid acquisition. Exit 2 emits `ERROR code`:

| Code | Meaning |
| --- | --- |
| 1 | decimal syntax, integer overflow, token capacity or row count |
| 2 | header outside supported bounds |
| 3 | raw, sequence or host outside bounds |
| 4 | anchor inconsistent with first residue |
| 5 | duplicate or reordered sequence |
| 6 | non-increasing host time |
| 7 | no admissible elapsed time |
| 8 | more than one admissible elapsed time |
| 9 | acquisition span exceeds bound |
| 10 | reconstructed absolute timestamp would overflow |

No timestamps are emitted until validation and reconstruction of the whole input
succeed. On invalid input, the first error in the documented validation order is
reported: syntax, header/length, all row ranges, anchor, then adjacent edges.

## Frozen acceptance and execution

`data/*.stdin`, `*.stdout`, `*.exit` and `oracles.sha256.json` were frozen before
any Omega execution. Numerical tolerance is **zero**, including jitter: byte-for-
byte output and exact exit matching. `reference.py` uses arbitrary integers and
exhaustive delta enumeration, independently of the implementation's ceil-based
candidate selection. Validation cannot overwrite the frozen expectations.
`wrap_drift`: timestamps 250,300,400,450,500; one wrap, one drop; rate 2 and
1000000 ppm. `exact_wrap`: 7,263,519, exactly two wraps. `multiwrap_unique`:
250,850,1450, five crossings and two drops. `jitter`: rate 1450/1400,
maximum residual 2400/1400. Signed boundary cases end exactly at i64::MAX.

From repository root, with a compiler built from this base and its matching
library selected. Prepare the host-local acceptance first with `omega update`,
inspect its generated review, edit the exact pending decisions to `accept`, then
run `omega update --resume`. Repository policy keeps `omega.lock` host-local:

```
OMEGA_LIBRARY_ROOT="$PWD/source/library" "$OMEGA_BIN" update --project samples/cli/independent/wrapping_clock --target linux_x86_64
# Inspect build/package-manager/review-linux_x86_64.txt and accept the reviewed rows.
OMEGA_LIBRARY_ROOT="$PWD/source/library" "$OMEGA_BIN" update --resume --project samples/cli/independent/wrapping_clock
OMEGA_LIBRARY_ROOT="$PWD/source/library" "$OMEGA_BIN" --check samples/cli/independent/wrapping_clock/main.omg
OMEGA_LIBRARY_ROOT="$PWD/source/library" "$OMEGA_BIN" run --both --keep samples/cli/independent/wrapping_clock/main.omg < samples/cli/independent/wrapping_clock/data/wrap_drift.stdin
python3 samples/cli/independent/wrapping_clock/validate.py --omega "$OMEGA_BIN" --compiler-source-commit "$OMEGA_COMMIT"
```

Validation serializes check, each native/Terminal comparison, and the complete
intended-invalid control `invalid/unchecked_timestamp.omg`. Its entry machine is
bound by its own build metadata; its source filename prevents sample discovery
from treating an intentionally rejected control as a runnable sample. Generated output and execution logs
stay in ignored `build/`. `build/validation.json` records actual commands,
compiler/base/source hashes, resource facts, exits and oracle comparison.
The harness requires Terminal agreement explicitly, not merely a native exit.
EOF replay by `run --both` gives both engines identical input. The entrypoint is
bound explicitly for supported hosted targets; execution here validates Linux
x86_64 only. Other targets are declared, not claimed as tested.
