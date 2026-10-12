# NEW080 execution evidence

Completed in isolated worktree `/home/ubuntu/omega-z50` on fetched main
`64533f403d5839cf9f939616422136e5067ba043`. The original NEW080 packet
(`082650733f59`) is unrecoverable — the ref is absent on the remote and the
Library archive `battery-discharge-capacity-candidate-082650733f59.tar.gz`
materialization failed HTTP 401 before this run. This tree is a
**reconstruction**: the stated numerical/application behavior was rebuilt
from repository contracts (the `nonlinear_diode_sweep` project shape) plus an
independent Decimal mathematical oracle. No reconstructed file claims
original provenance and no historical review is claimed.

## Source and executor identities

- Fetched main base: `64533f403d5839cf9f939616422136e5067ba043`.
- Frozen oracle SHA-256:
  `964fae42675ca54cc9f59b18a03f28e39584e4f0cd55942ff7d635fea2da470c`,
  frozen from `validation/reference.py` (Decimal, 70-digit) before any Omega
  check or execution. Totals are recorded on the application's nine-digit
  truncated emission grid; row values stay Decimal and carry tolerances.
- Release CLI SHA-256:
  `f28ced2aed05f3106a4657b63a183e4347c40636061900ee63534eb3179cfc1d`
  (`mbx build --locked -p omega --release` at the worktree base).
- Private Terminal observer SHA-256:
  `06b73e470dd7d80326392b432f435159238a6b017b3ca020695e3bed57f7ed49`
  (`validation/terminal-driver`, cargo `--offline` build; retained outside
  the package source root to stay under the managed source byte limit).
- Linux native application SHA-256 (identical across all retained runs):
  `f91701139a7bd666222675ba336ac9030f2237fb350e3438bc35682e946a8779`.
- Captured canonical Terminal artifact SHA-256:
  `45295d48d218e24e0809129d71143f5cf6a80ac2296dd88e68f1e8367e2bbe02`.

Host: Linux x86_64, kernel 6.8.0-1061-aws, 8 CPUs.
Rust: `1.100.0-nightly (a69a63265 2026-09-03)`.
Validation ran with `PYTHONDONTWRITEBYTECODE=1`; interpreter bytecode caches
inside the package mutate the managed source identity.

## Outcomes

| Route | Observation |
| --- | --- |
| Package review | `omega update` produced 287 pending rows across `battery_discharge_capacity` and pinned `omega_language_std` (`b7e52c4f0323`): 3 `terminal_permission` (Console read/write/exit), 141 `callable`, 141 `external_supply` (Linux syscall bindings incl. cross-target declarations std ships), 2 `dangerous_capability` (std's filesystem/process classes). Each row inspected against the pinned local packages and accepted individually in the review document; `--resume` published `omega.lock`. No blanket accept. |
| Application check | `omega --check --target linux_x86_64` exit 0. |
| Native | All 21 input records passed the frozen Decimal checker: header, per-step OCV/terminal/current/delivered mAh/Wh rows, exact stop reason and emitted summary totals. Stops exercised: `cutoff`, `depleted`, `budget`, plus immediate cutoff at step 0. |
| CLI native/Terminal (`--both`) | Agreement for the short fixtures; `interp` DECLINED on the 4500-step light-load record (sponsor fuel), so the full-CSV route below carries Terminal evidence. |
| Full Terminal streams | Private observer executed the retained `application.terminal` per fixture: all 21 records passed the same frozen checker; every stdout byte-identical to native, stderr empty, exits 0/64/65 as authored. |
| Intended-invalid source | `omega --check controls/intended_invalid/main.omg` exit 1 with exactly ``cannot prove index `2` is within length 2``; no package-acceptance failure. |

Validation is scoped to the actual Linux host. macOS and Windows entry
bindings are declared but their runtime routes were unavailable here; no
cross-platform or full-corpus claim is made.

## Compiler contact points (no compiler edits)

No compiler source was modified. The authored app stayed inside the current
executable lowering surface:

- `use omega::language::core::float_operations;` is required in any module
  performing `f64` arithmetic; without it `let` initializers containing
  operators fail `statement sequence: local data: scalar local: pure
  initializer` (the initializer never becomes a scalar computation).
- `let` initializers in machine bodies/states accept direct field reads and
  expressions, while stores into `&mut` data fields accept plain locals;
  matching the `nonlinear_diode_sweep` spelling solved both.

## Reproduction

```sh
sample=samples/cli/independent/battery_discharge_capacity
omega update --project "$sample" --target linux_x86_64   # review rows, accept, then:
omega update --resume --project "$sample"
omega --check --target linux_x86_64 "$sample/main.omg"
PYTHONDONTWRITEBYTECODE=1 python3 "$sample/validation/run.py" --omega <cli> --route check
PYTHONDONTWRITEBYTECODE=1 python3 "$sample/validation/run.py" --omega <cli> --route native
# Full Terminal CSV: build validation/terminal-driver (cargo --offline,
# CARGO_TARGET_DIR outside the package), --review for its compiler identity,
# then run it against the sample, then:
PYTHONDONTWRITEBYTECODE=1 python3 "$sample/validation/check_terminal.py"
PYTHONDONTWRITEBYTECODE=1 python3 "$sample/validation/run.py" --omega <cli> --route invalid
```
