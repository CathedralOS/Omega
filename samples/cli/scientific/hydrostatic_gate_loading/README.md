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

On `omega-mac`, use the project Build to select its ARM64 macOS entry. Linux
x86-64 uses the same entry machine. The bundled system library supplies standard
input, standard output and process exit.

```sh
cd samples/cli/scientific/hydrostatic_gate_loading
omega packages update --project . --target macos_arm64
# Review the pending decisions and resume the update after accepting them.
omega packages update --project . --resume
omega run --target macos_arm64 build.omg < data/rectangle.in
omega --target macos_arm64 --build-dir build/native build.omg
./build/native/omega-program < data/rectangle.in
```

The sample gate uses `input.txt`, a copy of the rectangle fixture. The
[reversed interval](../invalid_hydrostatic_gate_loading_intended/main.omg)
checks the compile-time requirement that the upper elevation exceeds the lower
elevation. It calls the interval calculation with elevations 2000 and 1000.
The malformed stdin fixtures exercise the parser in the running application.

Analytic rectangle and triangular-width force/moment anchors also constrain the
reference, as do subdivision and vertical-translation identities. Frozen
`data/oracle.json` contains exact fractions; `data/tolerances.json` fixes absolute
micro-unit and relative tolerances. `data/frozen.sha256.json` authenticates the
frozen data. Do not regenerate it to accommodate a compiler result.

Cases cover dry, partial, at-node, fully wetted, above-top, triangular,
irregular, elevated, zero-width, millimetre and maximum profiles, plus
subdivision/translation variants. Nineteen malformed fixtures must produce the
same exact error artifact and exit 64.

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
