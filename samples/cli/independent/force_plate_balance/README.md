# Force plate balance (ADD01)

This complete offline Omega application reconstructs a vertical resultant from
four synthetic corner load cells. It reads frames from stdin, computes force
and first moments, reconciles them with independently supplied applied force
and moments, and tests a reduced rectangular support limit. All algorithms,
parsing, and decimal output live in this project. The only dependency is the
current stable bundled standard Console provider.

`sensor_min_max/main.omg` scans scalar readings for extrema and clamps a peak;
it has no positions, spatial moments, equilibrium reconciliation or support
geometry. A source search found no uncertainty or force reconstruction sample
on base `678203d30209af45e0446530582ea89914fd6180`. This workflow adds spatial
reconstruction and decisions rather than renamed extrema or uncertainty output.

Input is seven whitespace-separated decimal integers with an optional minus sign per line:

```
load0 load1 load2 load3 applied_force applied_Qx applied_Qy
50 150 150 50 400 40000 0
```

Loads are in N. Cell positions in mm are ordered counterclockwise:
`(-200,-100), (200,-100), (200,100), (-200,100)`.
Each input magnitude is at most 1,000,000; accepted cell loads are 0..1000 N.
The support rectangle is inclusive at x=±150 mm and y=±75 mm. Lines have a
256-byte limit including LF (at most 255 content bytes, including at EOF). Oversized lines are drained before the next
frame. EOF accepts a final unterminated frame. Invalid frames produce diagnostics
and processing continues; process exit 0 means the stream was processed, not
that all frames were valid. Host I/O failure follows Console's Trap contract.

`Qx=sum(load*x)` and `Qy=sum(load*y)` are **first moments**, not torque axis
names: standard torque components are `Mx=Qy`, `My=-Qx`. Output positions are
unreduced exact ratios `Qx/F,Qy/F`, so no rounding or floating tolerance is
needed. Residuals are computed force/moments minus supplied applied values.
Negative loads, saturation, zero resultant and nonzero equilibrium residuals
invalidate a frame. An outside-support result remains a valid reconstruction
with an unsafe support decision. Diagnostic priority is documented in
[data/ORACLE.md](data/ORACLE.md).

This is a rigid, planar, static vertical-force model. It excludes shear,
free torque, dynamics, sensor calibration, noise, and compliance. A reduced
rectangle is an authored support policy rather than a physical tipping proof.
Synthetic results are research fixtures, not equipment or safety certification.

From the repository root, set `OMEGA_LIBRARY_ROOT` to this checkout's
`source/library` and `OMEGA` to a binary built from the candidate compiler source recorded in the review bundle. Prepare
std's entry contract with `"$OMEGA" update --project
samples/cli/independent/force_plate_balance --target linux_x86_64`, review the
proposal and `--resume`. Then:

```sh
"$OMEGA" --check --target linux_x86_64 samples/cli/independent/force_plate_balance/main.omg
"$OMEGA" run --keep samples/cli/independent/force_plate_balance/main.omg < samples/cli/independent/force_plate_balance/data/frames.txt > /tmp/force-plate.stdout
cmp /tmp/force-plate.stdout samples/cli/independent/force_plate_balance/data/expected_stdout.txt
"$OMEGA" run --both --keep samples/cli/independent/force_plate_balance/main.omg < samples/cli/independent/force_plate_balance/data/frames.txt
python3 samples/cli/independent/force_plate_balance/data/reference.py
```

Repeat native and Terminal comparison with `data/variants.txt`, checking
`data/variants_stdout.txt`. The frozen fixtures include asymmetric loads,
mirroring, inclusive boundaries, fractional positions, one-cell support,
maximum loads, seeded varied frames, all runtime diagnostic classes, oversized
input recovery, and final unterminated input. Exact tolerance is zero.

The separate complete `intended_invalid/reject.omg` project deliberately divides an
unconstrained moment by an unconstrained force. Its `--check` must reject the
unproved divisor obligation; it is not an invalid runtime-input test. Check it
separately and verify the rejection diagnostic rather than counting any failure.

See `VALIDATION.md` for exact binary/source identities, commands and results.
Generated compiler/native artifacts belong in ignored `build/`. Review bundles
include observed logs and hashes separately from candidate source.
