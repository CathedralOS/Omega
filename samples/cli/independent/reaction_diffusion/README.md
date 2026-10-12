# Reaction diffusion (IND10, reconstructed)

A complete bounded reaction-diffusion solver in Omega: the Fisher-KPP
`du/dt = D·u_xx + r·u·(1 − u/K)` explicit Euler update on a one-dimensional
cell grid, with concentrations and coefficients in decimal thousandths. This
is a declared synthetic model, not an empirical prediction. No external
scientific library or worker candidate is imported.

Provenance: the original IND10 `reaction_diffusion` source, its frozen Decimal
matrix oracles and its accepted package lock were preserved only on the Codex
Library, whose download route returned HTTP 401, and its candidate commit
`530c93cdae25311a2db28c854eaa5648f332b5c4` is not reachable from this remote.
This project is an independent reconstruction of the stated acceptance —
reaction-inclusive mass balance, a separate continuum error column, and
reflecting/periodic boundary cases — using repository spec contracts and a new
independent integer oracle. It makes no claim of byte identity or of inheriting
the original review; its fixtures, oracle, controls and review are new.

Input is 6 + N signed decimal fields, one per LF-delimited line, ending at EOF:
cell count `N` in [3,32]; step count in [0,64]; boundary mode (0 reflecting,
1 periodic); Courant number `lam = D·dt/dx²` in thousandths [0,500]; logistic
rate `r` in thousandths [0,1000]; carrying capacity `K` in thousandths
[1,1000000]; then N initial cell concentrations in thousandths [0,1000000].
At most ten decimal digits per field are accepted. Empty, malformed, missing,
short, overlong and trailing inputs exit 2 before any report is printed.

Each step applies a Jacobi update `u' = u + lam·laplacian/1000 + r·u·(K−u)/(1000·K)`
with truncating integer division. Under a reflecting boundary a missing
neighbor reads the boundary cell itself (zero-flux Neumann); under periodic the
grid wraps. The per-cell applied diffusion and reaction terms are summed into a
ledger: every emitted row attests `mass = previous mass + reaction + diffusion`,
the reaction-inclusive mass balance. A shadow trajectory integrates the same
operator in two half steps; each row's `continuum_error` is the largest cell
disagreement between the full step and the half-step pair — a measurable
discretization defect, reported separately from the mass ledger.

Output is stdout CSV: header `step,mass,reaction,diffusion,continuum_error`,
one row per completed step starting at 0, then a final `field,` row of the N
concentrations. A cell leaving [0,1000000] or a ledger disagreement prints
`ERROR infeasible state` and exits 3 after the completed rows. Valid runs exit
0. The ordinary sample gate supplies empty stdin, so its annotations require
the missing-input rejection (exit 2); the 25-case validator exercises runs.

`reaction/model.omg` owns the private update, neighbor and ledger rules;
`reaction/oracles.omg` checks nine transitions from three pre-frozen mini
grids at startup; `reaction/input.omg` owns the bounded streaming parser;
`reaction/report.omg` owns signed decimal CSV reporting. `main.omg` reads the
input and drives the simulation. `build.omg` binds the entry to the documented
Linux x86-64 program slot and uses the existing std Console provider.

## Frozen acceptance

`reference.py` independently derives every row with Python integers and exact
truncating division. It freezes expected results before any Omega execution.
`fixtures/oracle.json` records every input and output SHA-256 and required
exit. Numerical tolerance is exactly zero. `validate.py` pins the frozen
manifest/reference identities and all 25 cases, then verifies every fixture
before execution. It compares every stdout byte and exit status, requiring
empty native stderr or the exact documented CLI exit diagnostics for Terminal
comparison; it never rewrites expectations.
Twenty cases cover reflecting and periodic pulses, logistic growth, Fisher
transport under both boundaries, zero steps, the three-cell and 32-cell bounds,
the Courant stability edge, extinction, saturation, decay from above capacity,
the infeasible trajectory, every invalid header field, negative and overflowing
concentrations, and a short field; five malformed-input cases cover
malformed/missing/empty/overlong/trailing input.

The complete separate control `controls/invalid_mode_type` violates the
documented integer-versus-boolean type rule by assigning `true` to an `i64`
mode field. It must fail specifically for that mismatch. Package admission,
parse failure, provider failure or any other diagnostic is not a successful
negative control.

## Commands

From the repository root, use a compiler built from the recorded base revision
with its matching `source/library` (set `OMEGA_LIBRARY_ROOT` to that absolute
path). Here `omega` names that compiler and `S` is this sample directory:

```sh
S=samples/cli/independent/reaction_diffusion
omega update --project "$S" --target linux_x86_64
# Review the exact generated pending decisions before accepting and resuming.
omega --check "$S/main.omg"
omega --build-dir "$S/build" "$S/main.omg"
omega run --both --keep "$S/main.omg" < "$S/fixtures/reflecting_pulse.input"
python3 "$S/validate.py" /absolute/path/to/the/published/executable
python3 "$S/validate.py" /absolute/path/to/omega --both
omega --check "$S/controls/invalid_mode_type/invalid.omg"
```
