# NF15 Householder QR and least squares checkpoint

These are two complete independent mathematical fixture projects and a separate
invalid-source ownership control. They are preparatory coverage: the Omega mains
check authored algebraic witnesses, and the Python driver checks an independent
Householder model. They do not implement or qualify the reusable Omega QR API.
The owned library source is withheld pending the shared dependency signatures in
`source/library/numerics/linalg/householder_qr_least_squares.api.txt`.

From the repository root, run the frozen mathematical qualification:

```sh
python samples/cli/scientific/w100/nf15_householder_qr_least_squares/reference.py all
python samples/cli/scientific/w100/nf15_householder_qr_least_squares/tall_line_fit/run_reference.py
python samples/cli/scientific/w100/nf15_householder_qr_least_squares/square_calibration/run_reference.py
sha256sum --check samples/cli/scientific/w100/nf15_householder_qr_least_squares/evidence/oracle-freeze.sha256
```

Python needs only the standard library. Each project has its own `build.omg`,
`main.omg`, and frozen `oracle.json`; expected native exit is zero with no input.
The invalid project has its own build and complete source plus an intended
borrow-diagnostic fragment. All project dependencies resolve to existing std.

Oracles and tolerances were authored and hashed before the reference driver ran.
No result-derived expected values or tolerance widening was used. Each primary
oracle specifies `1e-12` absolute solution/residual tolerances, `1e-12` Frobenius
orthogonality/reconstruction tolerances, and rank cutoff `1e-12 * max_column_norm`
with absolute rank cutoff zero. Rank-table solutions use the same `1e-12`
absolute tolerance and rank cutoff. Exact Fraction calculations independently
establish the two solutions, residuals, stationary condition and uniqueness.
The reference model uses normal equations only for that exact witness; its solve
uses Householder reflectors and back substitution.

Tall and square are different applications, not dimension changes of one source.
The tall project is a noisy intercept/slope fit with a nonzero residual. The
square project solves a calibration system and checks the first reflector's sign.
Their separate READMEs specify the complete integration calls and buffer design.

`rank_oracles.json` also freezes three unsupported rank cases (dependent columns,
a zero first column followed by a nonzero column, and the zero table), a negative
leading coordinate, a 1e150-scaled system, and padded column-major backing. The
zero-first-column case must never claim that the matrix has rank zero solely
because the first unpivoted pivot is zero. Its true rank is one; the reported
independent prefix is zero. Unsupported means no solution is returned.

Once the dependency APIs exist, implement the owned library, replace the
oracle-only Omega mains with reusable API calls while retaining all frozen
expected values, and qualify each route separately. If package admission requests
review, inspect and accept the specific decisions before resuming the project.
From the repository root, substituting each project path for PROJECT:

```sh
omega --check PROJECT/main.omg
omega run --target linux_x86_64 PROJECT/main.omg
omega inspect-terminal --machine Main::main --target linux_x86_64 PROJECT/main.omg
```

For the invalid control, all three routes must reject because the live reflector
view aliases factor backing being mutated. A missing executable, parse error,
provider/admission failure, unrelated bound failure, or accepted nonzero exit
is not the intended rejection. Preserve full stdout/stderr and exact commands.

`evidence/reference-results.json` records the successful host mathematical run.
`evidence/route-attempts.json` records each unavailable tool invocation separately:
the initial shell had not activated Cargo/Omega. After sourcing the provisioned
activation script, all routes hit cache-custody rejection before source validation;
`evidence/activated-route-attempts.json` preserves those failures. The current
revision Rust release build passed in 13m49s. All 9 retries using the new binary
hit the same custody failure; see `evidence/current-route-attempts.json` and
`evidence/rust-build-result.json`. No Omega route passed, and no intended source
rejection was observed. `evidence/checkpoint.md` records the base,
resources, ownership frontier and missing interfaces.
