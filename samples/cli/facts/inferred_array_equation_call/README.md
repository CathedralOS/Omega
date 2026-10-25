# Inferred Array Equation Call

Fact: an omitted `Capacity` is inferred as 2 from the actual `[u8; 2]` type and `Length == [u8; Capacity]`, independently of element values.

The root witness and nine place witnesses call `capacity(items)` with a declared `[u8; 2]` actual.
The compiler combines closed declaration-backed actual/result types with where equations in the
structural solver to infer `Capacity == 2`. No runtime narrowing assertions or bounds checks are needed.

Every positive place (and the root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declares `Expected exit: 0`,
and carries an authored `expected_native_exit.txt` containing `0` beside its entry. Both engines
(`omega run --both`) agree: Terminal Psi and native execution report exit 0.

## Places

| Place | Positive Program | Fact-sensitive Reject Twin | Carrier / Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | [`invalid/main.omg`](invalid/main.omg) | canonical root witness passing `[u8; 2]` actual directly to `capacity` |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | declared `[u8; 2]` array copied to local variable before `capacity` call |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | `[u8; 2]` array stored in and projected from `Holder::items` nominal field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | declared ordinary `[u8; 2]` parameter passed to `capacity` |
| call_result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | declared `[u8; 2]` array return value from helper function |
| match_join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms return `[u8; 2]`; joined array passed to `capacity` |
| const_name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | declared array constant `BASE: [u8; 2]` passed to `capacity` |
| other_state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | state arrival parameter `step(items: [u8; 2])` forwards to `capacity` |
| other_machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine routes `[u8; 2]` array to `capacity` |
| target_body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | four hosted target implementations perform the same inferred call |

## Execution

Every positive place has an authored native exit oracle of zero and verifies `observed == 2`,
crashing `Trap` otherwise. Run with `omega run --both`:

```sh
omega run --both samples/cli/facts/inferred_array_equation_call/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/local/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/field/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/parameter/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/call_result/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/match_join/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/const_name/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/other_state/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/other_machine/main.omg
omega run --both samples/cli/facts/inferred_array_equation_call/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/` (and root `invalid/`) that
changes the call to `capacity<[u8; 2], 3>(items)` with explicit `Capacity = 3`, conflicting with
`Length == [u8; Capacity]` since `Length` is `[u8; 2]`. All reject twins fail closed at proof checking
before native execution:

```sh
omega --check samples/cli/facts/inferred_array_equation_call/<place>/invalid/main.omg
```

Every twin rejects with:
`where equation binds \`Capacity\` to 2 but its explicit argument is 3`

## Requirements and Evidence

- **Parent outcome:** `STRUCTURAL-GENERIC-MATCHING` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation and structural type equation inference for generic array calls. An omitted const generic argument `Capacity` is solved as 2 from the actual array type `Length = [u8; 2]` and the where equation `Length == [u8; Capacity]`, preserving the inferred value across all nine places with exact agreement between Terminal Psi and native execution across hosted targets.
- **Pinned fixtures:** positive places and reject twins under `tests/omega/pass/facts/inferred_array_equation_call_through_*_exit` and `tests/omega/fail/facts/inferred_array_equation_call_through_*_rejected`.
- **Authority:** [Structural type equations and inference](../../../../wiki/spec/language/generics.md#structural-type-equations-and-inference) (omitted arguments are recovered when structural equations and argument/result inference determine one substitution; all occurrences must be consistent).

Full 8-layer specification and lowering evidence:

- **Establishment:** Calling the equation-constrained array helper `capacity<Length [copy], const Capacity: u64>(items: Length) where Length == [u8; Capacity]` infers `Capacity = 2` directly from the caller's `[u8; 2]` actual type signature. The structural type equation solver unifies `[u8; 2]` with `[u8; Capacity]`, establishing `Capacity = 2` directly at the call edge without any post-hoc caller equality guard (`transition x == 100`).
- **Identity and lifetime:** The inferred array equation and resulting capacity value remain tied to the exact array and scalar subjects across local copy (`local`), nominal record field projection (`field`), explicit parameter passing (`parameter`), completed call return (`call_result`), match convergence join (`match_join`), named const bound equality (`const_name`), state transition arrival (`other_state`), forwarding machine calls (`other_machine`), and target-scoped body candidates (`target_body`). No mutation, alias clobbering, or invalidating write intervenes between array creation and consumption.
- **Composition:** The inferred array equation fact composes through all nine places into the consuming call and transition check (`observed == 2`). Calls substitute exact actuals, match joins retain the unified type on both incoming branches, and state transitions prove arrival obligations on incoming edges. Each `<place>/main.omg` vs. `<place>/invalid/main.omg` differs only in supplying an explicit conflicting const generic argument `3` (`capacity<[u8; 2], 3>(items)`), never in a caller guard.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins (and root `invalid/main.omg`) fail closed with `where equation binds \`Capacity\` to 2 but its explicit argument is 3` when the caller supplies a mismatched array extent or conflicting generic argument. The checker strictly refuses incompatible equation bindings during type checking, preventing invalid programs from reaching Terminal Psi or native code emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Inferred compile-time array length equations and proof-formal const parameters erase cleanly after verification, while monomorphized fixed-size array carriers and element operations lower without loss into canonical target-neutral Terminal Psi (`PSITERM\0`). The resulting programs execute identically in the Terminal Psi interpreter (`omega run --both`), observing `observed == 2`.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Concrete layout geometry for `[u8; 2]` contiguous arrays (2 bytes, 1-byte alignment) and `Holder` records validates element stride and alignment. `CallPlan` and `StatePlan` place scalar `u64` return values in target ABI integer registers (`rax` on x86-64 / `x0` on ARM64) and pass array arguments according to target calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions for immediate moves, array loads/stores, and integer comparisons (`cmp`/`sete` on x86-64 / `cmp`/`cset` on ARM64). Backend assigns register homes and disjoint stack-frame activation storage for array locals, inserting entry prologues and multi-return epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements and function call targets resolve, basic blocks and conditional branches relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across `windows_x86_64` and `macos_arm64`.

## What remains open

Open generic schemas, late receiver/extension selection, operator/conformance/evidence binders, non-array structural equations, and computed equation solving remain governed under the wider `STRUCTURAL-GENERIC-MATCHING` roadmap.

## Generic result production

[Forwarded results](forwarded_result/README.md) carry the same fact through an
inferred generic call before equation matching, with another nine-place row and
conflicting explicit-capacity twins. Its fresh baseline rejected the composition;
its Linux x86-64 native and Terminal execution evidence is distinct from the
historical direct-call observations above.
