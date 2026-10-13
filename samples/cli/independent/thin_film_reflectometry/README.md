# Thin film reflectometry

A nonabsorbing, normal-incidence optical stack calculator. This is a complete
bounded stdin application with explicit entries on Linux x86_64, Windows x86_64
and macOS arm64. Numerical qualification currently runs on Linux x86_64.

Input is whitespace-separated plain decimal numbers in this order:

```
ambient_index substrate_index layer_count wavelength_count
layer_index thickness_nm       # one pair per layer, incident side first
wavelength_nm                  # one value per requested wavelength
```

The literal comments above illustrate the format and are not valid stdin.
Indices lie in [1, 4], layer count is an integer in [0, 8], wavelength count
is an integer in [1, 32], thicknesses lie in [0, 2000] nm, and wavelengths
lie in [200, 2000] nm. Values have 1–12 digits total, an optional minus, and
an optional decimal point with digits on both sides. Exponents, plus signs,
NaN, infinity, extra tokens and incomplete requests reject. The whole stream
is bounded to 4096 bytes including whitespace; a completed stream needs EOF
within that budget. A rejected request prints `ERROR input or domain` and exits
10. Success exits 0. Source proof or package admission refusal belongs to the
compiler (exit 1), separately from runtime input refusal.

```
omega update --project samples/cli/independent/thin_film_reflectometry --target linux_x86_64
omega run samples/cli/independent/thin_film_reflectometry/main.omg < samples/cli/independent/thin_film_reflectometry/input.txt
```

The application multiplies characteristic matrices. Fields use exp(-i omega t),
with forward waves exp(+i k z). Reflection is referenced to the incident face;
transmission to the substrate face. Each wavelength emits its wavelength,
complex electric-field reflection and transmission amplitudes, and powers
R = |r|² and T = (substrate/ambient)|t|². All emitted numerical values are
signed decimal integers scaled by 10^12 (the `pico` suffix). Integer truncation
introduces less than 10^-12 absolute quantization error. The final two fields
give minimum sampled R and its wavelength. This is a sampled minimum, not an
interpolated or continuous optimizer; exact ties retain the first input sample.

The frozen oracle solves all interface boundary equations simultaneously using
complex Gaussian elimination. It does not multiply characteristic matrices.
Analytic bare-interface, zero-thickness and quarter-wave assertions challenge
phase convention and power normalization before freezing. `fixtures.json` is
frozen and hashed before execution; `freeze_reference.py` is for an intentional reviewed
oracle update, never part of ordinary validation.

```
CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p omega -p thin-film-reflectometry-validation
target/debug/thin-film-reflectometry-validation --review samples/cli/independent/thin_film_reflectometry
python3 samples/cli/independent/thin_film_reflectometry/validate.py --runner target/debug/thin-film-reflectometry-validation --evidence build/thin-film/numerical
```

The capture harness uses normal package preparation, then executes the same
retained Terminal artifact and published native product. The validator compares
actual output values independently against frozen rows, requires every amplitude
component and power error <= 10^-8, and requires |R+T-1| <= 10^-8. It covers
zero layers, zero thickness, matched indices, quarter-wave antireflection,
reversed-stack reciprocity, maximum sizes, and malformed/nonfinite/domain
refusals. `controls/invalid_layer_bound` must reject specifically for indexing
8 into the eight-layer array. Exit parity alone is not accepted.

The capture producer has its own compiler identity. Its `--review DIRECTORY`
command refreshes normal package review; `--resume-review DIRECTORY` applies
individually inspected decisions if review is required. A normal numerical run
never edits or accepts decisions. The CLI and capture producer need their own
normal identity refresh when switching between them.
