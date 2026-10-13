# Ecological rarefaction

This independent application calculates species richness when drawing labeled
individuals uniformly without replacement. It emits every richness bin from
0 through the number of supplied species, including impossible bins, followed
by exact rational expectation and variance for each requested depth. Zero-count
species retain their bins but never contribute richness. Query order and repeated
depths are preserved. Zero queries emit the community header and no depth rows.

Input is one ASCII line followed by EOF:

```text
S count_1 ... count_S Q depth_1 ... depth_Q
3 1 2 3 7 0 1 2 3 4 5 6
```

There must be 1–12 nonnegative abundance counts totaling 1–32 individuals and
0–16 depths in `[0,N]`. Tokens contain one or two decimal digits. Spaces, tabs,
and CR are separators; LF terminates the record. The buffer admits at most 255
bytes before LF (or at most 255 bytes with exact EOF). Additional records,
negative numbers, malformed or missing tokens, excessive dimensions, and
out-of-range depths are refused with `ERROR invalid_input` and exit 64. Exit 0
means all requested results were emitted. No authority grants or file/network
access are part of this application's input flow.

For each depth, `denominator` is `choose(N,depth)`, and each `bin=R numerator=W`
is the exact combinatorial count of individual subsets with richness R.
`expectation_numerator/expectation_denominator` and
`variance_numerator/variance_denominator` are exact, unreduced rational values.
The output therefore has no decimal-rounding loss. For counts 1,2,3 at depth 2,
the richness weights are 0,4,11,0 out of 15, expectation is 26/15, and variance
is 44/225.

The private kernel multiplies bounded species generating polynomials. Counts
never exceed `choose(32,16)=601080390`. Moment arithmetic centers at floor(E)
before multiplying by the denominator: its worst-case bound is
`37*choose(32,16)^2 < 2^64`. This avoids overflowing the uncentered squared
moment intermediate while preserving the exact result. The accumulation helper
receives a fixed-array row reference returned by a selector; writes reach the
original DP matrix. Compiler regression controls
cover returned, forwarded, generic and shared row loans and reject access
escalation, escaped local backing, overlap and unproved bounds. Working state is
zero-initialized by the application entry, and the kernel runs once per record.

From the repository root, using an optimized compiler of recorded provenance:

```sh
omega update --project samples/cli/independent/ecological_rarefaction --target linux_x86_64
omega run --both --keep samples/cli/independent/ecological_rarefaction/main.omg < samples/cli/independent/ecological_rarefaction/input.txt
python3 samples/cli/independent/ecological_rarefaction/validate.py --omega /absolute/path/to/omega --route native
python3 samples/cli/independent/ecological_rarefaction/validate.py --omega /absolute/path/to/omega --route terminal
python3 samples/cli/independent/ecological_rarefaction/validate.py --omega /absolute/path/to/omega --route source-invalid
```

Use the actual target for package preparation on macOS or Windows; Build binds
all three hosted entries. The acceptance runner's native build targets Linux,
the qualified host. The portable `python` spelling works in PowerShell, with
stdin piped from `Get-Content -Raw` for interactive CLI use.

`reference.py` creates the frozen `data/oracle.json` and input hashes; acceptance
never calls it or rewrites the oracle. Tiny fixtures enumerate independent
subsets of labeled individuals. Larger distributions use inclusion/exclusion
on species subsets. Exact inclusion probabilities and pairwise covariance
supply a third calculation of expectation and variance. `ORACLE_SHA256` pins
the pre-execution oracle. Fixtures cover zero/full depth, singleton communities,
unequal abundance, absent species, permutation, repeated queries, total and
simultaneous species/query capacity, EOF, whitespace, and line-length limits.

The runner validates the actual native output and the separately retained
`terminal.stdout` and `terminal.stderr` from `run --both --keep` against all
frozen counts and moment pairs. It checks normalization and moments exactly,
plus their numerical values within 1e-10. Exit agreement alone is insufficient.
Raw streams and command exits remain under ignored `build/validation`.
`controls/richness_bound` must be refused for indexing bin 13 of a 13-bin array;
a parse failure or unrelated rejection does not satisfy this control.

For a full fixture replay with one compilation, `validation/capture.rs` uses the
public complete-project preparation, native-product compilation and Terminal
runtime APIs. It retains the producer's exact artifact and provider provisions
and creates a fresh isolated Terminal domain per input; it constructs no IR or
scientific results. After the one compiler build finishes, the shared linker
verifies the CLI bytes against its Cargo/mbx unit output and traverses its exact
dependency fingerprints. It rejects ambiguous or stale artifacts; timestamps
do not select a compiler closure. The receipt records all linked-library hashes.
The capture source uses the CLI's 10,000,000 fuel allowance and worker stack.

Run from the repository root, with a fresh capture output directory:

```sh
python3 tools/link_terminal_sponsor.py --omega /absolute/path/to/omega --source samples/cli/independent/ecological_rarefaction/validation/capture.rs --output /absolute/path/to/terminal-capture
/absolute/path/to/terminal-capture samples/cli/independent/ecological_rarefaction/main.omg /absolute/path/to/captures
python3 samples/cli/independent/ecological_rarefaction/validate.py --omega /absolute/path/to/omega --route captured --captures /absolute/path/to/captures
```

Require capture exit 0 before validating its files. Both this capture path and
the public `run --both --keep` path are numerical acceptance routes.
