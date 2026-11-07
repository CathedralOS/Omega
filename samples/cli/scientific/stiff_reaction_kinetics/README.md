# Stiff reaction kinetics

A runtime-configured mass-action network with three species and three reactions.
The sample integrates supplied concentrations to a supplied observation time
using a second-order, two-stage SDIRK method, analytic Jacobians, damped Newton
solves and step doubling. Each stage reuses a backward-Euler residual.
Rejected trials retain the last committed time and concentrations. A successful
report includes concentrations, maximum mass drift, accepted and rejected steps.

This bounded application accepts integer reactant/product coefficients 0..2.
Each reaction must conserve the sum of concentrations (equal species weights).
A zero rate disables a reaction. This is an explicit model restriction: unequal
molecular weights and arbitrary larger networks require a different input contract.
No named fixture or reaction-specific formula appears in the integrator.

Input is exactly 40 unsigned decimal integers, each at most 10^12, separated by
ASCII whitespace. At most 4096 bytes are consumed. Rational quantities are
represented as numerator then positive denominator:

| Tokens (zero based) | Meaning |
| --- | --- |
| 0..5 | Three rate constants as rational pairs |
| 6..23 | For each reaction: three reactant coefficients, then three product coefficients |
| 24..29 | Three initial concentrations as rational pairs |
| 30..31 | Final time |
| 32..33 | Initial step size |
| 34..35 | Absolute tolerance |
| 36..37 | Relative tolerance |
| 38 | Trial budget, 1..1000000 |
| 39 | Newton iteration budget, 1..32 |

Time, step and tolerances must be positive. Initial total concentration and final time are at most 10^6, and
relative tolerance at most 1. Signs, decimal/exponent text, NaN, infinity,
partial records, extra tokens, mass-changing reactions and individual coefficients above two
are rejected. Every arithmetic conversion follows these bounded inputs.

Run from the repository root with a compiler built from the recorded source:

```sh
APP=samples/cli/independent/stiff_reaction_kinetics
omega update --project "$APP" --target linux_x86_64
omega run --both "$APP/main.omg" < "$APP/data/robertson_4_1_r100000.stdin"
```

Package review follows the normal editable review/resume workflow. Review
actual candidate source and capabilities; no blanket approval is required by
this application. An accepted lock is host-local and ignored.

Exit 0 means convergence, 2 means invalid input, and 3 means exhausted convergence.
EOF with no record is invalid. Valid output has three lines: a label, then
`time A B C maximum_mass_drift` in signed integer units of 10^-12, then
`accepted_steps rejected_steps`. Rounding/truncation resolution is 10^-12.

The Newton residual threshold is 0.01 of the component tolerance. Positivity
backtracking halves the Newton correction at most 12 times; it never clips or
renormalizes concentrations. The cofactor solve rejects singular/nonfinite
3x3 determinants. The SDIRK coefficient is gamma=1-1/sqrt(2); its tableau is
`A=[[gamma,0],[1-gamma,gamma]], b=[1-gamma,gamma]`. Each step-doubling trial
uses six Newton stage solves. The normalized full-versus-two-half-step difference
divided by three estimates the fine result's local error; the fine result is accepted
only at error at most one. The method is second order and L-stable; local tolerances
are not a guarantee of equal global error. The second-stage base must be finite
and nonnegative; rejection preserves positivity but can limit stiff step size.
No extrapolated concentration is clipped. Step growth doubles only below error 0.1;
rejection halves the step. Trial, Newton, damping and minimum-step bounds are
explicit convergence failures rather than successful partial trajectories. A trial
requires h > 10^-15(1+t) and t+h > t. Newton budgets count corrections;
the final corrected iterate still receives a residual check.

`reference.py` freezes SciPy Radau and BDF references at rtol=1e-11, atol=1e-14;
both linear chains additionally use exact exponential formulas. Frozen input
and oracle hashes live in `data/FROZEN.sha256`. The 38 cases include Robertson
at 0.4, 4 and 40, distinct-rate/equal-rate chains, three relative tolerances,
input refusals, both trial/Newton exhaustion, a stationary zero-rate network, and a
linear chain restricted to one Newton correction. `validate.py` checks actual
outputs, finite nonnegative concentrations, time, mass balance, numeric error,
and strict tolerance refinement for five model/time groups.

Native validation accepts the exact executable produced by normal compilation.
Direct Terminal validation links `validation/terminal_sdk.rs` against the exact
compiler build and executes its retained Terminal artifact with original runtime
provisions and fresh input for every fixture. The driver publishes that same retained native product through the canonical
product publisher; it does not repeat compilation for native qualification.
Its 100000000 fuel sponsor budget
is explicit. `run --both` compares exits only; it does not validate Terminal
numeric output. Use both validation routes:

```sh
python3 "$APP/validate.py" --executable /absolute/path/to/produced/executable
python3 "$APP/validate.py" --terminal-sponsor /absolute/path/to/terminal-sdk
# Reuse completed direct captures without recompilation:
python3 "$APP/validate.py" --terminal-sponsor /absolute/path/to/terminal-sdk --captures /absolute/path/to/captures
```

Generated products and capture reports live under excluded `build/`, so validation
does not change the package source identity.

The separate `controls/unguarded_index` source must reject on its authored
bounds diagnostic. Scientific qualification and source refusal are separate gates.

The [qualification record](../../../../wiki/drafts/measurements/stiff_reaction_kinetics_qualification.md)
records the observed producer, scientific comparisons, refusals and review.
