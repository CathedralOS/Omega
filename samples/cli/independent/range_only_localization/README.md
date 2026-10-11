# Range-only localization

A complete offline synthetic research application in Omega. Given three static
anchors and closed range intervals, enumerate every admissible integer position
in a bounded search window. Each position includes three squared-range residuals.
The private modules own geometry, parsing and decimal output. Only the existing
standard Console provider is used; no numerical package or worker candidate is
imported.

## Input

One stdin line, at most 127 bytes including LF:

```text
xmin xmax ymin ymax 3 ax ay lower upper bx by lower upper cx cy lower upper
```

All numbers are signed decimal integers with magnitude at most 20. Search
windows are closed and ordered; radii are nonnegative and lower <= upper.
Exactly three observations are required. Space, tab, CR and LF separate fields.
EOF without LF is accepted. Empty stdin runs the exact (3,4) demonstration.
Malformed input, full buffer, extra/missing fields and invalid bounds print
`error invalid input` and exit 64. The application processes one record per
invocation. Trailing records are outside this protocol.

The input fixture `data/unique.stdin` describes circles of radii 5,5,3 centered
at (0,0), (6,0), (3,1). Its exact output is:

```text
candidate 3 4 residual2 0 0 0
count 1
status unique
```

Output order is increasing y, then increasing x. `residual2` is
`(x-anchor_x)^2 + (y-anchor_y)^2 - lower^2`, measured in squared-distance units.
It is not radial error or a least-squares objective. A nonzero value is valid
inside a nonzero range interval. Every candidate is printed; no output cap
hides alternatives.

## Model and limits

Positions live on the unit integer lattice inside the supplied window. This is
an exact finite constraint solver, not a continuous optimizer. `unique` means
one lattice point; `ambiguous` means multiple lattice points; `inconsistent`
means no feasible lattice point **in this window**, not that the continuous
circles have no intersection. `degenerate` means all anchor centers coincide
and at least one candidate exists; the ranges supply no directional diversity.
An empty feasible set takes precedence over degeneracy. Collinear anchors can
produce mirror ambiguity and are not conflated with coincident anchors.

No moving state, filtering, traverse adjustment, neighbor averaging, statistical
confidence, sensor calibration or real-world safety claim is modeled. Units
must be consistent across coordinates and ranges. Quantization can miss a real
continuous position. At most 41*41=1681 points and three range tests per point
are visited, with constant scratch storage. Actual squared distances are <=3200;
explicit Wrapping integer types state arithmetic semantics, while input bounds
ensure no executed arithmetic overflows. The 128-byte buffer and 17-field table
are private fixed storage.

## Independent frozen evidence

`reference.py` constructs an admissible annulus set per anchor and intersects
those sets with the window, independently of the Omega point-first scan. The
first two exact circles also have independently calculated continuous
intersections (3,+/-4); the third selects (3,4). Constructed translation and
zero-radius cases challenge coordinate and sign assumptions. The oracle was
frozen before Omega execution in `data/oracle_manifest.json`, including input
and expected-output hashes. Integer results use tolerance **0**. The continuous
circle witness uses tolerance 1e-12. `validate.py` checks the frozen hashes and
compares complete stdout, stderr and exit status without regenerating output.

Fixtures cover exact unique, mirrored ambiguous, disjoint inconsistent,
coincident degenerate, nonzero bounded ranges, translated coordinates, a window
excluding the otherwise valid result, zero radius, and ten malformed runtime
inputs. `intended_invalid/` is a separate complete program with its own build
metadata: it accesses a three-element array through an unconstrained runtime
index and must reject its missing index bound.

## Commands

From the repository root, set `OMEGA_LIBRARY_ROOT` to this checkout's absolute
`source/library` path and use a binary built from the recorded base:

```sh
sample=samples/cli/independent/range_only_localization
omega update --project "$sample" --target linux_x86_64
# Read the pending review, accept the local std entry/provider decisions,
# then resume using the normal package-review workflow.
omega update --resume --project "$sample"
omega --check --target linux_x86_64 "$sample/main.omg"
omega --target linux_x86_64 "$sample/main.omg"
omega run --both "$sample/main.omg" < "$sample/data/unique.stdin"
python3 "$sample/validate.py" "$sample/build/range_only_localization"
omega --check --target linux_x86_64 "$sample/intended_invalid/rejected.omg"
```

The final command must exit nonzero and identify the unproved index bound.
See `EVIDENCE.md` for actual compiler identity, artifact paths and exact commands
from this candidate. Native support was exercised on Linux x86_64; the build
also binds existing Windows x86_64 and macOS arm64 Console entry contracts,
without claiming those hosts were tested. `reference.py` is an oracle authoring
tool, not the application deliverable.

The validator also reuses the frozen unique/error output for EOF without LF,
CRLF, tabs, the full 41x41 search window and a blank input line. A separately
frozen wide-band fixture admits all 121 points of an 11x11 window, testing
multi-digit coordinates, residuals and counts and complete output ordering.
`reference.py` reproduces both the original manifest and this wide-band oracle.
