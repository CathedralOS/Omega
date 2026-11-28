# Bounded spatial diffusion / Poisson

See [scientific validation](../README.md) for the current status of this sample’s Rust execution driver.

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

The Build binds the same program on `linux_x86_64` and `macos_arm64`. It grants
the bundled system library standard input, standard output and process exit.
On an ARM64 Mac, run from the repository root with the sample's default input:

```sh
cd samples/cli/scientific/spatial_diffusion
omega packages update --project . --target macos_arm64
# Review the pending proposal and resume it after accepting its decisions.
omega packages update --project . --resume
omega run --target macos_arm64 build.omg < input.txt
omega --target macos_arm64 --build-dir build/native build.omg
./build/native/omega-program < input.txt
```

The [scientific capture checker](../README.md) checks recorded exit codes and
stderr for the 20 cases. For numerical validation, compare every cell with
`data/oracle.json`, reconstruct the residual from the printed field and check
the refinement ratios for the three quadratic grids.
