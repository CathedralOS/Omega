# Nonlinear diode sweep (NEW031)

A complete console application solves the operating point of a voltage source,
positive series resistor, and Shockley diode. It emits CSV operating points,
currents, voltage residuals, bracket widths, iteration counts, evaluation counts,
and a summary. The application owns its exponential evaluator, scanner,
bisection algorithm, parser, and decimal reporter. It imports only settled core
float operations, bindings, and the existing standard Console provider.

## Scientific sample model

`domain/shockley.omg` declares the `Circuit` model. For source voltage `Vs`,
diode voltage `v`, resistance `R`, saturation current `Is`, and effective thermal
voltage `a = n Vt`, the equations are:

```text
I(v) = Is (exp(v/a) - 1)
F(v) = v + R I(v) - Vs
```

This is an ideal DC model with positive `R`, `Is`, and `a`. It excludes reverse
breakdown, temperature feedback, capacitance, and parasitic series resistance.
The result is a sample calculation, not a calibrated physical-device prediction.
`F` is strictly increasing, so a sign bracket contains at most one root. A root
outside the user-selected voltage interval is `no_bracket`, even if the full
unbounded ideal equation has a root. No guessed operating point is emitted for
that status. A bounded bisection that exhausts its budget is `iteration_limit`
and reports its final approximation rather than calling it converged.

The private exponential divides its argument by 128, evaluates 32 Taylor terms,
then squares seven times. Validated model bounds give arguments between -48
and 36. The reference independently uses `Decimal.exp`, not this expansion.

## Input

One record on stdin contains nine decimal integers with optional minus signs, separated by spaces
or tabs, terminated by LF or EOF. CR is accepted as whitespace for CRLF input.
The parser reads at most 160 bytes, rejects malformed/extra/missing fields and
decimal overflow, and stores exactly nine bounded values.

| Field | Unit | Accepted values |
| --- | --- | --- |
| source start | mV | -1000..5000 |
| source stop | mV | -1000..5000 |
| point count | count | 2..16 |
| resistance | ohm | 10..1000000 |
| saturation current | pA | 1..1000000 |
| effective thermal voltage | mV | 25..50 |
| bracket lower | mV | -1200..900 |
| bracket upper | mV | -1200..900, strictly above lower |
| iteration budget per point | count | 1..80 |

Ascending, descending, and constant-source sweeps are supported. For example:

```text
-1000 1200 12 1000 1000 26 -1200 900 60
```

Syntax errors exit 64; model/sweep range errors exit 65. A valid completed sweep
exits 0, including a sweep containing `no_bracket` or `iteration_limit` rows.
There is no pause or hidden default input. The application consumes one record.

## Output and convergence

The header names SI units. Numeric fields use signed integer mantissas and
powers of ten with nine significant decimal digits, for example `260000000e-9`
means 0.26 V. `NA` denotes a missing operating point. Iteration and evaluation
counts are emitted as decimal integers. The final `summary,converged,failed`
row accounts for every point.

The scanner tests 64 equal cells including both interval endpoints. Exact-zero
endpoints complete without bisection. Bisection stops when `|F| <= 1e-10 V` or
the pre-step interval width is at most `1e-12 V`, subject to the input budget.
Each emitted width is the nominal interval from which that row's midpoint was
selected, carried through binary halvings to avoid endpoint-subtraction cancellation.
Evaluation counts include scan and bisection residual evaluations, excluding
the extra endpoint current reporting call.

## Independent acceptance

`validation/oracle.json` was frozen before the first Omega check/run. The
authoring script refuses to overwrite it. It uses 70-digit Decimal exponentials,
a separate 4096-cell bracket scan, and 240 refinement steps. The frozen file
contains fixture hashes, expected statuses and root/current values, independently
calculated budget diagnostics, and tolerances. The principal tolerances are
2e-9 V absolute plus 2e-8 relative for voltage; 5e-12 A absolute plus 3e-8
relative for current; reported converged residual at most 1e-9 V. Diagnostic
counts permit two iterations of floating-point stopping variation while checking
the exact evaluation-count identity and width for the emitted iteration.

The fixtures exercise reverse bias and forward conduction across zero, steep
forward response, large resistance/saturation current, descending sweeps,
unbracketed configurations, a one-iteration budget, exact-zero endpoints,
CRLF, EOF termination, and twelve invalid runtime records.

The complete source in `controls/intended_invalid/main.omg` reads index 2 of a
two-element array. Acceptance requires rejection for that bounds violation.
Package-review, entry-slot, parse, or unrelated type errors do not pass this
control. It has no package dependency and is checked separately.

## Commands

Run from the repository root. Use the verified `omega` binary recorded in
`validation/EVIDENCE.md`, or build one with `mbx build --locked -p omega --release
-j 4` (`cargo` if mbx is unavailable). Do not run simultaneous compiler or Rust
build processes on one executor.

```sh
sample=samples/cli/independent/nonlinear_diode_sweep
omega update --project "$sample" --target linux_x86_64
# Read the resulting proposal, decide its exact review rows, then:
omega update --resume --project "$sample"
omega --check --target linux_x86_64 "$sample/main.omg"
omega run --keep "$sample/main.omg" < "$sample/data/mixed_bias.stdin"
omega run --both --keep "$sample/main.omg" < "$sample/data/mixed_bias.stdin"
omega --check "$sample/controls/intended_invalid/main.omg"
```

Package acceptance is a distinct review operation. A denied acceptance is
recorded without retry or bypass. See `validation/EVIDENCE.md` for the actual
commands, routes, exits, binary/base identities, and artifacts from this run.

`validation/run.py` executes the routes serially, captures stdout/stderr and
return codes under local `build/validation`, and invokes the frozen output checker.
It does not accept packages. The same Python 3 entrypoint works on Linux, macOS,
and Windows with the corresponding available target. The application binds four
host targets; validation of this candidate is limited to the actual recorded host.

The CLI `--both` route reports exit agreement and exposes native stdout. For full
Terminal CSV validation on Linux, the private `validation/terminal-driver` uses
the normal accepted-project API, retains its exact native report's Terminal
artifact and provisions, and executes a fresh instance for each fixture. Its
explicit `--review` and `--resume-review` modes run the ordinary package proposal
workflow for that executable's compiler identity; neither mode edits decisions.
The run mode requires an accepted lock and cannot bypass identity or custody.

```sh
cargo build --locked --manifest-path "$sample/validation/terminal-driver/Cargo.toml" -j 4
observer="$sample/validation/terminal-driver/target/debug/new031-terminal-validation"
"$observer" --review "$sample"
# If the ordinary proposal requires decisions, review them before this command:
"$observer" --resume-review "$sample"
"$observer" "$sample" "$sample/build/validation/terminal-api"
PYTHONDONTWRITEBYTECODE=1 python3 "$sample/validation/check_terminal.py"
```

## Existing sample comparison

The compared `arithmetic/dual_float` sample checks basic mixed-width arithmetic;
`simulation/particle_sim` advances a fixed Euler state; `probes/float_ops_surface`
tests operator resolution; `text/parse_number`, `text/format_number`, and
`text/hex_dump` exercise tiny formatting/byte cases. None solves a nonlinear
circuit, scans root intervals, owns an exponential approximation, accepts a
parameterized scientific sweep, or reports convergence diagnostics. This sample
adds that workflow without shared library or export changes.
