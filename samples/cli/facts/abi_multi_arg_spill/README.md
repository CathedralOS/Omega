# Ten-Argument ABI Multi-Arg Register and Stack Spill Realization

Fact: a ten-argument weighted scalar call preserves exact positional argument order across register and stack ABI slots when subtracting 100u64.

Every place performs the same operation: invoke a ten-argument scalar function `weighted_ten_sum(a1..a10)` with distinct constant actuals (`1, 2, 3, 4, 5, 6, 7, 8, 24, 40`) summing to `100u64`, verify contract substitution and exact positional argument correspondence across register argument slots and stack spill slots on all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), and perform exact subtraction `total - 100u64` returning 0. Only the transport mechanism of the operands and call invocation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root ten-argument call across register and stack spill slots |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | ten distinct arguments are stored in locals before passing to the call |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | arguments are stored in and read from record fields |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | ten arguments arrive as explicit parameters and are forwarded directly |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | contracted helper call results supply the arguments |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms execute and join the multi-argument call |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply all ten register and stack operands |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives all ten arguments and invokes the call |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate machine forwards all ten arguments |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | target-scoped machine bodies execute on each hosted platform |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and declares
`Expected exit: 0`.

## Execution

Each positive place is an application that exits 0, and every positive place
(and the root `main.omg`) carries an authored `expected_native_exit.txt`
containing `0` beside its entry. `omega run --both` must report Terminal Psi
and native execution agreeing, so each line below observes both engines:

```sh
omega run --both samples/cli/facts/abi_multi_arg_spill/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/local/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/field/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/parameter/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/call_result/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/match_join/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/const_name/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/other_state/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/other_machine/main.omg
omega run --both samples/cli/facts/abi_multi_arg_spill/target_body/main.omg
```

Every one of the ten places was observed with `native exit 0` and
`interp exit: 0`.

### The oracle discriminates

The authored `0` is not assumed. Corrupting the value under test in the
`local` place by inverting the entry guard to `transition probe() == 1` makes
the fact's operation unreachable at runtime: the state machine takes `bad()`
and traps. Both engines still agree — `native signal 5` and
`interp crash: Trap (host presentation agrees)` — and the run exits `255`
instead of `0`. The recorded oracle therefore separates a preserved
ten-argument call from a broken one:

| Program | Native engine | Terminal Psi | wrapper exit |
| --- | --- | --- | --- |
| `local/main.omg` (authored) | `native exit 0` | `interp exit: 0` | 0 |
| same place, entry guard inverted | `native signal 5` | `interp crash: Trap` | 255 |

The `invalid/` twins below also discriminate in the other direction: they are
refused during proof checking rather than run, so they can never be mistaken
for a passing positive.

The control above moves the entry guard, not the argument. Corrupting the
tenth argument itself isolates the ABI lane under test, and the two spellings
separate cleanly:

- **A purely static corruption refuses before native production.** Replacing
  the tenth actual `40u64` with `39u64` is caught by the checker while
  instantiating the callee's `requires` clause, with `the instantiated fact is
  structurally false` naming `39 == 40`. No executable is produced, so this
  spelling cannot show that the stack lane is realized; it only shows the
  contract is enforced.
- **A boundary-seeded corruption reaches runtime and traps there.** Seeding
  the tenth argument from a boundary provider whose checked contract admits
  `result >= 39u64 && result <= 40u64` while its body returns `39u64` keeps the
  argument a genuine runtime operand the prover cannot constant-fold. The
  provided value is admissible at check time and wrong after the call, so the
  call returns `99u64` instead of `100u64`, the transition takes `bad()`, and
  both engines trap — `native signal 5` and
  `interp crash: Trap (host presentation agrees)`. A register-lane control
  built the same way (first argument seeded from a provider admitting
  `0u64..=1u64` and returning `0u64`) traps identically, so the differential
  covers both sides of the register/stack partition this fact crosses.

| Corruption of the value under test | Reaches native? | Native engine | Terminal Psi |
| --- | --- | --- | --- |
| tenth actual `40u64` → `39u64`, written statically | no, checker refuses | — | — |
| tenth argument seeded by a `39..=40` provider returning `39` | yes | `native signal 5` | `interp crash: Trap` |
| first argument seeded by a `0..=1` provider returning `0` | yes | `native signal 5` | `interp crash: Trap` |

Only the runtime rows count as discrimination. The static row is recorded
because it is the trap a reader falls into first, and because it explains why
the twin's diagnostic is a checker refusal rather than an execution result.

Each place has an `invalid/` twin whose 10th argument is `39u64` instead of `40u64` (summing to `99u64`), causing exact subtraction `total - 100u64` to fail proof checking with `may overflow \`u64\`` (`exact subtraction underflow`). All nine twins were observed refused with `checked compilation failed for package \`abi_multi_arg_spill_<place>_invalid\`` and the `may overflow \`u64\`` diagnostic naming that place's own entry point; none reaches native execution.
Check each twin with:

```sh
omega --check samples/cli/facts/abi_multi_arg_spill/<place>/invalid/main.omg
```

The same nine twins are also mirrored under `invalid/<place>/` with an authored
`expected.txt`, so the invalid controls are discoverable by both the per-place
route and the batch route. Each mirror is byte-identical to its per-place twin.

### Acceptance Gate

```sh
python3 tools/sample_native_gate.py --filter cli/facts/abi_multi_arg_spill
python3 tools/corpus_gate.py --native --filter facts/abi_multi_arg_spill
```

`sample_native_gate.py` scores each sample on the targets it authors. `corpus_gate.py`
scores the pinned fixtures directly: nine `built ... exit:0` rows for the positives
in `tests/omega/corpus_native_macos_arm64.txt` and nine `rejected ... expected`
rows for the twins in `tests/omega/corpus_outcomes.txt`.

## Pinned Fixtures

The nine positive places are pinned as
`tests/omega/pass/facts/abi_multi_arg_spill_through_<place>_exit/`, each
byte-identical to its sample place and carrying
`expected_native_exit.txt` with `0`. The nine reject twins are pinned as
`tests/omega/fail/facts/abi_multi_arg_spill_through_<place>_rejected/`, each
byte-identical to its `<place>/invalid/main.omg`, binding `ProgramEntry` for
all four hosted targets, carrying `expected.txt` with the
`may overflow \`u64\`` fragment, and rejecting with the exact-subtraction
diagnostic; their `rejected ... expected` rows are recorded in
`tests/omega/corpus_outcomes.txt` at `@114906953e11+wt`. The places and twins
were measured with the wave's shared opt-level-1 producer at `9ff26a870215`:
the root and all nine places report `native exit 0` and `interp exit: 0`, and
all nine twins refuse with their own `checked compilation failed for package
\`abi_multi_arg_spill_<place>_invalid\`` diagnostic carrying the
`may overflow \`u64\`` fragment. The corpus legs then re-measured the row on
the rebased checkout at `114906953e11`: the check leg is clean over 36
fixtures (18 `checked` positives, 18 `rejected ... expected` twins), the
native leg executed all nine `_exit` fixtures at exit 0, and the Terminal leg
recorded `exit:0 interpreter-exit:0 interpreter:agrees` for each of them.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `789f09ed1a6543bb909557ecbf53ba03` (owner `DeepSeek / w22 pool`).
- **Requirement established:** End-to-end nine-place fact preservation and ABI realization for multi-argument calls exceeding register limits. Specifically, positional argument correspondence and contract substitution across both register argument slots (RCX/RDX/R8/R9 on `windows_x86_64`, RDI/RSI/RDX/RCX/R8/R9 on `linux_x86_64`, X0..X7 on `macos_arm64`/`linux_arm64`) and stack spill argument slots (args 5..10 on Windows, 7..10 on Linux x86-64, 9..10 on ARM64) survive across all nine program places with exact agreement between Terminal Psi and native execution.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Concrete caller actuals satisfy callee `requires` obligations across all ten register and stack argument positions (`a1 == 1`, `a2 == 2`, ..., `a10 == 40`), establishing the combined postcondition `result == 100u64` directly without any post-hoc caller equality guard (`transition x == 100`) after the call completes.
  - **Identity and lifetime:** Positional parameter order is preserved across call entry, callee body execution, and return through ABI lowering, stack argument emission, and register allocation without clobbering, register collisions, or positional displacement. Every register-passed and stack-passed argument retains its exact identity and static bound throughout the call lifecycle.
  - **Composition:** Multi-argument parameter bounds and call results compose through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction (`total - 100u64`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in an argument value or parameter contract bound (`39u64` instead of `40u64`), while preserving all call signatures, calling conventions, and control paths.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins (and their mirrors under `invalid/<place>/`) fail closed during compile-time proof checking when the 10th stack argument is mutated from `40u64` to `39u64` (summing to `99u64`): the callee's postcondition fails, causing exact subtraction `total - 100u64` to reject with `exact arithmetic in machine ... may overflow \`u64\`` (underflow below zero). No unproved parameter sum reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Multi-argument calls, parameter bindings, and arithmetic sum reductions lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing mathematical proof obligations while preserving the ten-lane parameter list and exact actual arguments. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The ten `u64` scalar layout plans validate (`size_of == 8`, `align_of == 8`), and target `CallPlan`/`StatePlan` classify arguments into target-specific register vs. stack partitions: on `windows_x86_64`, the first 4 arguments occupy registers (`RCX`, `RDX`, `R8`, `R9`) and arguments 5..10 occupy outbound/inbound caller-frame stack parameter slots (plus 32-byte shadow space); on `linux_x86_64` (System V), the first 6 arguments occupy registers (`RDI`, `RSI`, `RDX`, `RCX`, `R8`, `R9`) and arguments 7..10 occupy stack slots; on `macos_arm64` and `linux_arm64` (AAPCS64), arguments 1..8 occupy registers `X0`..`X7` and arguments 9..10 occupy stack slots.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering materializes caller outbound stack argument stores at caller call sites (`mov [rsp + disp], reg/val`), moves register arguments into assigned physical register homes, and emits inbound caller-frame parameter loads (`mov reg, [rbp/rsp + disp]`) in callees. Activation storage composers allocate disjoint stack-frame space for outbound call arguments, compute worst-case stack usage (WCSU), and insert entry prologues and multi-return epilogues honoring 16-byte stack alignment.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Caller outbound stack argument displacements and callee inbound stack parameter offsets resolve after final stack-frame sizing, basic blocks and branch targets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Aggregate struct argument splitting across register/stack boundaries, indirect large struct return pointers (`sret`), and floating-point multi-argument register/stack partition rows.
