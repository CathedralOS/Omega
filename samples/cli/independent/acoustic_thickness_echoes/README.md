# Acoustic thickness echoes (NEW020)

A complete stdin-driven Omega application for a declared normal-incidence
sample model. It predicts bounded reflected paths through up to four layers,
classifies up to eight measured echoes, and estimates thickness only for a
unique primary reflection. No worker exports or unfinished numerics package
are imported. Its private records, decimal parser, path enumeration and report
formatter live in this directory. Python supplies an independent oracle only.

## Input and physical model

Whitespace-separated unsigned decimal tokens, at most nine digits each:

```
layer_count echo_count window_ns reference_present reference_ns
speed_m_per_s thickness_um   # repeated layer_count times
arrival_ns                  # repeated echo_count times
```

The `#` comments above explain the format and are not accepted input. Layer
count is 1..4; echo count 1..8; window 0..1000 ns; reference flag 0 or 1;
reference 0..1000000 ns; speed 1000..10000 m/s; thickness 1..1000000 um;
arrival reference..9000000 ns. A missing reference rejects the acquisition.
Negative numbers, excess digits, incomplete records and surplus tokens reject.
Leading/trailing spaces, tabs, CR and LF are accepted. Reading acknowledges
host blocking and traps; finite fixtures end at EOF.

For each layer, integer round-trip duration is `floor(2000*thickness/speed)`
nanoseconds. Durations are accumulated before multiplying each boundary time by
orders 1, 2 and 3. These repeat paths are an explicit simplified model; there
is no attenuation, mode conversion, oblique incidence, transducer delay fitting,
or arbitrary interlayer reverberation. A relative arrival matches every path
whose absolute residual is at most the input window (inclusive). More than one
match is ambiguous. A unique repeated path identifies its layer and order but
produces no thickness estimate. A unique primary estimates the current layer
as `floor((relative_arrival - previous_boundary_ns)*speed/2000)` micrometres,
using the prior layers' model times. It does not jointly fit all unknown layers.
Subnanosecond durations may quantize to zero and become ambiguous.

All carriers use declared Saturating arithmetic; validated bounds keep every
physical product under 90 billion, so saturation is never needed for accepted
inputs. Integer division deliberately truncates nonnegative model durations.
Reports contain exact integer estimates; numerical comparison tolerance is zero.
The physical matching window is separate from validation tolerance.

## Frozen independent results

`data/oracle.json` was frozen before any Omega execution, SHA-256
`5b0dc70c24f6aa0755767fa16f45f3733e831bf2533cf288603ed9952e9ad4f2`.
Each `.stdin` has a separately authored `.stdout` and pinned exit. Cases cover
separated primary/repeated/unmatched paths; overlapping echoes; two nanosecond
inclusive window endpoints and a just-outside echo; nondivisible speeds;
four layers; zero-duration ambiguity; missing reference; zero, excessive and
negative speeds; early echoes; oversized layer count; malformed, long, short,
surplus and empty inputs; eight echoes filling the final slot; and a rejected
nine-echo header. Valid acquisitions exit 0; runtime rejection exits 2.

For example, `separated` yields 1000 um at layer 1 and 1500 um at layer 2,
then identifies a double layer-1 and triple layer-2 path plus an unmatched echo.
`overlap` retains two paths at 2000 ns instead of guessing a thickness.
`window` estimates 1496 and 1504 um at the two layer-2 window endpoints.
`quantized` yields 1000 and 1197 um under the declared integer model.

## Build and run

From the repository root with a compiler built from the same source revision:

```
omega update --project samples/cli/independent/acoustic_thickness_echoes --target linux_x86_64
# Read and accept each requested package review decision, then:
omega update --resume --project samples/cli/independent/acoustic_thickness_echoes
omega --check --target linux_x86_64 samples/cli/independent/acoustic_thickness_echoes/main.omg
omega run --keep samples/cli/independent/acoustic_thickness_echoes/main.omg < samples/cli/independent/acoustic_thickness_echoes/data/separated.stdin
omega run --both --keep samples/cli/independent/acoustic_thickness_echoes/main.omg < samples/cli/independent/acoustic_thickness_echoes/data/separated.stdin
python3 samples/cli/independent/acoustic_thickness_echoes/validate.py --compiler /absolute/path/to/omega
```

`build.omg` binds the exact entry for Linux x86_64, Windows x86_64 and macOS
arm64 using ordinary std providers. This validation executor is Linux x86_64;
other hosts are declared targets, not claimed validation results. Package
review and trust admission remain mandatory. A denied acceptance is recorded
without retry. Generated images, reports and logs belong under ignored `build/`.

The repository native sweep has no fixture stdin surface, so this input-driven
sample deliberately uses its own complete driver instead of no-input header
annotations. The validation driver executes one Omega command at a time, saves commands,
status, stdout and stderr, verifies all frozen file hashes, compares native
stdout and statuses, and runs native/Terminal comparison with finite input.
The CLI's `--both` checks termination agreement; the driver separately checks
native stdout against the complete frozen output. This is not independent
Terminal stdout trace validation.

## Intended-invalid control

`controls/invalid_bounds/main.omg` is a separate complete source program with
no hosted effects. It indexes slot 4 of a four-element layer store. With the
ordinary core slice operator imported, the required fact `index < items.len`
is structurally false. The control passes validation only if compilation
rejects specifically for that `Slice::index` contract; a cache, package,
syntax or unrelated proof refusal is not a pass.

## Semantic overlap review and ownership

Compared `cli/audio/sine_tone` (WAV oscillator, no inversion),
`cli/arithmetic/dual_float` (numeric widths), `cli/arithmetic/dot_product`
(reduction), `cli/collections/matrix_multiply` (nested array loops),
`cli/algorithms/insertion_sort` (constant adjacent comparisons), and
`cli/text/parse_int` / `parse_number` (scripted decimal inputs). The new
workflow combines runtime bounded records, layered travel-time prediction,
ambiguity retention and inverse thickness estimation; it does not rename any
existing fixture constants.

DRI: NEW020 acoustic thickness echoes. Namespace and local candidate branch are
isolated. Claims status timed out, and the board-scoped claim rejected because
NEW020 is absent from TASKS.md; no successful registry ownership is claimed.
The starting checkout and all prior work are preserved. No remote worker
branches or additional workers are used. Review and integration follow the
repository's main-only landing workflow; this assignment returns a candidate.
