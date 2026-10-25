# Callee-Saved Register Restoration Across Nested Calls

Parent task: **CONSERVATION-CONTRACT** (`TASKS.md`).

## Governing requirement

A bounded scalar value (`saved <= 70u64`) held live across two sequential machine calls (`step_one(...)` and `step_two(...)`) preserves its exact static bound and runtime value across caller spill slots and callee-saved register restoration epilogues, discharging exact subtraction `70u64 - saved` without underflow across all nine places.

**Fact: a bounded value held live across two sequential machine calls preserves its static bound and value across callee-saved restoration when subtracting from 70u64.**

Every place performs the same operation: establish or transport a bounded scalar `saved: u64` (`40u64`, with bound `<= 70u64`), execute two sequential interior helper calls (`step_one` and `step_two`, which themselves exert register pressure and execute interior calls to `clobber`), preserve `saved` across both call boundaries, and perform exact subtraction `remaining = 70u64 - saved` (`30u64`) followed by `diff = remaining - (r1 + r2)` (`30u64 - 30u64 == 0u64`), returning `0` as `i32` to transition to `done()` and exit `0`. The positive oracle asserts exit `0` under both native and Terminal Psi execution. Only the transport and placement of `saved` changes across places.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| Root | [`main.omg`](main.omg) | — | Direct live-across-call scalar binding and exact subtraction |
| Local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | The bounded value is stored in a local binding held live across two calls |
| Field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | A record field carries the bounded value held live across two calls |
| Parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | The bounded value is passed as a machine parameter held live across two calls |
| Call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | A machine call returns the bounded value held live across two calls |
| Match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Both match arms produce the bounded value joining into a live binding across two calls |
| Const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | A named constant carries the bounded value held live across two calls |
| A nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | A successor machine receives the bounded value held live across two calls |
| Another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | A forwarding machine forwards the bounded value held live across two calls |
| Another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Each of four target bodies holds the bounded value live across two calls |

### Verification commands

Root `main.omg` and all nine place projects bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry `expected_native_exit.txt` (`0`). Run any positive directly:

```sh
omega run --both samples/cli/facts/callee_saved_across_nested_calls/main.omg
omega run --both samples/cli/facts/callee_saved_across_nested_calls/local/main.omg
omega run --both samples/cli/facts/callee_saved_across_nested_calls/field/main.omg
omega run --both samples/cli/facts/callee_saved_across_nested_calls/parameter/main.omg
omega run --both samples/cli/facts/callee_saved_across_nested_calls/call_result/main.omg
omega run --both samples/cli/facts/callee_saved_across_nested_calls/match_join/main.omg
omega run --both samples/cli/facts/callee_saved_across_nested_calls/const_name/main.omg
omega run --both samples/cli/facts/callee_saved_across_nested_calls/other_state/main.omg
omega run --both samples/cli/facts/callee_saved_across_nested_calls/other_machine/main.omg
omega run --both samples/cli/facts/callee_saved_across_nested_calls/target_body/main.omg
```

Each place has an `invalid/` twin that sets `saved = 71u64` (violating `saved <= 70u64`), causing exact subtraction `remaining = 70u64 - saved` to underflow. The control must reject with `may overflow `u64``. Check each twin with:

```sh
omega --check samples/cli/facts/callee_saved_across_nested_calls/<place>/invalid/main.omg
```

## Requirements and Evidence

Owning requirement: `wiki/spec/resources/activation_storage.md#callee-saved-register-groups`, `wiki/spec/build/calling_plans.md#register-preservation`, and `wiki/spec/terminal-psi/integer_certificates.md#canonical-scalar-goals`.
Parent task: **CONSERVATION-CONTRACT** (`TASKS.md`).
This row establishes that a bounded scalar value held live across sequential interior machine calls survives caller spill allocation, nested call execution, and callee-saved register restoration epilogues, maintaining its static bounds and runtime bit-exact value across all nine places.

- **Establishment:** The caller establishes `saved <= 70u64` (instantiated at `40u64`), while the two sequential callees `step_one` and `step_two` establish `ensures result == 15u64`. Holding `saved` live across both calls retains its static interval bound `[0, 70]`, which directly proves `70u64 - saved` cannot underflow `u64` without requiring any post-hoc caller equality guard (`transition x == 0`).
- **Identity and lifetime:** The scalar `saved: u64` value resides in an assigned non-volatile register or activation spill slot during execution of `step_one` and `step_two`. Because `step_one` and `step_two` execute interior calls to `clobber` that touch caller-saved registers, the callee's register allocator saves and restores non-volatile registers (such as `RBX`, `R12`-`R15` on `x86_64` or `X19`-`X28` on `AArch64`) using entry prologues and restoration epilogues, preserving `saved`'s bit-exact identity and lifetime across both call boundaries without memory corruption or clobbering.
- **Composition:** The preserved live-across-call scalar value composes through all nine standard program places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), feeding into the exact subtraction `70u64 - saved` and subsequent subtraction of call results `remaining - (r1 + r2) == 0`. Every place and its reject twin differ solely in the initial bound or value of `saved` (`40u64` vs. `71u64`), keeping the call structure and subtraction operations identical.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time: setting `saved = 71u64` violates the subtraction precondition (`70u64 - saved` would underflow `u64`), causing the compiler's range check and integer proof checker to reject with `error: exact arithmetic in machine ... may overflow `u64`` (`exact subtraction underflow`). Static verification enforces that unproved subtraction obligations cannot be admitted or executed.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Sequential machine calls lower into discrete Terminal Psi basic blocks with explicit `Call` instructions and live variable continuation environments (`PSITERM\0`). The live value `saved` is tracked across block boundaries in the continuation's live set, preserving proof obligation tags and integer certificates (`integer_certificates`) into the Terminal Psi interpreter, which evaluates the sequential calls and subtraction in exact agreement with native execution (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings join `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Standard calling conventions (`CallPlan`) assign parameter and return registers (`rcx`/`rax` on Windows x86_64, `rdi`/`rax` on Linux x86_64, `x0` on AArch64). Primitive `u64` scalars (`size_of = 8`, `align_of = 8`) and `SavedHolder` records validate.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** The backend register allocator assigns homes for `saved` and intermediate values. Because `saved` is live across two sequential calls, the allocator places it either in a callee-saved non-volatile register or in a dedicated stack spill slot in the activation frame. In `step_one` and `step_two`, interior call pressure triggers callee-saved register usage; the frame realization pass emits matching entry push/sub/store sequences and exit restoration load/add/pop sequences that preserve callee-saved registers across both calls.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Relocation fixups resolve symbolic stack-frame displacements and function addresses, basic blocks and branch offsets reflow across prologue/epilogue insertion, transitive machine-state footprint evidence is validated (`actual_transitive_footprint ⊆ permitted_transitive_use`), and direct PE/ELF/Mach-O images are emitted, achieving native execution agreement with Terminal Psi (`omega run --both`, exit 0) across all hosted targets including `windows_x86_64` and `macos_arm64`.

The row is verified by `samples/cli/facts/callee_saved_across_nested_calls` where every positive records native exit 0 and Terminal Psi execution agreement (`omega run --both`), and every invalid twin rejects with `may overflow `u64``.

## Measured evidence

Producer: `omega` opt-level 1, `--build-profile` `{"schema":"omega-compiler-build-profile-v1","opt_level":"1"}`.
Host: macOS arm64 (`darwin`/`arm64`). Base revision: `964b471b6cbf3f10b718089af3f74f121153b1c7`.

| Command | Result |
| --- | --- |
| `omega run --both` on root and all nine places (10 projects) | 10/10 `native exit 0` with `interp exit: 0` |
| `omega --check <place>/invalid/main.omg` (9 twins) | 9/9 reject with `may overflow `u64`` |
| `sample_native_gate --programs-only --facts --filter callee_saved_across_nested_calls` | 10/10 meet the documented oracle, 0 unscored |
| `sample_native_gate --facts --filter <nine>/invalid` | 9/9 intended-invalid controls match their authored rejection |
| `corpus_gate --native --filter facts/callee_saved_across_nested_calls` | clean: 9 fixtures `{'built': 9}`, executed 9 `{'0': 9}` |

Pinned fixtures: nine positives under
`tests/omega/pass/facts/callee_saved_across_nested_calls_through_<place>_exit`
(each with `main.omg`, `build.omg` binding all four hosted targets, and
`expected_native_exit.txt` `0`), and nine reject twins under
`tests/omega/fail/facts/callee_saved_across_nested_calls_unproved_<place>`
(each with `expected.txt` `may overflow `u64``). Golden rows: the nine
`rejected ... expected` rows in `tests/omega/corpus_outcomes.txt` and the nine
`built ... exit:0` rows in `tests/omega/corpus_native_macos_arm64.txt`.

### Pin re-verification (`9ff26a870215`, 2026-10-22)

Producer: `factsr1-producer` `target/debug/omega` built at `9ff26a870215`,
macOS arm64. Commands and numbers:

| Command | Result |
| --- | --- |
| `omega run --both` on root and all nine places (10 projects) | 10/10 `native exit 0` with `interp exit: 0` |
| `omega --check <place>/invalid/main.omg` (9 twins) | 9/9 reject, exit 1, `may overflow `u64`` |
| `omega --check tests/omega/fail/facts/callee_saved_across_nested_calls_unproved_<place>/main.omg` (9 fixture twins) | 9/9 reject, exit 1, `may overflow `u64`` |

Each of the nine pinned twins is byte-identical to its sample-side
`<place>/invalid/main.omg`, and each pinned positive is byte-identical to
`<place>/main.omg`, so the fixtures pin exactly the programs the row authors.

### Discrimination

Each positive place and its reject twin differ only in the construct's own
value or domain (`saved = 40u64` with bound `<= 70u64` vs. `71u64`); the call
structure, the two sequential nested calls, and both subtraction operations are
identical. Verified by `diff` between `<place>/main.omg` and
`<place>/invalid/main.omg`.

Static versus runtime corruption was measured rather than assumed:

- Corrupting `saved` to `71u64` (or `41u64`) as a source literal is refused
  **statically** by the checker before native production
  (`exact arithmetic ... may overflow `u64``); it never reaches an engine.
- Corrupting `step_one`'s interior `clobber` return is likewise refused
  statically, by that machine's own `ensures result == 0u64` contract.
- To reach a **runtime** trap the value is seeded through a `boundary trait`
  whose contract admits a range (`ensures result <= 70u64`) the prover cannot
  constant-fold. The admitted run is `native exit 0` / `interp exit: 0`;
  corrupting the supplied value then traps in both engines
  (`native signal 5`, `interp crash: Trap`), and the two engines agree.

The nested-depth obligation itself is carried by the emitted frame, not by the
arithmetic observation: on `macos_arm64` the positive root's `run` saves `x19`
and `x20` in its prologue, keeps `saved` in `x19` live across two sequential
callee frames (`step_one` and `step_two`, each of which itself calls `clobber`),
and restores `x19`/`x20` in its epilogue before reading `70u64 - saved`. A
corruption of a value that only had to survive one call still traps, so the
arithmetic oracle alone does not isolate nesting; the frame liveness above is
the nesting evidence.

What remains open: Dynamic stack realignment across multi-nested exception boundaries and vectorized callee-saved SIMD register preservation profiles.
