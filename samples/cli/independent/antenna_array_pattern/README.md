# Antenna array pattern (NEW039)

A complete bounded console application evaluating four-element antenna arrays.
The explicitly declared scientific model is a scalar far-field sum of isotropic,
uncoupled elements in a linear geometry. Positions are in wavelengths, weights
are real voltage excitations, steering and observation are direction cosines.
It excludes mutual coupling, polarization, near-field effects and element patterns.

`A(u) = sum_j w_j exp(i 2 pi x_j (u - steering))`, `P(u) = |A(u)|^2`.
Normalized power divides by `sum(abs(w))^2`; zero excitation reports zero.
Co-located elements are summed independently and reported as duplicate pairs.
Symmetry is `max |P(u)-P(-u)|` across the sampled grid, rather than an assumed
property of a steered array. A null means raw power below `1e-10`.

The entrypoint reads a selector from stdin: `0` broadside, `1` steering to
`u=0.5`, `2` zero excitation, `3` duplicated positions, `4` taper `[1,2,2,1]`.
EOF, newline or `a` runs all five. Selectors `5` and `6` deliberately supply
out-of-range position and weight fixtures and must exit 65. Unknown selectors
or trailing bytes after a selector must exit 64. Selectors accept a
single optional LF, followed by EOF. Close stdin after the request, including
after LF; hosted reads can block. Fixtures close stdin. Positions and
weights must lie in `[-2,2]`, steering in `[-1,1]`; ordered comparisons reject
nonfinite values. The UI exposes a bounded fixture catalog rather than claiming
a general floating-point text parser.

Output contains nine rows per case at `u=-1,-0.75,...,1`, raw and normalized
power in integer millionths, null flags, duplicate count and symmetry residual.
Ten-digit decimal fields include leading zeros. The stdout stream is the
beam-pattern artifact; the validation commands retain it under ignored `build/`.

The application owns private geometry validation, direct phasor summation and
range-reduced degree-20 cosine. It uses ordinary existing float, conversion and
Console providers, with no numerics package or dependency on another worker.
`domain/phasor.omg` reduces bounded angles to `[-pi,pi]`; Taylor remainder is
below `8e-11`. The independently frozen `data/oracle.json` uses Python's cmath
exponential, not the Omega polynomial. Its SHA-256 was fixed before execution:
`3e352196111cd4296a2d556e5e896abc3ad7c7d55df9b233a14a8bafdf9a9699`.
Frozen tolerances: raw power `2e-8`, normalized power `2e-9`, printed values two
integer millionths (including truncation). The Omega entrypoint checks the raw and normalized tolerances before rounding and exits 66 on a numerical failure. Null flags, row order, duplicate
counts, completion marker and exit codes are exact. `reference.py` checks the
frozen oracle and strictly validates stdout; it never supplies results to Omega.

Semantic-overlap review searched existing samples for antenna, beam, phasor,
Fourier and DFT workflows. `arithmetic/vector_distance` covers scalar vector
magnitude; `arithmetic/stats_compute` covers aggregate statistics;
`audio/sine_tone` synthesizes a waveform using a lookup table. This application
adds geometry-dependent coherent interference, steering, null classification,
zero normalization, duplicate handling and symmetry diagnostics.

From the repository root, with a compiler built from the same base:

On `linux_x86_64`, all 15 authored input fixtures now agree in native and
Terminal execution: eight valid selections exit 0, five malformed-input cases
exit 64, and two invalid geometries exit 65. Both execution streams are checked
against the frozen 45-row oracle, including symmetry, duplicates, null flags,
row order and completion markers.

Each selector and phase-reduction dispatch now covers its own subject, following
[dispatch coverage](../../../../wiki/spec/language/patterns.md#coverage-and-domain-tests).
The numerical polynomial, geometry fixtures, frozen oracle and its tolerances
are unchanged. The reference checker verifies the frozen SHA-256 before
recomputing with cmath; host libm rounding is compared using the authored
absolute tolerances rather than requiring bit-identical floating-point values.

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
OMEGA=/absolute/path/to/omega
SAMPLE=samples/cli/independent/antenna_array_pattern
python3 "$SAMPLE/reference.py"
"$OMEGA" update --project "$SAMPLE" --target linux_x86_64
# Inspect build/package-manager/review-linux_x86_64.txt and accept only reviewed decisions.
"$OMEGA" update --project "$SAMPLE" --resume
"$OMEGA" --check --target linux_x86_64 "$SAMPLE/main.omg"
python3 "$SAMPLE/validate.py" --omega "$OMEGA"
```

`validate.py` uses `run --both --inputs DATA --outputs OUTPUTS` to compile once
and replay all 15 finite inputs. It checks every native and Terminal status,
checks each stdout against the independent frozen reference, and requires equal
streams with empty stderr. Captures and command records are retained under
`build/validation/`. `run --target` is compile-only; matching-host execution uses
`run --both` without that flag. `--native EXECUTABLE` remains available for
native-only replay of an already retained artifact. A check-only result cannot
establish runtime acceptance.

`controls/intended_invalid/invalid.omg` is a separate complete application with build
metadata and entry binding. It writes index 4 of `[i32;4]`, violating the
ordinary bounds obligation. Its rejection must specifically identify
`cannot prove index \`4\` is within length 4`; unrelated package, type or provider
rejection is insufficient. Check it separately from the valid application with `omega --check --target linux_x86_64 controls/intended_invalid/invalid.omg` (use its full sample-relative path from the repository root). The alternate source filename keeps it out of runnable-main discovery; the role-aware sample gate discovers and validates this intended-invalid control separately.
