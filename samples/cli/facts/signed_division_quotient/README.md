# Signed Division Quotient

Fact: a signed i64 quotient from -300i64 divided by -3i64 equals 100i64 when subtracting 100i64.

Every place performs the same operation: compute the signed division of negative
dividend `-300i64` by negative divisor `-3i64`, transport the resulting `100i64` quotient,
verify it by casting to `u64` and performing exact subtraction `verified - 100u64`, and
return `0` as `i32` to exit `0`. Only the transport of the quotient changes.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness computing signed division quotient `(-300i64) / (-3i64)` |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the quotient is copied into a local binding |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | a record field carries the quotient into the subtraction check |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the quotient is passed as an explicit machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a call returns the quotient into the subtraction check |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms produce the quotient and join into the subtraction check |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant carries the quotient into the subtraction check |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives the quotient and performs the subtraction check |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine forwards the quotient to the verifying machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies performs the division and verifies the quotient |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and native execution agreement (`interp exit: 0` and `native exit 0`):

```sh
omega run --both samples/cli/facts/signed_division_quotient/main.omg
omega run --both samples/cli/facts/signed_division_quotient/local/main.omg
omega run --both samples/cli/facts/signed_division_quotient/field/main.omg
omega run --both samples/cli/facts/signed_division_quotient/parameter/main.omg
omega run --both samples/cli/facts/signed_division_quotient/call_result/main.omg
omega run --both samples/cli/facts/signed_division_quotient/match_join/main.omg
omega run --both samples/cli/facts/signed_division_quotient/const_name/main.omg
omega run --both samples/cli/facts/signed_division_quotient/other_state/main.omg
omega run --both samples/cli/facts/signed_division_quotient/other_machine/main.omg
omega run --both samples/cli/facts/signed_division_quotient/target_body/main.omg
python tools/sample_native_gate.py --facts --filter cli/facts/signed_division_quotient
```

### The oracle discriminates

The exit code `0` is strictly discriminative. Exact subtraction `verified - 100u64` requires the quotient to be at least `100u64`. When the quotient is computed as `(-300i64) / (-3i64) = 100i64`, exact subtraction evaluates to `0` and exits `0`. Corrupting the dividend or divisor changes the quotient and either violates compile-time exactness obligations or produces an unexpected exit code at runtime.

## Reject Twins

Each place has an `invalid/` reject twin with authored `sample.json` (`"role": "intended-invalid"` and `"rejection_contains": ["may overflow `u64`"]`). Breaking the quotient value by dividing `-297i64` by `-3i64` yields `99`, causing the downstream exact subtraction `verified - 100u64` to underflow. Compile-time proof checking fails closed with `may overflow `u64``:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `may overflow \`u64\`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `may overflow \`u64\`` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `may overflow \`u64\`` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `may overflow \`u64\`` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `may overflow \`u64\`` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `may overflow \`u64\`` |
| another state | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `may overflow \`u64\`` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `may overflow \`u64\`` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `may overflow \`u64\`` |

## Pinned Fixtures

The row is pinned by `tests/omega/pass/facts/signed_division_quotient_through_*_exit` and
`tests/omega/fail/facts/signed_division_quotient_through_*_rejected` fixtures; every positive
records native exit 0 and Terminal Psi execution agreement.

## Rounding discrimination

The nine places divide `-300i64` by `-3i64`, whose quotient `+100i64` is exact and
integral. It is therefore **identical under truncation toward zero and under
floor**, so the positive places alone cannot detect a compiler that rounds
negative quotients the wrong way. The rounding half of the fact is pinned
separately by the run-tier canary
`tests/omega/run/facts/signed_division_quotient_truncation_not_floor_traps`,
which asserts the floor result `(-7i64) / (2i64) == -4i64` and must trap,
because [numeric values](wiki/spec/language/numeric_values.md#integer-quotient-and-remainder)
fixes quotient truncation toward zero with `r = 0 or sign(r) = sign(a)`.

Measured on `macos_arm64` at `fca093f90f6d`, both engines independently agree:

```text
run/facts/signed_division_quotient_truncation_not_floor_traps built exit:signal signal:5 interpreter:trapped
```

A compiler that floored negative quotients would satisfy that assertion and exit
`0`, failing the canary. Two further measurements confirm the same boundary: the
wrong-sign corruption (`300i64 / -3i64`, expected `-100i64`) is refused at check
time by the exact-arithmetic obligation on `verified - 100u64`, and the
non-integral dividend `-301i64 / -3i64` truncates to `100i64` and exits `0`,
where a floor compiler would produce `-101i64`.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Governing specification:** [Numeric values](wiki/spec/language/numeric_values.md#division-and-remainder) and [Integer certificates](wiki/spec/terminal-psi/integer_certificates.md#canonical-scalar-goals).

### Specification and Lowering Layers

1. **Establishment:** The divisor non-zero bound (`divisor != 0`) and `MIN / -1` overflow exclusion bound (here with dividend `-300i64` and divisor `-3i64 <= -2i64`) establish safe signed division (`ExactDivisionDefined`) directly from static literal values and parameter `requires` contracts. The quotient's exact interval bound `quotient == 100i64` is proved statically from the operands, establishing that `verified: u64 = 100u64` satisfies `verified >= 100u64` and allowing `verified - 100u64` to discharge exact subtraction without underflow, with no post-hoc caller equality guard (`transition x == 100`).
2. **Identity and lifetime:** The signed dividend, divisor, and quotient bounds stay bound to the exact scalar values across all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`). The quotient `100i64` retains its exact magnitude and sign through scalar copying, aggregate record placement, parameter registers, and call returns without loss of precision or range distortion.
3. **Composition:** The signed division quotient fact composes through all 9 places into the consuming exact subtraction operation `(quotient as u64) - 100u64`. In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in the dividend (`-300i64` yielding quotient 100 vs `-297i64` yielding quotient 99), cleanly isolating the quotient value fact under test across every transport carrier.
4. **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof checking when the dividend is adjusted to `-297i64`. The resulting quotient `99i64` cast to `u64` violates the exact subtraction precondition `verified >= 100u64`, rejecting with diagnostic `error: ... may overflow `u64``. Similarly, divisor bounds that permit `0` or `i64::MIN / -1i64` fail closed before division admission, preventing division-by-zero or signed overflow faults from reaching runtime.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Proved signed division operations, quotient narrowing/casts, and exact subtraction lower without information loss into target-neutral canonical Terminal Psi (`PSITERM\0`). Arithmetic proof certificates and safety obligations are checked and erased, while preserving explicit signed integer division nodes (`DivS`), type conversions, and state transitions. The Terminal Psi interpreter executes root and all 9 positive places in complete agreement with native execution (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`, `ProviderPlan`, `CompilerBuiltinExecution`, `BoundaryExecutionBinding`) across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Signed integer scalar types (`i64`, `i32`) evaluate target layout geometry (`size_of` and `align_of`). `CallPlan` and `StatePlan` lower signed integer arguments and return registers according to target platform calling conventions (e.g. `RCX`/`RDX`/`RAX` on Windows x64, `RDI`/`RSI`/`RAX` on System V, `x0`-`x7` on ARM64).
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target signed division instructions respecting architecture-specific constraints: on `x86_64`, sign-extension (`cqo` for 64-bit dividend in `RAX` to `RDX:RAX`) followed by `idiv` with fixed register allocation; on `arm64`, `sdiv` instruction with general register allocation. Lowering assigns register homes and disjoint activation frame spill slots, verifying worst-case stack usage (WCSU) bounds and emitting platform-standard prologues and epilogues.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Final encoding resolves symbolic instruction displacements, lays out basic blocks with short/near branch relaxation, validates transitive machine-state footprints, and emits direct PE/ELF/Mach-O executable placements. Native images execute natively with exit code 0 and agree with Terminal Psi (`omega run --both` exit 0) across all hosted targets.
