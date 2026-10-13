# Molecular pair distribution in a periodic cube

Read whitespace-separated signed decimal integers:

```
box particles snapshots bins
upper_edge_0 ... upper_edge_(bins-1)
x y z ...  (particles triples per snapshot, snapshot-major)
```

Lengths use a common caller-chosen unit. The wire accepts box 2..1000,
2..32 particles, 1..8 snapshots, 1..16 upper radial edges and coordinates
-1000000..1000000. Edges are positive and strictly increasing, ending at most
half the box. There is an implicit lower edge of zero. Every coordinate is
wrapped into [0,box), including negative and multiple-box translations.
The input budget is 32768 bytes; integers have at most seven digits, with an
optional leading minus. Decimal fractions, exponents, NaN, infinity, malformed,
missing and extra tokens are refused. Smaller physical units allow integer
coordinates to express fractional physical lengths; all lengths must share
that unit. No external data, randomness or filesystem service is requested.

Each snapshot contributes only unordered distinct-particle pairs i<j, without
cross-snapshot pairs. Coincident particles contribute at r=0. Bins are
[0,e0), [e0,e1), ...; the final bin includes its upper endpoint. Exact interior
edge pairs belong to the bin on their right; pairs beyond the last edge are
excluded. Squared integer distances and edges preserve these decisions without
sqrt rounding. The largest squared distance is safely exactly representable.

For shell b with volume Vb=4*pi*(upper^3-lower^3)/3 and cubic volume V=box^3,
`g[b] = count[b] / (snapshots*N*(N-1)/2 * Vb/V)`. This uses the finite N
unordered-pair ideal expectation, avoiding the biased N^2 normalization.
Cumulative coordination is `2*cumulative_count/(snapshots*N)`, averaging
neighbors per particle across snapshots. Output is one header followed by
`zero_based_bin count g coordination`, with scientific decimal values.
Compensated division, a split reciprocal of pi and compensated decimal
reporting preserve the required absolute 1e-8 error even for large g.
The ordinary cubic difference and integer normalization factors are exact
within the declared bounds. Counts must match exactly.

Valid input exits 0. Input refusal prints `INVALID molecular input` and exits
2. The internal positive-box invariant is checked before remainder arithmetic;
a violated internal invariant crashes Trap. Compiler/package/custody refusals
are separate diagnostics, never successful scientific results.

From the repository root, use the normal package workflow, inspecting any
actual pending authority decision before accepting it:

```sh
omega update --project samples/cli/independent/molecular_pair_distribution --target linux_x86_64
omega run --keep samples/cli/independent/molecular_pair_distribution/main.omg < samples/cli/independent/molecular_pair_distribution/input.txt
python3 samples/cli/independent/molecular_pair_distribution/validate.py --executable /absolute/native/path
python3 samples/cli/independent/molecular_pair_distribution/validate.py --captures /absolute/terminal/captures
```

`reference.py --freeze` was run before application execution. It canonicalizes
coordinates and enumerates 27 images independently using high-precision Decimal
squared distances. Separate Decimal shell integration uses a 100-digit pi;
its results and stdin hashes are pinned in `data/FROZEN.sha256`. Validation
reads these files without regenerating or altering expectations. The matrix
covers exact edges, box wrapping, translated/permuted inputs, coincident pairs,
independent snapshots, capacity maxima, precision adversaries and input refusals.
Both native and captured Terminal values must meet the same numeric oracle;
CLI exit comparison alone is insufficient.

`validation/terminal_sdk.rs` is a host sponsor using the normal accepted native
product and its retained Terminal artifact and runtime provisions. Each case
gets finite stdin, an isolated exit domain and 100000000 fuel. It publishes the
same native product through the normal custody path and captures real Terminal
stdout/stderr/exit. Link it against the recorded compiler build, with its normal
package-manager, compiler, publication, target and interpreter dependencies.
Application numerics remain in Omega. Capture manifests name all frozen cases.

`controls/unchecked_snapshot` is a separate source-invalid application: its
snapshot-derived access has no bounds proof. It must reject for that reason.
See [qualification](evidence/qualification.md) for producer identities, actual
measurements, independent review and landing status.
