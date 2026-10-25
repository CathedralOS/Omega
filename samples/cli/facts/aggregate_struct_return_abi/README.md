# Aggregate Struct Return ABI Realization

Parent task: `NORMALIZED-ABI-LOWERING` / `CONSERVATION-CONTRACT` (`TASKS.md`).
Ticket: `268c1b7311ca4420ac912427369c41dc`.

## Governing requirement

End-to-end nine-place fact preservation and ABI realization for large structural aggregate returns exceeding register-return limits (`sret`). Specifically, a 32-byte structural record (`Quartet`) returned and passed by value preserves field refinement bounds summing to `100u64` across indirect return homes when subtracting `100u64`, with exact execution agreement between Terminal Psi and native code across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`).

**A four-word structural aggregate returned and passed by value preserves field refinement bounds summing to 100u64 across indirect return homes when subtracting 100u64.**

Every place performs the same operation: construct and return a 32-byte (four-`u64`-field) structural record `Quartet` whose refinement clauses specify `first == 10u64`, `second == 20u64`, `third == 30u64`, and `fourth == 40u64` (summing to `100u64`). Because 32 bytes exceeds the 16-byte register return pair capacity on all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`), returning `Quartet` by value exercises indirect aggregate return (`sret`) lowering into caller-allocated stack frame return homes via target-mandated hidden pointer registers (`RCX` on `windows_x86_64`, `RDI` on `linux_x86_64`, and `X8` on `macos_arm64`/`linux_arm64`). Each place transports `Quartet` by value across one of nine canonical program locations, sums its four fields (`q.first + q.second + q.third + q.fourth`), and performs exact subtraction `total - 100u64` returning 0. Only the transport mechanism of the aggregate operand changes across places.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| Root | [`main.omg`](main.omg) | — | Direct indirect aggregate return and field summation |
| Local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | Aggregate returned by value is bound to a local before field projection and subtraction |
| Field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | Aggregate returned by value is stored into an enclosing record field before projection |
| Parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Aggregate returned by value is passed by value as an explicit parameter |
| Call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Aggregate returned by value is returned directly from an indirect helper call |
| Match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Both match arms execute indirect aggregate returns and join into an aggregate |
| Const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Named constants define the field values returned in the four-word aggregate |
| A nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | A successor machine receives the returned aggregate by value and projects its fields |
| Another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | An intermediate forwarding machine transports the returned aggregate by value |
| Another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Target-scoped machine bodies execute indirect aggregate returns on each hosted target |

### Verification commands

Root `main.omg` and all nine place projects bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declare `Expected exit: 0`. Run any positive directly:

```sh
omega run --both samples/cli/facts/aggregate_struct_return_abi/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/local/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/field/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/parameter/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/call_result/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/match_join/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/const_name/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/other_state/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/other_machine/main.omg
omega run --both samples/cli/facts/aggregate_struct_return_abi/target_body/main.omg
```

Each place has an `invalid/` twin whose fourth field bound and value is `39u64` instead of `40u64` (summing to `99u64`), causing exact subtraction `total - 100u64` to fail proof checking with `may overflow \`u64\`` (`exact subtraction underflow`). Check each twin with:

```sh
omega --check samples/cli/facts/aggregate_struct_return_abi/<place>/invalid/main.omg
```

## Observed evidence

Measured on `2d783d092d9d` (producer `target/debug/omega`, opt-level 1, host `macos_arm64`):

- `python3 tools/sample_native_gate.py --filter cli/facts/aggregate_struct_return_abi` exits 0: `10/10 samples meet their documented oracle on macos_arm64` and `9/9 intended-invalid controls match their authored rejection`.
- `python3 tools/corpus_gate.py --filter facts/aggregate_struct_return_abi` reports `0 structural move(s)` over `18 fixtures ({'checked': 9, 'rejected': 9})`, with `0 fail fixtures did not reject`.
- `python3 tools/corpus_gate.py --native --record --filter facts/aggregate_struct_return_abi` pinned the nine positive places as `built ... exit:0`.

The oracle was shown to discriminate at the native boundary rather than merely agreeing with itself. `omega run --both --keep --target macos_arm64` emits a Mach-O arm64 image in which `make_quartet` writes the 32-byte aggregate through the caller return home in `X8` (`mov x8, x8` / `str x1, [x8]` … `str x0, [x8, #0x18]`) and `probe` sets `mov x8, sp` before the call, copies four words, and computes `sum - 0x64`. Re-signing that image and patching only the single instruction that loads the fourth field (`movz x3, #0x28` to `movz x3, #0x27`, 40 to 39) changes the observed process outcome from `exit 0` to `exit 133` (SIGTRAP from the `brk #0` crash arm). The recorded exit is therefore what the program actually produces through the indirect aggregate return path.

## Requirements and Evidence

Owning tasks: **NORMALIZED-ABI-LOWERING** and **CONSERVATION-CONTRACT** (`TASKS.md`).
Ticket: `268c1b7311ca4420ac912427369c41dc`.
This row establishes that end-to-end nine-place fact preservation and ABI calling plan realization for large structural aggregate returns exceeding register-return limits (`sret`) preserves field refinement bounds summing to `100u64` across indirect return homes when subtracting `100u64`.

- **Establishment:** Constructing and returning the multi-field aggregate record `Quartet` with `where` refinement invariants (`first >= 10u64 && first <= 10u64`, `second >= 20u64 && second <= 20u64`, `third >= 30u64 && third <= 30u64`, `fourth >= 40u64 && fourth <= 40u64`) establishes the returned fields' static bounds directly without any post-hoc caller equality guard (`transition x == 100`). Helper machine `probe() -> u64` declares postcondition contract `ensures result == 0u64`, which is established and proved directly from the summation of the invariant bounds (`10 + 20 + 30 + 40 = 100`) and exact subtraction `total - 100u64`.
- **Identity and lifetime:** The returned aggregate record `Quartet` and its projected fields retain their exact storage and projection identity across indirect `sret` return transfer and through all nine places. As an owned copyable value (`[copy]`), `Quartet` occupies a contiguous 32-byte layout geometry. Values written into the caller-allocated stack return home survive stack-frame movement, local binding, record encapsulation, parameter passing, and state transitions without clobbering, displacement, or lifetime aliasing.
- **Composition:** The aggregate return fields and static bounds compose through all nine places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction `total - 100u64`. Across all places, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ solely in the fourth field value and bound (`40u64` vs. `39u64`), keeping aggregate construction, `sret` return transport, and consuming operations completely identical.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time: weakening the fourth refinement bound to `39u64` reduces the guaranteed sum to `99u64`, causing exact subtraction `total - 100u64` to underflow and strictly reject during proof checking with `may overflow \`u64\`` (`exact subtraction underflow`). Static verification enforces that unproved or out-of-bounds aggregate returns cannot be admitted to realization or execution.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Aggregate construction, return outcomes, and field projections lower into canonical target-neutral Terminal Psi (`PSITERM\0`), using `ReturnStructural` with a distinct operation-result place and whole-value structural custody while erasing proof-only obligations (`proof-term-tags`). The Terminal Psi interpreter executes indirect structural returns and agrees identically with native execution (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows join `ProgramEntry` declarations across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) to the Terminal catalog without reopening source. Layout plans validate 32-byte aggregate layout geometry (`size_of = 32`, `align_of = 8`, field offsets 0, 8, 16, 24). Target `CallPlan`/`StatePlan` classify the 32-byte aggregate return as exceeding the 16-byte register return pair limit (`RAX`+`RDX` on x86_64, `X0`+`X1` on ARM64), mandating hidden indirect aggregate return (`sret`) destination pointer lanes (`RCX` on `windows_x86_64`, `RDI` on `linux_x86_64`, `X8` on `macos_arm64`/`linux_arm64`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backend lowering allocates caller-frame `sret` destination stack storage, passes the destination pointer in the target-mandated register to the callee, materializes 64-bit field stores into the return home, computes worst-case stack usage (WCSU), and inserts function entry prologues and multi-return epilogues honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements and `sret` field offsets resolve, basic blocks and short/near branches relax with offset reflow around exit transitions, machine-state footprint evidence is validated (`actual_transitive_footprint ⊆ permitted_transitive_use`), and direct PE/ELF/Mach-O images are emitted, achieving native execution agreement with Terminal Psi (`omega run --both`, exit 0) across host targets including `windows_x86_64` and `macos_arm64`.

The row is pinned by `tests/omega/pass/facts/aggregate_struct_return_abi_through_*_exit` and `tests/omega/fail/facts/aggregate_struct_return_abi_through_*_rejected` fixtures; every positive records native exit 0 in `tests/omega/corpus_native_macos_arm64.txt` and every twin records `rejected ... expected` in `tests/omega/corpus_outcomes.txt` at `@114906953e11+wt`, with the nine `_exit` fixtures also recording `exit:0 interpreter-exit:0 interpreter:agrees` in `tests/omega/corpus_both_macos_arm64.txt`. The places and twins were measured with the wave's shared opt-level-1 producer at `9ff26a870215`: the root and all nine places report `native exit 0` and `interp exit: 0`, and all nine twins refuse with the `may overflow \`u64\`` fragment; the corpus legs on the rebased checkout at `114906953e11` are clean over 36 fixtures (18 `checked`, 18 `rejected ... expected`) with all nine `_exit` fixtures executed at exit 0.
Remaining open: Floating-point vector aggregate return classification (`HFA`/`HVX`), multi-return branch epilogue reflow with divergent aggregate return shapes, and dynamic slice view returns across indirect memory destinations.
