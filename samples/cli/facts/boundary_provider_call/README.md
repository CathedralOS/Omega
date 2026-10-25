# Boundary provider call

A provider-settled boundary call whose contract establishes `result >= 100u64 && result <= 100u64` discharges exact subtraction `result - 100u64`.

The original root reproduction is preserved in [main.omg](main.omg) and [build.omg](build.omg).

## Places

The nine places carry the provider-settled boundary contract through each language construct,
accompanied by `<place>/invalid/` reject twins:

| Place | Positive Program | Invalid Reject Twin | Carrier / Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | provider-settled boundary call discharges subtraction directly |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | boundary call result bound to a local variable |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | boundary call result stored in and projected from record field |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | boundary call result passed as an explicit machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | boundary call result returned across machine call boundary |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | boundary call result preserved across match convergence join |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | boundary call result subtracted against named constant |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | boundary call result passed as an argument across state machine transition |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | boundary call result forwarded through another machine |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | boundary call consumed across target-scoped machine bodies |

Run each place with `omega run <place>/main.omg`.

## Pinned Fixtures

The row is pinned by `tests/omega/{pass,fail}/facts/boundary_provider_call_through_*`
fixtures: nine positives, each with its own `expected_native_exit.txt`, and nine
reject twins with an authored `expected.txt`. Every positive place and the root
bind `ProgramEntry` for all four hosted targets, so this construct is not
target-restricted and the golden row for it belongs on each host's own
`corpus_native_<target>.txt`; the host recorded here is
`tests/omega/corpus_native_macos_arm64.txt`.

The fact's obligation class is a **value/arithmetic bound**: the boundary
contract's interval `[100, 100]` must be strong enough to discharge exact
subtraction `result - 100u64`. Discriminating that class measured:

- A contract admitting a RANGE (`result >= 99u64 && result <= 101u64`) still
  refuses statically — `exact arithmetic in machine run state entry terminal
  expression may overflow u64` — so weakening the interval never reaches
  runtime. The subtraction obligation is proved, not executed.
- A `requires`-bounded caller parameter carrying `99u64` likewise refuses at
  check time, as does a construction fact built from a `99u64`-bounded value.
  All nine reject twins therefore refuse at the check/construction gate.
- A runtime trap is reachable only by opting into a defined-overflow domain,
  which removes the static obligation rather than satisfying the contract:
  `Provider::get() as u64 in Trapping` subtracted by `100u64` reaches
  `native signal 5` with `interp crash: Trap (host presentation agrees)`. Both
  engines agree, and the compiler warns that the trapping computation "ALWAYS
  overflows `u64` ... traps unconditionally at runtime".

## Requirements and Evidence

- **Parent outcome:** `PROVIDER-SELECTION` / `TOP-LEVEL-BOUNDARY-REQUIREMENTS` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place provider settlement and boundary contract enforcement, verifying that boundary calls `Provider::get()` resolve to concrete provider implementations via `Build::select_provider`, and their postcondition contracts statically discharge exact arithmetic proof obligations across all nine language places.

Full 8-layer specification and lowering evidence:

- **Establishment:** The boundary contract is introduced via `boundary trait Provider { machine get() -> u64 ensures result >= 100u64 && result <= 100u64; }` and satisfied by concrete implementation `Actual satisfies Provider`. Provider selection is explicitly settled at build time via `Build::select_provider<Provider, Actual>(builder)`. The settled boundary call `Provider::get()` directly establishes the postcondition interval `[100, 100]` without requiring any post-hoc caller equality guard (`transition diff == 0u64` verifies the exact subtraction result, rather than manufacturing the premise).
- **Identity and lifetime:** The established postcondition interval `[100, 100]` attaches to the value produced by the settled boundary call `Provider::get()`. The `u64` scalar value retains its identity and range invariants across local variable binding, field storage/projection, parameter passing, machine call returns, match join convergence, constant arithmetic, state transitions, machine forwarding, and target-scoped execution bodies.
- **Composition:** The boundary call result composes uniformly across all 9 standard places to discharge exact subtraction `result - 100u64`:
  - `local`: boundary call result is bound to a local variable before subtracting 100u64.
  - `field`: boundary call result is stored inside record field `Carrier.result` and projected for subtraction.
  - `parameter`: boundary call result is passed to machine parameter `compute(result: u64)` with bounded contract.
  - `call_result`: boundary call result is returned from helper machine `acquire()` and subtracted.
  - `match_join`: boundary call results from divergent match branches converge to a unified interval.
  - `const_name`: boundary call result is subtracted against a named constant `const SUBTRAHEND: u64 = 100u64`.
  - `other_state`: boundary call result is passed as an argument to state `evaluate(val: u64)` across a transition edge.
  - `other_machine`: boundary call result is forwarded through helper machine `compute()`.
  - `target_body`: boundary calls and exact subtractions are executed across target-scoped bodies for all four hosted targets (`linux_x86_64`, `linux_arm64`, `windows_x86_64`, `macos_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in weakening the provider contract to `ensures result >= 99u64 && result <= 99u64`, proving that exact subtraction `result - 100u64` is strictly dependent on the established lower bound.
- **Soundness boundaries:** All nine reject twins under `<place>/invalid/` reject during compile-time verification (`omega --check`) with `error: exact arithmetic in machine ... may overflow `u64``, confirming that the arithmetic proof engine soundly detects possible underflow when the boundary contract guarantees only `result >= 99u64`.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower into canonical `PSITERM\0` target-neutral representations. Boundary application demands (`Provider::get`) lower into boundary call operations with explicit outcome contracts, and proof bounds are checked before mathematical terms are erased. Both Terminal Psi interpretation and native execution agree (`omega run --both` reports exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`, `ProviderPlan`) via `Build::select_provider<Provider, Actual>(builder)`. The provider binding is statically settled across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 64-bit scalar layout (8 bytes, alignment 8) and record layout, while `CallPlan`/`StatePlan` assign register and stack homes for boundary dispatch and return.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for direct function calls to the settled provider entry, 64-bit subtraction (`sub`), and equality comparisons. Backend register allocation assigns register homes and disjoint stack activation frames, inserting entry prologues and restoration epilogues honoring the `StatePlan` frame ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Settled provider call targets and symbolic stack displacements resolve into concrete relocations. Branch offsets relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images execute natively with Terminal Psi agreement (`omega run --both` reports `native exit 0` and `interp exit: 0`) across host targets.

Remaining open under PROVIDER-SELECTION: multi-provider dynamically selected dispatch, boundary wire-compatibility assertions, and parameterized trait provider bindings.
