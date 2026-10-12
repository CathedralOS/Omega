# Survey traverse (IND13)

A complete land-surveying traverse application: read a station count and up to
eight `(distance_mm, bearing_millidegree)` legs, adjust the misclosure with the
Bowditch rule (transit-rule departures weighted by leg length), print the raw
and adjusted departures and latitudes, the perimeter, the closed-figure bounds,
and the traverse precision as a `1:N` ratio. Inputs are synthetic; this is not
a validated surveying instrument.

The entire application lives here. `survey/model.omg` owns the traverse record
and results; `survey/input.omg` owns the bounded token parser; `survey/millidegree.omg`
owns integer millidegree-to-binary64 trig through the frozen 45-degree
quarter-table spline (`sin_mdeg`, `cos_mdeg`); `survey/traverse.omg` owns
departure/latitude accumulation and Bowditch adjustment; `survey/report.omg`
owns the signed fixed-decimal report. It depends only on the repository's
standard console provider. No in-progress numerics package is imported.

## Provenance (reconstruction, not the original candidate)

This directory is a **new reconstruction** authored under assignment Z47. The
historical IND13 candidate `eae2969ab400e635d7c79f1c959c3c003501b74b` and its
patch export were unreachable on this machine and remote at reconstruction
time, so the application was re-authored from the stated
numerical/application behavior (Bowditch traverse, millidegree trig, signed
fixed-decimal report) against repository contracts plus an independent
mathematical oracle. No file here claims to be the original code, and no
historical approval or lock was transplanted: the omega.lock produced by this
work is a fresh review acceptance, generated and decided locally.

What is preserved from the historical acceptance where recoverable: the
application contract (input grammar, exit codes, report fields), the frozen
oracle tolerances (`data/oracle.json`), the independent millidegree sine
reference check (max error 5.6e-16 recorded reference-only), and the
signed-report bounds policy.

## Input

Exactly `1 + 2*legs` unsigned decimal integers separated by ASCII space, tab,
LF or CR, terminated by EOF, at most 512 bytes:

```text
legs dist_1 bearing_1 dist_2 bearing_2 ... dist_legs bearing_legs
```

`3 <= legs <= 8`; each distance is `1..2000000` mm; each bearing is
`0..359999` millidegrees. Leading and trailing separators are allowed.
Signs, malformed tokens, extra/missing tokens, out-of-range values and
overlong streams are rejected with exit 2. A well-formed traverse whose
misclosure grade fails `1:1000` reports normally and exits 3; accepted
traverses exit 0.

## Output

Signed fixed-decimal lines (six fractional digits, mm) for each leg's raw and
adjusted departure/latitude, then perimeter, misclosure components, precision
ratio and closed-figure bounding box. Print fields carry a frozen tolerance of
`+/-1` unit in the last digit (`print_tolerance_units`); the precision ratio
carries `+/-1` (`precision_num_tolerance`).

## Controls

`controls/intended_invalid/` writes one past the eight-slot bearing table and
must be rejected during checking with the pinned diagnostic in `expected.txt`,
before package admission or execution.

## Validation

`data/oracle.json` freezes 25 cases (inputs + expected stdout + exit codes)
produced by `data/reference.py`, an independent Python twin of the millidegree
trig and Bowditch math. `validate.py EXECUTABLE` compares a retained native
executable; `validate.py <omega-binary> --both` additionally requires the
exact engine-agreement stderr `native exit N` / `interp exit: N` per case —
the validator was repaired to match the CLI's current `interp exit:` marker
(historically it expected `AGREED`). Per-case
`twin_platform_divergence_mm` records the largest observed divergence between
the frozen twin and the realized platform output at freeze time.
