# NF14 Cholesky SPD solve

This candidate contains two distinct complete mathematical sample projects,
one complete invalid-source control, exact independent oracles, and the owned
scalar pivot kernel. The descriptor-based public factor/solve API requires
NF01/NF02/NF11/NF12 interfaces, absent on the recorded base. It is not qualified
as a finished reusable SPD solver. No replacement shared package metadata,
rank-two descriptor, allocation, accuracy policy, copy or triangular solver is
introduced. Exact dependency signature requests and the future public contract
are in `source/library/numerics/linalg/cholesky_spd_solve.api.txt`.

## Sample designs

- `diagonal_readonly`: A=diag(4,9,16), readonly borrowed input and distinct factor
  backing; scalar diagonal factorization, two diagonal solves, x and residual
  oracles, preserved input.
- `correlated_copied`: dense off-diagonal SPD input, explicit element copy into
  work backing, successive Schur pivots, lower solve then transpose lower solve,
  factor/solution/residual/input-preservation oracles. Private two-by-two runtime
  controls distinguish nonsymmetry, negative pivot and zero pivot.
- `invalid_readonly_write`: intended static rejection is writing through
  `&[f64;9]`. The project declares std and hosted roots like the valid samples.
  Its expected diagnostic is an intended requirement, not a passing observation.

All matrices use column-major offsets. Arrays and borrowed fixed arrays make
the fixture backing explicit while the NF01 rank-two descriptor is unavailable.
These mathematical examples are pressure programs, not substitutes for shared
interfaces. They do not yet call the scalar library helper through the pending
numerics package. There are no hidden allocations or automatic copies.

## Frozen numerical oracles

`oracles.json` was committed in `0f6ccf05b5` before Omega execution. The tolerances
are absolute 1e-12 and relative 1e-12 for factors/solutions, absolute 1e-12 for
infinity-norm residual, and exact symmetry for these exact fixtures. The sample
checks use equality, stronger than these limits for their representable integer
intermediates. This does not define NF02's general numerical policy.

```sh
python3 samples/cli/scientific/w100/nf14_cholesky_spd_solve/verify_oracles.py
```

The independent rational verifier confirms A=L*L^T, Ly=b, L^T x=y and Ax=b
without implementing or running the proposed Cholesky/solve algorithm.
Nonsquare, nonsymmetric, nonpositive pivot and nonfinite-input intended outcomes
are frozen as well. Descriptor-shape, nonfinite intermediate, general stride,
aliasing and shared solver outcome integration remain dependent work.

## Separate qualification routes

From the repository root after activating the host Rust/mbx environment:

```sh
CARGO_TARGET_DIR=/workspace/Omega/target mbx build --release -p omega
python3 samples/cli/scientific/w100/nf14_cholesky_spd_solve/qualify.py --omega /workspace/Omega/target/release/omega
```

The recorded executor used `. /workspace/cloud-setup/activate.sh`. This is host
configuration, not a project dependency. On other hosts supply the matching
compiled Omega path; the Python runner is portable. It runs a single Omega
process at a time, and keeps all stdout/stderr plus exact command vectors in
`evidence/`. Native means `omega run`; Terminal comparison means
`omega run --both` with finite empty stdin. They are separately recorded.
`--both` success requires native/Terminal termination agreement, not independent
stdout equality or proof of all numerical properties. Each sample's native
exit 0 is the authored oracle; any other status needs diagnostic attribution.

For one project substitute its directory below:

```sh
omega --check samples/cli/scientific/w100/nf14_cholesky_spd_solve/diagonal_readonly/main.omg
omega run samples/cli/scientific/w100/nf14_cholesky_spd_solve/diagonal_readonly/main.omg
omega run --both samples/cli/scientific/w100/nf14_cholesky_spd_solve/diagonal_readonly/main.omg
omega --check samples/cli/scientific/w100/nf14_cholesky_spd_solve/invalid_readonly_write/main.omg
omega --check source/library/numerics/linalg/cholesky_spd_solve.omg
```

A cache-custody failure before source validation is environmental. It cannot
count as an invalid-source rejection or a source/compiler diagnosis. Do not
change ancestor ownership, permissions, or custody checks to manufacture a
result. Package review/entry refusal also does not count as readonly rejection.
The owning parent will review candidates and use normal landing.
