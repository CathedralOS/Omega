# Zero-Byte Erased Carrier

Fact: a record pairing a zero-byte token with a u64 value carries value >= 100u64 && value <= 100u64 with zero physical byte overhead across nine places, discharging exact subtraction value - 100u64 and size_of checks.

Owning Task: `CONSERVATION-CONTRACT`
Specification Reference: [Layout plans](wiki/spec/layouts/plans.md) and [Structural access](wiki/spec/terminal-psi/structural_access.md)

This fact row demonstrates that a nominal record `Carrier` containing a zero-byte token field `Token { empty: [u8; 0] }` paired with an executable scalar `value: u64` contributes zero physical byte overhead (`size_of<Token>() == 0` and `size_of<Carrier>() == 8`). The static contract bound `carrier.value >= 100u64 && carrier.value <= 100u64` is preserved across all nine standard places, discharging exact subtraction `carrier.value - 100u64 == 0` and size assertions at exit code 0.

Every positive place and the root witness bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry an authored `expected_native_exit.txt` containing `0`. Both engines (`omega run --both`) agree on exit code 0.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness creating Carrier pairing zero-byte Token with value 100u64 | [`main.omg`](main.omg) | - |
| local | `local/` | the carrier record is bound to a local variable before calling verify_carrier | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the carrier record is wrapped in a nominal Holder record field before extraction | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | a helper probe machine receives the carrier record as an explicit parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | a producer machine returns the carrier record under guaranteed contract postcondition | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both branches of a boolean match expression yield the carrier record before verification | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | a named constant INIT_VALUE initializes the carrier value before verification | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | `other_state/` | an terminal machine transfer transfers the carrier record to a successor machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | an associated Relay machine forwards the carrier record to verify_carrier | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of the four hosted target bodies receives the carrier record to verify | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/zero_byte_erased_carrier/main.omg
omega run --both samples/cli/facts/zero_byte_erased_carrier/local/main.omg
omega run --both samples/cli/facts/zero_byte_erased_carrier/field/main.omg
omega run --both samples/cli/facts/zero_byte_erased_carrier/parameter/main.omg
omega run --both samples/cli/facts/zero_byte_erased_carrier/call_result/main.omg
omega run --both samples/cli/facts/zero_byte_erased_carrier/match_join/main.omg
omega run --both samples/cli/facts/zero_byte_erased_carrier/const_name/main.omg
omega run --both samples/cli/facts/zero_byte_erased_carrier/other_state/main.omg
omega run --both samples/cli/facts/zero_byte_erased_carrier/other_machine/main.omg
omega run --both samples/cli/facts/zero_byte_erased_carrier/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg` where the carrier's value is weakened to `99u64`, violating the contract precondition `carrier.value >= 100u64 && carrier.value <= 100u64`. Check each twin with:

```sh
omega --check samples/cli/facts/zero_byte_erased_carrier/local/invalid/main.omg
omega --check samples/cli/facts/zero_byte_erased_carrier/field/invalid/main.omg
omega --check samples/cli/facts/zero_byte_erased_carrier/parameter/invalid/main.omg
omega --check samples/cli/facts/zero_byte_erased_carrier/call_result/invalid/main.omg
omega --check samples/cli/facts/zero_byte_erased_carrier/match_join/invalid/main.omg
omega --check samples/cli/facts/zero_byte_erased_carrier/const_name/invalid/main.omg
omega --check samples/cli/facts/zero_byte_erased_carrier/other_state/invalid/main.omg
omega --check samples/cli/facts/zero_byte_erased_carrier/other_machine/invalid/main.omg
omega --check samples/cli/facts/zero_byte_erased_carrier/target_body/invalid/main.omg
```

Every reject twin fails closed with:
`error: cannot prove requires contract for call ...: carrier.value >= 100 && carrier.value <= 100`

## Pinned fixtures

The row is measured by its fixtures under `tests/omega/`, not by the sample-side
oracle file: the sample's `expected_native_exit.txt` scores only through
`tools/corpus_gate.py`, which reads it directly. Each positive place is pinned
as `tests/omega/pass/facts/zero_byte_erased_carrier_through_<place>_exit` and
each reject twin as `tests/omega/fail/facts/zero_byte_erased_carrier_unproved_<place>`,
where `<place>` is one of `local`, `field`, `parameter`, `call_result`,
`match_join`, `const_name`, `other_state`, `other_machine`, `target_body`.

Measured on `macos_arm64` at the recorded revision:

| Leg | Command | Result |
| --- | --- | --- |
| check | `python3 tools/corpus_gate.py --filter zero_byte_erased_carrier` | 18 fixtures: 9 `checked`, 9 `rejected`; 0 rejected without their expected fragment, 0 fail fixtures did not reject |
| native | `python3 tools/corpus_gate.py --native --filter pass/facts/zero_byte_erased_carrier` | 9 fixtures `built`; executed 9 with exit codes `{0: 9}` |

### Oracle discrimination

The zero-byte member carries no bytes, so the oracle must be shown to depend on
the carrier at all, and specifically on its `u64` payload rather than its token.
Corrupting the payload (`value: 100u64` to `value: 100u64 + 1u64`) while leaving
the construct otherwise identical flips the observed result from exit 0 to a
trap in **both** engines: native reports `native signal 5`, Terminal Psi reports
`interp crash: Trap`, and agreement is preserved. The payload is what the oracle
observes; the token is not.

### Zero-storage measurement

Applying the same construct, `size_of<Token>() == 0`, `align_of<Token>() == 1`,
`size_of<Carrier>() == 8`, and `align_of<Carrier>() == 8` all hold as exit 0 in
both engines. Falsification: flipping the claim to `token_size == 1` traps in
both engines, so the measurement discriminates rather than reporting a constant.
The token therefore contributes zero extents at byte alignment, and `Carrier`
is exactly its 8-byte scalar with no padding overhead.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation for erased zero-byte carrier layout geometry and contract bounds. A zero-byte token (`size_of<Token>() == 0`) paired in a structural record with an executable `u64` value (`size_of<Carrier>() == 8`) imposes zero physical byte overhead while preserving static contract guarantees across all nine language transport places with exact differential agreement between Terminal Psi and native execution across hosted targets.
- **Authority:** [Layout plans](wiki/spec/layouts/plans.md#erased-and-zero-sized-carriers) and [Structural access](wiki/spec/terminal-psi/structural_access.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** Constructing the nominal `Carrier { token: Token { empty: [] }, value: 100u64 }` record establishes the static contract bound `carrier.value >= 100u64 && carrier.value <= 100u64` at construction. The `verify_carrier` machine checks `size_of<Token>() == 0`, `size_of<Carrier>() == 8`, and performs exact subtraction `carrier.value - 100u64 == 0` directly from the established bounds without any post-hoc caller equality guard (`transition x == 100`). The rejecting form of this establishment is pinned by `zero_byte_erased_carrier_unproved_*`: supplying `99u64` denies the bound, and the checker refuses the call rather than admitting it.
- **Identity and lifetime:** The zero-byte token field `empty: [u8; 0]` contributes zero physical bytes to the layout while maintaining distinct nominal type identity. The carrier value and contract bound retain exact identity and scope across all nine places without loss or unexpected field padding.
- **Composition:** The zero-byte carrier composes through all 9 standard places: local variable assignment (`local`), struct field wrapping (`field`), machine parameter passing (`parameter`), function call return value (`call_result`), match branch convergence join (`match_join`), named constant initialization (`const_name`), state transition arrival (`other_state`), forwarding machine delegation (`other_machine`), and target-scoped body candidates (`target_body`). Each `<place>/invalid/main.omg` differs solely in supplying `value: 99u64`, violating the contract bound.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `cannot prove requires contract for call ...: carrier.value >= 100 && carrier.value <= 100`. The compiler strictly refuses callers that cannot prove the contract precondition before lowering.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Mathematical/proof-only constraints and zero-length array fields lower cleanly into canonical `PSITERM\0`. Proof-term tags (`proof-term-tags`) track semantic contracts, while the runtime carrier executes identically in the Terminal Psi interpreter (`omega run --both`), observing exit code 0.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout planning evaluates `size_of<Token>() == 0` (zero extent at byte alignment) and `size_of<Carrier>() == 8` (8-byte scalar value with zero padding overhead from the token). In `CallPlan` and `StatePlan`, zero-byte carriers and fields allocate exactly zero registers or stack slots, passing only the 8-byte scalar value in the target ABI integer register (`rcx`/`rdi`/`x0`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects 64-bit integer arithmetic (`sub`) and comparison instructions without emitting storage, moves, or spill slots for the zero-byte token. Disjoint activation frames allocate space only for the executable 8-byte value, verifying worst-case stack usage (WCSU).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks and conditional branches relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O images are emitted that execute and exit with code 0 on `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Multi-field composite zero-sized carriers, trailing dynamically-sized byte views following zero-byte prefixes, and generic const zero-sized carrier parameterization remain governed under the wider `CONSERVATION-CONTRACT` and layout plan roadmap.
