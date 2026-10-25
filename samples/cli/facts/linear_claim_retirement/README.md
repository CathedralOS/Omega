# Linear claim retirement

A linear Token [linear] carries its exact payload across the place and retires through its explicit consuming machine on every normal exit.

## Owning requirement and capability

- **Parent task**: `STATIC-MACHINE-MIGRATION` (claim ticket: `339ebefd000346a4915c9689f8350b11`)
- **Requirement**: Explicit linear claim transport and terminal retirement through static consuming machines across all nine language transport places, ensuring exact-once consumption without implicit discard or duplicate consumption.

## The nine transport places

| Place | Positive witness | Reject twin | Transport |
| --- | --- | --- | --- |
| `root` | [main.omg](main.omg) | — | Direct linear `Token` construction and retirement without intermediate transport. |
| `local` | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | A local binding carries the linear `Token` before retirement. |
| `field` | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | An enclosing record field carries the linear `Token` before retirement. |
| `parameter` | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | A parameter carries the linear `Token` into a consuming machine. |
| `call_result` | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | A call result yields the linear `Token` before retirement. |
| `match_join` | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | Both branches yield the linear `Token` into retirement. |
| `const_name` | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | A named constant `const EXPECTED: u64 = 42` supplies the payload carried by the linear `Token` before retirement. |
| `other_state` | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | A successor machine receives the linear `Token` before retirement. |
| `other_machine` | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | A separate forwarding machine carries the linear `Token` before retirement. |
| `target_body` | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | Each target body retires the linear `Token`. |

## Execution and validation

Each place is an application that exits 0 with native and Terminal execution agreement:

```sh
omega run --both samples/cli/facts/linear_claim_retirement/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/local/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/field/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/parameter/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/call_result/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/match_join/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/const_name/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/other_state/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/other_machine/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/target_body/main.omg
```

Each place has an `invalid/` twin that breaks only the linear obligation by consuming the transported `Token` a second time after retirement. The control must reject with `after ... was transferred or consumed`. Check a twin with:

```sh
omega --check samples/cli/facts/linear_claim_retirement/<place>/invalid/main.omg
```

## Requirements and Evidence

Owning spec: [ownership and borrowing](../../../../wiki/spec/language/ownership.md) and [nominal cleanup](../../../../wiki/spec/terminal-psi/ownership.md#nominal-cleanup).
This row proves transport and explicit retirement of linear claims across all carrier places under static machine semantics.

Full 8-layer specification and lowering evidence:

- **Establishment:** Explicit construction of `Token { value: 42 }` (or `EXPECTED` in `const_name`) establishes the linear claim and obligation. Linear resources must be deterministically retired before normal exit without relying on any post-hoc caller equality guard (`transition x == 100`).
- **Identity and lifetime:** Linear values track exact unique storage and ownership identity; consuming the `Token` transfers it to `retire`, which consumes it via explicit `drop(token)` and invalidates the source place, preventing duplicate use or implicit disposal.
- **Composition:** The linear claim and retirement obligation compose across all nine standard places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`). Across calls, record fields, function parameters, match branches, successor machines, forwarding machines, and target-scoped bodies, linear conservation holds. In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in attempting duplicate consumption after retirement.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time (`--check`), soundly refusing duplicate consumption with a precise diagnostic naming the consumed place (`after `place_token` was transferred or consumed`, `after `holder.item` was transferred or consumed`, etc.).
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source linear constructs and explicit retirement lower without loss into canonical target-neutral Terminal Psi (`PSITERM\0`). The retired claim lowers into an explicit `RetireHome` / `InvokeNominal` affine cleanup action with no implicit discard, and executes identically in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Record layout geometry evaluates `Token` (one-field `u64` record, 8 bytes, alignment 8), and `CallPlan`/`StatePlan` lower the scalar carrier by value across register argument/return slots and spill slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 64-bit scalar moves and nominal drop calls, assigns register homes and disjoint stack-frame activation storage for live-across-call values, and inserts entry prologues and multi-return restoration epilogues honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve, basic blocks and branch offsets relax with offset reflow after prologue/epilogue insertion, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across host targets including `windows_x86_64` and `macos_arm64`.

## Discrimination and Acceptance

- **Discrimination**: The oracle is not vacuous. Changing the retired claim's own payload in `local/main.omg` from `Token { value: 42 }` to `Token { value: 41 }` survives checking and reaches runtime, where both engines observe the failure: native reports `signal 5` (SIGTRAP) and the Terminal Psi interpreter reports `crash: Trap (host presentation agrees)`. This is the corruption the instruction asks for -- the claim's own value, not a caller guard. Corrupting the retire operation's input instead (`retire(place_token) + 1`) is refused earlier by the static arithmetic proof obligation, so it cannot serve as a runtime discriminator.
- **Acceptance status**: This completed 9-place fact row proves transport and explicit retirement of linear claims across all carrier places under static machine semantics. Remaining open in `STATIC-MACHINE-MIGRATION` are the legacy `Binding<R>` receiver carrier removals, memory canaries, and static provider slot demands.

## Recorded evidence

Every positive place ran native and Terminal Psi on base `cc2f948d148e` (rebased onto `f3ea3cca357` before landing) with `native exit 0` and `interp exit: 0`:

```sh
OMEGA_LIBRARY_ROOT=<worktree>/source/library omega run --both samples/cli/facts/linear_claim_retirement/<place>/main.omg
```

- `python3 tools/sample_native_gate.py --facts --filter linear_claim_retirement`: 10/10 samples meet their documented oracle on `macos_arm64` and `windows_x86_64`, and 9/9 intended-invalid controls match their authored rejection.
- `python3 tools/corpus_gate.py --native --filter facts/linear_claim_retirement`: clean, 9 fixtures `{'built': 9}`, executed 9 with exit codes `{'0': 9}`, 9 rows stamped.
- `python3 tools/corpus_gate.py --filter facts/linear_claim_retirement`: 0 structural moves, 18 fixtures `{'checked': 9, 'rejected': 9}`.

The row is pinned by `tests/omega/{pass,fail}/facts/linear_claim_retirement_through_*` fixtures. Each of the positives carries `expected_native_exit.txt` reading `0`.
