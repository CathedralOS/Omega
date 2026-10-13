# Planar truss elasticity (W2-07)

A bounded, small-displacement, linear-elastic planar truss solver. Axial bars
carry tension and compression; joints have no bending stiffness. The model uses
consistent arbitrary units, zero prescribed displacement, and positive EA.
This is an educational numerical sample, not a qualified structural design tool.

Input is EOF-terminated ASCII signed decimal integers, with spaces, tabs, CR or
LF between tokens. A leading minus is allowed; plus, decimal points, exponents,
NaN, infinity and other text are refused. Maximum token magnitude is 1000000,
maximum seven digits, maximum 4095 bytes, and maximum 98 tokens. Numeric inputs
are integers in the chosen units; fractional geometry/material/load syntax is
outside this sample's published interface.

```text
node_count bar_count
# node_count rows: x y fixed_x fixed_y force_x force_y
# bar_count rows: first_node second_node EA
```

There must be 2–8 nodes and 0–16 bars. Coordinates and forces are in [-1000,1000],
flags are 0 or 1, indices are zero-based and refer to existing nodes, and EA is
1–1000000. Bars with coincident endpoints or zero length are refused, including
self-bars. The stream must contain exactly the fields specified by the counts.
The `#` annotations above are explanatory, not supported input comments.

`domain/solver.omg` assembles each element as `(EA/L) d d^T`, with
`d=(-cos(theta),-sin(theta),cos(theta),sin(theta))`. Fixed coordinates and unused
capacity are eliminated with zero displacement. Gaussian elimination on the
symmetric positive-definite free stiffness uses a scale-relative pivot test;
there is no stabilization or invented solution for mechanisms. A pivot no larger
than 1e-12 times the largest constrained matrix entry refuses a mechanism or
numerically unresolved structure, even with zero load. This conservative test is
not a proof of a condition-number bound. `validation/reference.py` independently
scatters explicit 4x4 endpoint matrices and solves only free equations using
70-digit Decimal partial pivoting.

The output contains decimal numbers with twelve fractional digits, truncated
toward zero. Integer and fractional portions are emitted separately so large
energy/work values do not overflow a scaled i64. Output magnitude must be below
1e12; nonfinite or larger output refuses with exit 66. Rows are:

- `N,index,ux,uy,reaction_x,reaction_y` for each node. A free coordinate has zero
  support reaction. Fixed-coordinate reactions include loads applied at supports.
- `B,index,extension,force` for each bar. Positive force and extension mean tension.
- `S,strain_energy,external_work,max_nodal_equilibrium_residual,balance_x,balance_y,moment`.
  External work is `F^T u`, so linear-elastic equilibrium requires twice the
  strain energy to equal that work. Balance includes applied forces plus reactions;
  moment is about the coordinate origin. Nodal residual is the maximum absolute
  component of `K u - F - R`.

Exit 0 means solved; 64 reports invalid input or geometry; 65 reports a mechanism
or unresolved stiffness; 66 reports a numerical output outside the published
range. Invalid data never produces a successful result row.

The 37-case oracle was frozen before application execution. Its SHA-256 is
`92d5871f34cb91e4035ef2a4abdb3a586d66e67d6a4ca2a4cbeafb218f91b9d6`.
Thirteen original successes plus CRLF cover analytic axial-bar and 3-4-5 triangle
cases, compression, horizontal loads, an asymmetric eight-node/sixteen-bar dense
system, reordered/reversed bars and nodes, 90-degree rotation with transformed
loads/support flags, unloaded stable geometry, unloaded mechanisms, all-fixed
structures, support loads and stiffness/load scales. Refusals exercise dimensions,
indices, flags, EA, zero length, nonfinite spelling, malformed and bounded input.
`freeze.py` documents oracle construction and refuses to overwrite the frozen
file; `verify.py` only reads it and certifies analytic values and dense recomputation.

Qualification compares displacement, reaction, extension and bar force with
1e-7 relative tolerance plus 2e-12 times the larger of one and the family's
reference scale. It separately reconstructs nodal equilibrium from emitted bar
forces; both reported and reconstructed residuals must be at most 1e-8 times
the larger of one and the maximum applied load component. Force and moment
balances, support displacement, extension/force consistency, strain energy,
external work and `2U=F^T u` are checked from the actual emitted values.
The constitutive reconstruction propagates endpoint decimal-emission error
through EA/L; the frozen force comparisons and equilibrium thresholds remain
unchanged. The energy/work identity must agree within 1e-8 times the larger of
one, work and twice the energy.

From the repository root, using a compiler built from recorded source:

```sh
python3 samples/cli/independent/planar_truss_elasticity/validation/verify.py --oracle-only
omega update --project samples/cli/independent/planar_truss_elasticity --target linux_x86_64
# Inspect pending review decisions and accept only authorized decisions, then:
omega update --resume --project samples/cli/independent/planar_truss_elasticity
omega run --both samples/cli/independent/planar_truss_elasticity/main.omg < samples/cli/independent/planar_truss_elasticity/input.txt
python3 samples/cli/independent/planar_truss_elasticity/validation/build_terminal.py --omega /absolute/path/to/omega --output /absolute/path/to/terminal-validator
/absolute/path/to/terminal-validator /absolute/path/to/sample /absolute/path/to/captures
python3 samples/cli/independent/planar_truss_elasticity/validation/verify.py --captured /absolute/path/to/captures --output /absolute/path/to/terminal-results
python3 samples/cli/independent/planar_truss_elasticity/validation/verify.py --native /absolute/path/from/captures/native-path.txt
```

The private test consumer links the exact compiler library fingerprints retained
by its verified mbx CLI build. It uses ordinary package preparation and native
compilation, then executes that retained Terminal artifact and its admitted
provisions in a fresh isolated runtime per bounded stdin fixture. It captures
both streams and completion before publishing the same native product. It never
accepts packages or supplies expected-result callbacks. The application remains
Omega; exit parity alone does not establish numerical agreement. Its 100-million
fuel limit bounds interpretation; exhaustion is a validation failure.

`controls/invalid_displacement` is a separate intended source-invalid project:
reading `displacement[16]` from `[f64;16]` must reject specifically with
`cannot prove index 16` and `within length 16`, rather than a
package/provider/entry error.

```sh
omega --check samples/cli/independent/planar_truss_elasticity/controls/invalid_displacement/main.omg < /dev/null
```

Generated locks, native programs, review records and execution captures stay in
ignored `build/` or other repository-ignored paths. Frozen inputs and independent
oracles are committed. Acceptance observations identify their exact compiler,
source/oracle identities, target, host, commands and any unresolved gate.

`validation/qualification.json` records Linux x86_64 qualification on compiler
revision `3c1bdf7bcc87cfb3947bd7e29399064f81120e70`: 37 cases on each native
and retained Terminal route, comprising 14 solves and 23 runtime refusals, plus
the exact source-invalid rejection. Maximum reconstructed residual is
2.061e-13 of load scale; maximum energy/work defect is 1.001e-12. Both sample
source/native gates also include the intended-invalid control.
