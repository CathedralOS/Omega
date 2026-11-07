# Terrain contour tracing (NEW043)

An independent bounded scientific sample: extract marching-squares edge
crossings from a 3×3 terrain grid, stitch every segment into an open or closed
contour, and emit paths with exact coordinates plus a topology report.
It imports only the existing hosted standard console and core facilities.

Input is exactly eleven ASCII digits, optionally followed by LF: one level,
one saddle policy, and nine row-major heights. Heights and level range 0–9.
Policy 0 separates high corners in a saddle; policy 1 connects high corners
(by separating low corners). Elevations equal to the level are rejected:
vertex degeneracy would need a separate scientific policy. Other invalid
records exit 64; an internal graph invariant failure exits 70.

This is an explicitly declared piecewise bilinear scientific grid model.
Edge intersections linearly interpolate. Saddle topology is a categorical
choice, not an asymptotic-decider claim about the bilinear interior. Cells
are numbered row-major 0–3. Horizontal edges are 0–5, vertical edges 6–11.
Coordinates use units of 1/2520 (the LCM of differences 1–9), so every accepted
crossing is exact. Absolute and relative comparison tolerances are both zero.
No floating-point or unfinished numerical library is involved.

`terrain/crossings.omg` computes shared crossings; `terrain/extract.omg`
classifies cells and owns a bounded eight-segment graph; `terrain/stitch.omg`
consumes each segment once, preferring boundary starts before cycles.
`reporting/report.omg` prints topology, per-cell segments and paths. Closed
paths repeat their first vertex. Storage is bounded to twelve edges, eight
segments/paths and sixteen path vertices. The public entry is `Main::main`,
explicitly bound to the existing linux_x86_64 program-entry provider.

Examples:

```text
10000020000   closed central hill
10000000000   flat grid below the level
10024024024   ramp with boundary exits
10202020202   checkerboard with high corners separated
11202020202   same elevations with high corners connected
```

No existing sample supplies this workflow. The matrix-multiply sample tests
array multiplication, cellular-automaton tests neighborhood evolution, and
Langton's ant tests a grid walk. They have semantic overlap only in bounded
array access; none extracts a contour graph, resolves a saddle or stitches
paths. A repository search for contour/marching/terrain/saddle on the base
found no related sample.

`data/reference.py` independently derives crossings with exact rational
arithmetic and constructs saddle pairs from corner neighborhoods rather than
copying the application's sixteen-case dispatch. `data/expected.json` and
`data/FROZEN.json` were generated before the first Omega check or execution.
All sixteen cell masks under both policies appear in 32 fixtures; the emitted
per-cell segments make those cases independently observable. Additional cases
cover flat terrain, hills, pits, exits, multiple saddles, asymmetric crossings,
EOF input and malformed input. `validate.py` canonicalizes path orientation
and cycle rotation, then compares all coordinates, segments, paths and counts;
process status and stderr are also checked. It never updates expectations.

From the repository root, with a verified compiler at `$OMEGA` and matching
`OMEGA_LIBRARY_ROOT=source/library` (use absolute paths when needed):

```sh
$OMEGA update --project samples/cli/independent/terrain_contour_tracing --target linux_x86_64
# Read the generated review and explicitly accept its intended rows before --resume.
$OMEGA --check --target linux_x86_64 samples/cli/independent/terrain_contour_tracing/main.omg
$OMEGA --target linux_x86_64 samples/cli/independent/terrain_contour_tracing/main.omg
python3 samples/cli/independent/terrain_contour_tracing/validate.py --executable <published-executable>
printf '10000020000\n' | $OMEGA run --both --keep samples/cli/independent/terrain_contour_tracing/main.omg
```

The separate complete `controls/intended_invalid` project deliberately reads
index 2 of a two-element array. Its check must fail for the documented bounds
rule (`wiki/spec/language/numeric_values.md#collection-bounds`), not package preparation, imports or
entry binding. Review its std dependency independently; do not count an
unrelated refusal as intended-invalid acceptance.

Build outputs and local package decisions are ignored. See the bundled
validation evidence for exact executed commands, binary identity, results and
remaining unsupported or denied routes. This candidate is for independent
review and has not been published to main.

Recorded outcome on main base `33e34fd97e763f4725c6290f0a0d6ea1ec829224`:

- Fresh `mbx build -p omega --release`: exit 0, 8m06s, four jobs.
- Default sandbox check: custody refusal because synthetic `/` ownership is
  UID 65534. Supported per-command host execution reached ordinary checking.
- Three application checks resolved authored arithmetic and borrow errors;
  the final check reports only the missing accepted std entry binding.
- `omega update` checked both source packages and produced a 285-decision
  review. Source rendering exceeded its resource limit; exact local sources
  are available. The first capture correctly refused a concurrent validator
  edit; the second completed with exit 3 and no accepted lock.
- Automatic approval rejected the scoped acceptance request for broad std
  external supplies and FilesystemHost scope. Acceptance was not retried;
  the review remains entirely pending and no accepted lock exists.
- Native compile: exit 1, unaccepted entry binding. `run --both --keep` with
  finite hill input: exit 200 at the same compile refusal. No native executable,
  contour output, or Terminal interpretation was produced. The 48 runtime
  fixture checks are pending, not passing.
- Separate intended-invalid check: exit 1, exactly “cannot prove index 2 is within length 2”; this is the intended collection bounds rule.

The review bundle retains command/result evidence and the unaccepted review.
The original review proposal predates these documentation/evidence additions;
source identity therefore differs. After independent authorization, regenerate
review from the final candidate before accepting; do not resume the historical
proposal blindly. Required choices cannot be omitted, and a rejected choice
prevents publication (`review/decision/policy/document.rs`,
`operations/package_change.rs` in the package-manager source). No compiler
semantic defect was demonstrated and no shared implementation was changed.
