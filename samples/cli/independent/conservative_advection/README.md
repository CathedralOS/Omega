# Periodic conservative advection

Bounded stdin contains whitespace-separated `cells steps velocity spacing timestep`
followed by exactly `cells` concentrations. Cells must be integers from 4 through
64, steps integers from 0 through 256, velocity signed and finite, spacing and
timestep finite and positive, and concentrations finite and nonnegative.
Decimal tokens accept optional signs, decimal points and `e`/`E` exponents;
the finite request is bounded to 4096 bytes, 69 tokens, 18 mantissa digits per
token and exponent magnitude 400. EOF terminates the request. No preset replaces
user data. Within these lexical bounds, missing/extra data and invalid physical
controls exit 65; malformed, empty or oversized lexical input exits 64. The 69-token
lexical cap takes precedence over the physical data-count check. `|velocity*timestep/spacing|>1` rejects.

For positive velocity, each new cell is `(1-CFL)*cell+CFL*left`; for negative
velocity it is `(1-|CFL|)*cell+|CFL|*right`. Neighbors wrap over the accepted cell
count. Separate 64-cell buffers prevent one time level contaminating another.
Concentrations evolve as per-cell mantissas and decimal exponents; only a cell
and its upstream neighbor are aligned for each convex update. This permits a
minimum-subnormal impulse to split without disappearing. Zero-transport and
exact-shift cases retain original decimal coefficients, preserving mixed scales
and all accepted mantissa digits. An exact decimal product comparison determines
CFL stability. The complementary local weight is computed from exact decimal
digit subtraction before conversion, so an interior CFL rounded to one retains
its small local contribution. Only exact decimal CFL-one uses the shift path.
Authored negative values remain
rejectable even when conversion would otherwise produce negative zero.
Uniform convex weights preserve mass, nonnegativity and initial extrema.
Formatted decimals are compared with the exact original extrema. Endpoint
formatting corrects only a 1e-12 relative rounding envelope; larger deviations
refuse with 70. It does not alter a transport weight or the evolved buffers.

The final output includes MASS, MIN, MAX, signed CFL, then every CELL and
ADVECTED. Every numerical line contains an integer mantissa followed by a
base-ten exponent: `value = mantissa * 10^exponent`. Approximately sixteen significant digits describe computed values; exact
endpoints retain all original digits. Decimal exponents avoid a fixed output
magnitude limit. Mass is accumulated relative to
the maximum concentration with a separate decimal scale, so a mathematically
finite mass beyond the floating carrier's exponent range remains reportable.
Derived nonfinite numerical output exits 65 or 70 rather than claiming success.

`reference.py` freezes an independent analytic binomial convolution using
80-digit decimal arithmetic and exact binomial coefficients; it does not read
application source or execution output. `data/freeze.sha256` pins the generator,
full oracle and stdin matrix before execution. Tests include both velocity signs,
constants, impulses, sinusoids, CFL=1 exact shifts, zero velocity, zero steps,
endpoint dimensions/step count, scale extremes and intended invalid controls.
`validate.py` checks every field to 1e-9 absolute/relative, mass to 1e-10 of its
natural scale, positivity, no new extrema and numerical reports. It consumes
actual native processes or captured Terminal stdout, stderr and completion.
`--omega` uses the public CLI to compile once and execute all 48 frozen stdin
cases in both engines, checking each engine's actual numerical output and exit.
`controls/stencil_bound` must fail source checking: `index<=64` cannot justify
indexing 64 cells. Exit parity is never a numerical oracle.

From the repository root, using a compiler built from a recorded revision:

```sh
omega update --project samples/cli/independent/conservative_advection --target linux_x86_64
# Inspect the exact proposal, then accept/resume only the scoped normal decisions.
omega run --keep samples/cli/independent/conservative_advection/main.omg < samples/cli/independent/conservative_advection/input.txt
python3 samples/cli/independent/conservative_advection/validate.py --native-binary /absolute/path/to/retained/executable
python3 samples/cli/independent/conservative_advection/validate.py --omega /absolute/path/to/omega --output /absolute/batch/capture/directory
CARGO_PROFILE_RELEASE_OPT_LEVEL=1 mbx run --release -p advection-terminal-validation -- /absolute/sample/directory /absolute/capture/directory
python3 samples/cli/independent/conservative_advection/validate.py --captured /absolute/capture/directory
omega --check samples/cli/independent/conservative_advection/controls/stencil_bound/main.omg < /dev/null
```

The observation harness has explicit `--review` and `--resume-review` modes for
its own executable identity. Its ordinary mode requires the normal accepted
lock and uses the native report's retained Terminal artifact and admitted
provisions. Each request has fresh finite stdin and an isolated containing
domain. It does not disable custody or authority checks or supply fixture trust.
`Build::bind` explicitly selects `Main::main`. Its ordinary mutable `Main` context is ZII-valid and contains no service routing fields. I/O helpers call `Console::operation(arguments)` through the exact static Console slot; published helpers retain explicit Console invocation ceilings. Provider selection and normal package/runtime admission remain required. The sample requests std Console and no filesystem or subprocess authority.
Source-risk disclosure never grants host access.

The original 39 cases remain frozen. Three additional independent convolution
probes pin 18-digit constant fields for both signs and a narrow 18-digit interval;
`data/supplemental_freeze.sha256` preserves their separate freeze.
Six further binomial cases cover both signs of an interior CFL extremely close
to one, including full 36-digit products. Their original preexecution freeze
records and inputs are pinned by `data/near_one_freeze.sha256`; the original
oracles and tolerances remain unchanged.

Validation, retained evidence and the owner-authorized migration scope are documented in `evidence/status.md`.
