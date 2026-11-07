# Principal component analysis

This complete stdin application calculates PCA for 3–16 observations and 2–4
features. `build.omg` explicitly binds the Linux x86-64, Windows x86-64 and macOS
arm64 entries. Numerical qualification in this directory is on Linux x86-64.

Input is whitespace-separated ASCII: `mode observations features`, followed by
exactly `observations * features` finite decimal measurements in row-major order.
Mode `0` uses covariance; mode `1` uses correlation. For example:

```text
0 5 3
2 1 5
4 3 2
1 4 3
5 2 7
3 6 4
```

The reader consumes at most 4096 bytes, 67 tokens and 64 bytes per token. Decimal
syntax permits a leading sign, a decimal point and a signed `e`/`E` exponent with
absolute exponent at most 1000; it requires mantissa and exponent digits.
Empty, truncated, extra, malformed, nonfinite and out-of-dimension inputs refuse
with exit 64 and `error invalid input`. Correlation refuses constant features
with exit 65. Covariance accepts constant features and rank deficiency, including
an entirely zero matrix. Intermediate nonfinite covariance or failed numerical
qualification refuses with exit 66. Finiteness alone does not guarantee that
covariance products are representable in binary64.

Output includes feature means, unbiased sample covariance (denominator `n-1`),
the matrix analyzed, decreasing eigenvalues, explained variance fractions,
eigenvectors as columns, row-major principal scores, maximum absolute
reconstruction error in centered analysis units, relative eigen-residual and
orthogonality error. Arrays contain only active dimensions, flattened by row.
Numbers use scientific notation with twelve digits after the decimal point.
In correlation mode, scores use centered measurements divided by their sample
standard deviations; reconstruct original units using those deviations and means.
Zero total variance produces zero explained fractions.

The implementation uses 40 cyclic Jacobi sweeps of the symmetric matrix and
checks the maximum entry residual relative to the leading eigenvalue and the
maximum entry orthogonality error against `1e-8`. Zero eigenvalues are valid.
Signs and rotations within a repeated eigenspace are not unique.
`validation/verify.py` additionally requires spectral-norm residual and
orthogonality errors at most `1e-8`, and compares each repeated-eigenvalue
subspace projector against the independent oracle. Scores are checked against
centered data times the actual emitted basis; reconstruction is checked in
original measurement units. These checks run on captured numerical output,
independently of exit parity.

From the repository root, use the compiler from a recorded build:

```sh
omega update --project samples/cli/independent/principal_component_analysis --target linux_x86_64 < /dev/null
omega --check samples/cli/independent/principal_component_analysis/main.omg < /dev/null
omega run samples/cli/independent/principal_component_analysis/main.omg < samples/cli/independent/principal_component_analysis/input.txt
```

Review normal package decisions before resuming any pending update. Std source
risk disclosures do not grant host access, and no validation script edits those
gates. Use the platform-approved host command route when cloud filesystem UID
mapping prevents Omega from establishing cache custody; retain the checks.

`validation/frozen.json` and `FROZEN.sha256` were produced before sample execution
by `validation/oracle.py`, which centers measurements and calls NumPy SVD rather
than reimplementing Jacobi. The frozen cases cover both modes, translation,
column permutation, rank deficiency, zero covariance, repeated eigenvalues,
minimum/maximum dimensions and small/large measurement scales. Refusal streams
cover malformed/nonfinite data, dimensions, byte/token limits and constants.
The separate `controls/feature_bound` source must reject a fifth feature index
with its authored diagnostic and never enter execution.

The Terminal sponsor uses the public package preparation, compilation,
publication and runtime APIs with the exact retained Terminal artifact and
provisions of the produced native image. Its isolated ProcessExit delegation
and finite input stream are explicit; no checked-tree interpreter or alternative
application numerics exist. Link it to the exact successful CLI build artifacts:

```sh
mbx build --release -p omega --message-format=json > build/pca-build.jsonl
python3 samples/cli/independent/principal_component_analysis/validation/link_terminal.py build/pca-build.jsonl . build/pca-terminal
# CASE_NAMES lists frozen case names, one per line.
build/pca-terminal samples/cli/independent/principal_component_analysis CASE_NAMES build/pca-capture
python3 samples/cli/independent/principal_component_analysis/validation/verify.py --native "$(cat build/pca-capture/native.path)" --output build/pca-native-results
python3 samples/cli/independent/principal_component_analysis/validation/verify.py --terminal build/pca-capture --output build/pca-terminal-results
```

Python 3 with NumPy is required for the numerical verifier. Python, rustc and
mbx/Cargo are available on both supported development hosts; sponsor compilation
requires the same toolchain as the CLI build. The example sponsor currently
selects Linux x86-64 explicitly. Changing its target needs an actual host
qualification; the declarations alone do not establish cross-host numerical
acceptance.
