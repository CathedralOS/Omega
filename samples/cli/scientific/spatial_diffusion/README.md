# Bounded spatial diffusion / Poisson

This sample solves `-div(k grad u) + reaction*u = f` on the unit square.
A uniform cell-centred finite-volume mesh has at most 12 by 12 cells. Sparse
CSR assembly holds at most 720 entries and 145 row offsets. All four sides
have explicit boundary kinds: 0 is Dirichlet, 1 is an outward normal derivative.
The preset boundary values come from the selected manufactured solution.

Dirichlet faces use their half-cell distance and eliminate the prescribed
value into the right-hand side. Each interior face contributes a symmetric
edge difference. Neumann faces put `k*q/h` into the right-hand side. Supported
systems have finite positive diffusion, finite nonnegative reaction, four
valid sides, and at least one Dirichlet anchor. Their energy is positive, so
conjugate gradient is appropriate. Pure Neumann systems are rejected rather
than applying CG to a singular problem. Negative reaction is rejected rather
than assuming an indefinite system is SPD. No dense matrix enters the sample.

CG reports iteration count, status, the independently recomputed infinity
norm of `b-A*x`, its stopping threshold, and the manufactured maximum error.
The threshold is `1e-9 + relative_tolerance*||b||inf`, using `1e-10` by default.
A cap of 400 iterations and finite positive curvature checks bound execution.
Limit exhaustion produces the partial field and `NOT_CONVERGED`, exit 2;
it does not declare success. Numerical output is integer picounits, truncated
toward zero, with a trailing space after each number.

Pass exactly one input byte, with EOF and no newline:

| Byte | Problem / control |
| --- | --- |
| `0`, `1`, `2` | Quadratic `u=x^2+y^2`, Dirichlet, grids 4, 8, 12 |
| `3` | Linear `u=x+2y`, grid 8, west Dirichlet and three Neumann sides |
| `4` | Quadratic, grid 8, diffusion 2 and reaction 3 |
| `5`, `6`, `d`, `e` | Zero, negative, NaN and infinite diffusion |
| `7`, `c`, `8` | Unknown side kind, missing side and singular pure Neumann |
| `9`, `a` | One iteration and zero iterations: nonconvergence |
| `b`, `f` | Negative reaction and zero tolerance |

Valid cases exit 0. Invalid grid controls exit 65. Empty input, unknown bytes
and any trailing byte exit 64. The native sample gate feeds root `input.txt`
(the coarse-grid case); a direct run with empty stdin is a rejection.

The quadratic's second-order error is exactly `h^2/4` for zero reaction under
this face discretization: 0.015625, 0.00390625 and 0.001736111111 for the three
meshes. The mixed linear field is reproduced to solver tolerance. The independent
oracle builds a dense matrix by adding undirected edge energies and uses pivoted
Gaussian elimination. It cross-checks the analytic quadratic offset and linear
field before freezing its 20-case data; it never reads Omega output or source.
`data/freeze.sha256` pins that oracle. Do not regenerate it from observations.

Run from the repository root with a current compiler:

```sh
omega update --project samples/cli/independent/spatial_diffusion --target linux_x86_64
# Inspect the exact pending package proposal and source-only risks before accepting.
# Resume the reviewed exact proposal through the normal package workflow.
printf 0 | omega run samples/cli/independent/spatial_diffusion/main.omg
python3 samples/cli/independent/spatial_diffusion/validate.py --omega /absolute/path/to/omega
```

The build only declares an application, std path dependency, and Linux entry.
It requests no host filesystem or subprocess service and grants no additional
terminal permissions. Package risk classification is not authority acceptance.
Never bypass a custody or permission refusal in order to run this sample.

Terminal numerical validation is separate from CLI exit-code comparison:

```sh
CARGO_PROFILE_RELEASE_OPT_LEVEL=1 mbx run --release --locked -p sci04-terminal-validation -- --evidence /absolute/path/to/spatial_diffusion
python3 samples/cli/independent/spatial_diffusion/validate.py --captured samples/cli/independent/spatial_diffusion/build/validation/terminal
```

The Rust harness is a root workspace member sharing the CLI dependency resolution.
It uses the normal native report's retained Terminal artifact and
admitted provisions, gives each case an isolated domain with finite stdin, and
captures actual stdout, stderr and completion. Every capture is compared
against the frozen oracle's exit for that case and must leave stderr empty
before the numbers are checked. After a normal `omega run --keep` compile, `validate.py --native-binary` can
reuse that exact retained executable for the full input matrix. This avoids
recompiling identical Omega source for each request; every input still runs in
a fresh OS process and is checked against the frozen oracle.

The Python validator checks every
cell against the frozen dense oracle, checks reports, reconstructs residuals
with a bound for picounit truncation, and checks the refinement ratios. It does
not treat exit-code agreement as numerical evidence.

See `evidence/status.md` for producer identity, measured results and
qualification status.
