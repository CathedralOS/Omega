# Hydrostatic gate loading (ADD02)

A complete offline synthetic research application in Omega. Supply a bounded
vertical gate silhouette and several water elevations through stdin. The
application validates the profile, clips every linear width interval at the
moving waterline, integrates pressure loads, and emits each wetted interval and
aggregate area, force, signed moment and center of pressure. Its algorithms,
fixed storage, scanner and formatter are private to this directory. The only
external code is the existing bundled standard console/core provider surface.

Input is whitespace-separated unsigned decimal integers:

```text
node_count level_count specific_weight_N_per_m3 hinge_elevation_mm
node_elevation_mm node_width_mm      # repeat node_count times (no comments)
water_elevation_mm ...              # level_count values
```

Counts are 2–8 nodes and 1–8 levels. Specific weight is 1–20000 N/m³.
Elevations, widths, hinge and waterline are 0–100000 mm. Nodes must be strictly
ascending; zero widths are permitted. The scanner accepts ASCII space, tab, CR
and LF, at most six digits per token and 4096 consumed bytes including EOF
probes. It rejects incomplete records, extra tokens, signs, decimals and other
characters. Parsing and all validation finish before any load output. Invalid
input prints exactly `ERROR invalid bounded gate input` and exits 64. Valid input
exits 0. No command-line options or interactive pause are needed.

The model is a vertical, planar silhouette whose horizontal width varies
linearly between nodes. Water is quiescent and one-sided with uniform specified
specific weight; gauge pressure is `gamma * (water - elevation)` below the
waterline. A dry span is omitted. Moments are signed about the supplied
horizontal hinge: positive above it. Center is the absolute elevation of the
resultant and is `undefined` for zero force. Positive-height intervals of zero
width still count as wetted geometry. No dynamics, inclined gates, two-sided
pressure, structural stress, fluid compressibility, or design/safety conclusion
is modeled. This is deterministic synthetic research, not an engineering tool.

`WET` rows preserve segment indices and the clipped endpoints and widths.
Lengths and center use integer micrometres (`*_um`), area uses one millionth of
m² (`area_micro_m2`), force uses micro-newtons, and moment uses micro-newton-metres.
Values are rounded half away from zero. They are observations of computations,
not an embedded table of expected results. The chosen bounds keep f64
intermediates finite and scaled output below signed i64 capacity.

## Run and review

Use a compiler built from this candidate, and select its matching library.
The pristine base `9a0463cc42` accepts the source but cannot produce its native
artifact. The candidate includes bounded general repairs for computed indexed
scalar stores and per-instruction copies for fixed integer operands whose source stays live.
Current main supplies canonical requirement-use traversal through constructed
record fields, replacing the earlier local repair. Complete corpus controls
reproduce the failures and retain exit/input/output expectations. No application algorithm or expected numerical result depends on
an active worker's implementation.
From the repository root (replace `OMEGA` with that compiler's executable):

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
APP=samples/cli/independent/hydrostatic_gate_loading
OMEGA update --project "$APP" --target linux_x86_64
# Read pending package-policy decisions and accept them in the review file,
# then OMEGA update --resume --project "$APP" if review is requested.
OMEGA --check --target linux_x86_64 "$APP/main.omg"
OMEGA run --keep "$APP/main.omg" < "$APP/data/rectangle.in"
OMEGA run --both --keep "$APP/main.omg" < "$APP/data/piecewise.in"
OMEGA --check --target linux_x86_64 "$APP/intended_invalid/main.omg"
```

The intended-invalid command must reject the reversed interval's violated
`upper > lower` precondition. It is a separate complete application with its
own build file; a rejection for package policy, syntax, or provider absence
would not count. Runtime-invalid inputs exercise the valid application's parser
and validator, independently of this static negative control.

`validation/reference.py` generated the fixtures and expected outputs **before
Omega execution**. Its exact rational Simpson integration of degree ≤3
integrands is independent of the application's local polynomial coefficients.
Analytic rectangle and triangular-width force/moment anchors also constrain the
reference, as do subdivision and vertical-translation identities. Frozen
`data/oracle.json` contains exact fractions; `data/tolerances.json` fixes absolute
micro-unit and relative tolerances. `data/frozen.sha256.json` authenticates the
frozen data. Do not regenerate it to accommodate a compiler result.

`validation/verify.py CASE OUTPUT EXIT` checks an artifact against its frozen
oracle, exact geometry/record identities, force/area/moment conservation, and
center/moment consistency. Cases cover dry, partial, at-node, fully wetted,
above-top, triangular, irregular, elevated, zero-width, millimetre and maximum
profiles, plus subdivision/translation variants. Nineteen malformed fixtures
must produce the same exact error artifact and exit 64.

## Overlap review

At base `9a0463cc42`, searching the actual main sample sources for quadrature,
Simpson, trapezoid, hydrostatic, pressure and waterline found no SCI-16 sample.
`simulation/particle_sim` performs Euler time integration of a particle;
`rendering/bouncing_particles` and `bouncing_ball_2d` integrate motion. Neither
owns waterline clipping, variable-width gate geometry, or pressure resultants.
The stated SCI-16 quadrature workflow is distinguished by this complete
moving-waterline load application; no active worker's code or numerics package
is imported. This comparison covers landed source only, with no remote worker
branch inspection.
