# Unsigned Division Remainder

Parent task: **CONSERVATION-CONTRACT** (`TASKS.md`).

## Governing requirement

Computing the remainder of a bounded unsigned `u64` dividend (`500u64`) divided by a bounded non-zero unsigned divisor (`200u64`) establishes the exact unsigned remainder bound (`remainder == 100u64`), discharging exact subtraction `rem - 100u64` with exit code `0`.

**Fact: an unsigned u64 remainder from 500u64 % 200u64 equals 100u64 when subtracting 100u64.**

Every place performs the same operation: compute the unsigned division remainder of dividend `500u64` by divisor `200u64`, transport the resulting `100u64` remainder, perform exact subtraction `rem - 100u64` yielding `0`, and return `0` as `i32` to transition to `done()` and exit `0`. The positive oracle asserts exit `0` under both native and Terminal Psi execution. Only the transport of the remainder changes across places.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| Root | [`main.omg`](main.omg) | — | Direct unsigned division remainder and exact subtraction check |
| Local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | The remainder is copied into a local binding before verifying exact subtraction |
| Field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | A record field carries the remainder into the subtraction check |
| Parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | The remainder is passed as an explicit machine parameter |
| Call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | A call returns the remainder into the subtraction check |
| Match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Both match arms produce the remainder and join into the subtraction check |
| Const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | A named constant carries the remainder into the subtraction check |
| A nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | A successor machine receives the remainder and performs the subtraction check |
| Another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | A forwarding machine forwards the remainder to the verifying machine |
| Another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Each of four target bodies computes the remainder and verifies exact subtraction |

Match the fact sentence beside every place: each positive place is pinned as `tests/omega/pass/facts/unsigned_division_remainder_through_<place>_exit` and each reject twin as `tests/omega/fail/facts/unsigned_division_remainder_through_<place>_rejected`, byte-equal to the place and `invalid/` programs beside them.

### Verification commands

Root `main.omg` and all nine place projects bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry `expected_native_exit.txt` (`0`). Run any positive directly:

```sh
omega run --both samples/cli/facts/unsigned_division_remainder/main.omg
omega run --both samples/cli/facts/unsigned_division_remainder/local/main.omg
omega run --both samples/cli/facts/unsigned_division_remainder/field/main.omg
omega run --both samples/cli/facts/unsigned_division_remainder/parameter/main.omg
omega run --both samples/cli/facts/unsigned_division_remainder/call_result/main.omg
omega run --both samples/cli/facts/unsigned_division_remainder/match_join/main.omg
omega run --both samples/cli/facts/unsigned_division_remainder/const_name/main.omg
omega run --both samples/cli/facts/unsigned_division_remainder/other_state/main.omg
omega run --both samples/cli/facts/unsigned_division_remainder/other_machine/main.omg
omega run --both samples/cli/facts/unsigned_division_remainder/target_body/main.omg
```

Each place has an `invalid/` twin that computes `499u64 % 200u64` (yielding remainder `99u64`), causing exact subtraction `diff: u64 = rem - 100u64` to underflow. The control must reject with `may overflow `u64``. Check each twin with:

```sh
omega --check samples/cli/facts/unsigned_division_remainder/<place>/invalid/main.omg
```

## Requirements and Evidence

Owning requirement: `wiki/spec/language/numeric_values.md#division-and-remainder` and `wiki/spec/terminal-psi/integer_certificates.md#canonical-scalar-goals`.
Parent task: **CONSERVATION-CONTRACT** (`TASKS.md`).
This row establishes that unsigned integer division remainder (`500u64 % 200u64`) produces an exact remainder (`100u64`) whose static bounds preserve across all nine places and discharge exact subtraction `rem - 100u64` without underflow.

- **Establishment:** Constructing and evaluating the unsigned remainder operation `500u64 % 200u64` satisfies `ExactDivisionDefined` with non-zero unsigned divisor `200u64 >= 1u64`, proving the remainder is representable and exact `100u64` from checking through proof admission without requiring any post-hoc caller equality guard (`transition x == 100`). Helper machines verify postcondition `ensures result >= 100 && result <= 100` or preconditions `requires rem >= 100 && rem <= 100`, which directly prove that exact subtraction `rem - 100u64` cannot underflow `u64`.
- **Identity and lifetime:** The unsigned `u64` remainder is a 64-bit scalar value type whose value and arithmetic bounds retain their exact identity across register and stack storage, surviving local copies, record encapsulation (`RemResult`), parameter passing, call returns, match joins, named constants, state transitions, machine forwarding, and target-scoped bodies without aliasing or storage mutation.
- **Composition:** Unsigned division remainder evaluation composes through all nine places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction `rem - 100u64` yielding `0`. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ solely in the dividend or bound (`500u64` yielding `100u64` vs. `499u64` yielding `99u64`), keeping the remainder and consuming subtraction operations identical.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time: computing `499u64 % 200u64` produces remainder `99u64`, for which exact subtraction `diff: u64 = rem - 100u64` cannot prove the subtraction does not underflow, rejecting strictly with `error: exact arithmetic in machine ... may overflow `u64`` (`exact subtraction underflow`). Static verification enforces that unproved subtraction obligations or out-of-range remainders cannot be admitted or executed.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Unsigned remainder expressions lower into canonical target-neutral Terminal Psi (`PSITERM\0`), using target-neutral unsigned remainder operators (`ExactIntegerRemainder` / `WrappingIntegerRemainder`) and integer proof certificates (`integer_certificates`), while erasing proof-only obligation tags (`proof-term-tags`). The Terminal Psi interpreter evaluates unsigned division remainder and exact subtraction, agreeing identically with native execution (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows join `ProgramEntry` declarations across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) to the Terminal catalog without reopening source. Primitive `u64` scalars (`size_of = 8`, `align_of = 8`) and `RemResult` records validate. Target `CallPlan`/`StatePlan` lower 64-bit unsigned function arguments and returns across standard integer registers (`rcx`/`rax` on Windows x86_64, `rdi`/`rax` on Linux x86_64, `x0` on AArch64) and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backend lowering selects target ISA instructions for remainder. On `x86_64`, unsigned division `div` yields quotient in `RAX` and remainder in `RDX` (clearing `RDX` with `xor edx, edx` before `div`); the backend selects `RDX` for the remainder result. On `arm64`, where no single remainder instruction exists, the backend selects `udiv` followed by multiply-subtract `msub` (`rem = a - (a / b) * b`). Register homes and activation frame storage are assigned, computing worst-case stack usage (WCSU), and standard function entry prologues and epilogues honoring the `StatePlan` ceiling are inserted.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks and conditional branches relax with offset reflow around exit transitions, machine-state footprint evidence is validated (`actual_transitive_footprint ⊆ permitted_transitive_use`), and direct PE/ELF/Mach-O images are emitted, achieving native execution agreement with Terminal Psi (`omega run --both`, exit 0) across all hosted targets including `windows_x86_64` and `macos_arm64`.

The row is verified by `samples/cli/facts/unsigned_division_remainder` where every positive records native exit 0 and Terminal Psi execution agreement (`omega run --both`), and every invalid twin rejects with `may overflow `u64``.
Pinned record at `b1493daf54dd+wt` on macOS arm64 (shared opt-level 1 producer): the root and nine places exit 0 under `omega run --both` (native exit 0, interpreter exit 0; 10/10) and `sample_gate.py --cli` reports the row's 10 positive projects and 9 intended-invalid controls checked with all 9 rejections matched. The nine `_exit` fixtures carry `checked` with `check-target:macos_arm64` and the nine `_rejected` twins carry `rejected ... expected` with their own `may overflow `u64`` diagnostic in `corpus_outcomes.txt`; nine `built ... exit:0` rows in `corpus_native_macos_arm64.txt`; nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in `corpus_both_macos_arm64.txt`. 0 keys lost, 0 timeout rows.
What remains open: Non-constant unsigned divisor interval-set theorem propagation and 128-bit wide unsigned remainder lowering across target platforms.
