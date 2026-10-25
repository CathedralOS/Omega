# Write-Only Borrow Initialization Fact

Fact: a write-only borrow (`&write`) initializes a destination slot to `100u64` and restores owner readability across call completion when subtracting `100u64`.

Every place performs the same operation: a caller allocates a destination slot `slot: BoxedSlot`, lends it via a write-only borrow `&write slot` to `initialize_slot(destination: &write BoxedSlot)` which initializes `destination.value = 100u64`, restores owner readability across call completion, transports the initialized `100u64` value across that place, and performs exact subtraction `v - 100u64` returning `0`.

The original `main.omg` remains the starting witness. The nine places are:

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | The initialized slot value is bound to a local variable in root main. |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | The initialized slot value is bound to a local variable before exact subtraction. |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | The initialized slot value is stored in and read from a record field before exact subtraction. |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | The initialized slot value is passed as an explicit machine parameter before exact subtraction. |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | The initialized slot value is returned from a call before exact subtraction. |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | The initialized slot value is unified across both match arms before exact subtraction. |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | The subtrahend is governed by a named constant before exact subtraction. |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | The initialized slot value is passed as a successor machine parameter before exact subtraction. |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | The initialized slot value is forwarded through another machine before exact subtraction. |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Target-scoped machine bodies execute on each hosted target. |

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/write_only_borrow_initialization/main.omg
omega run --both samples/cli/facts/write_only_borrow_initialization/local/main.omg
omega run --both samples/cli/facts/write_only_borrow_initialization/field/main.omg
omega run --both samples/cli/facts/write_only_borrow_initialization/parameter/main.omg
omega run --both samples/cli/facts/write_only_borrow_initialization/call_result/main.omg
omega run --both samples/cli/facts/write_only_borrow_initialization/match_join/main.omg
omega run --both samples/cli/facts/write_only_borrow_initialization/const_name/main.omg
omega run --both samples/cli/facts/write_only_borrow_initialization/other_state/main.omg
omega run --both samples/cli/facts/write_only_borrow_initialization/other_machine/main.omg
omega run --both samples/cli/facts/write_only_borrow_initialization/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The authored exit 0 is not vacuously assumed:
- In root `main.omg` and child places, `probe()` returns the diff from `current - 100u64`. `main` checks `transition diff == 0 { true -> done() _ -> bad() }` where `machine bad() { crash Trap; }`. Any mismatch traps with `native signal 5` and interpreter crash `Trap` (`rc=255`), agreeing across both engines.
- In each `invalid/` twin, `SlotDomain` is widened to `v >= 99u64 && v <= 100u64` and `initialize_slot` stores `99u64` instead of `100u64`. Proof checking detects that `current - 100u64` can underflow `u64` and strictly rejects at compile time with `exact arithmetic in machine ... may overflow `u64`` (`exact subtraction underflow`), preventing execution entirely.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/write_only_borrow_initialization/<place>/invalid/main.omg
```

Every twin strictly rejects during proof checking with `may overflow `u64``.

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\omega-latest.exe --filter cli/facts/write_only_borrow_initialization --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `macos_arm64`).
- 9/9 intended-invalid controls match their authored rejection (`may overflow `u64``).

## Pinned Fixtures

The row is pinned by `tests/omega/pass/facts/write_only_borrow_initialization_through_<place>_exit` and `tests/omega/fail/facts/write_only_borrow_initialization_unproved_<place>` fixtures.
The nine positive places are pinned under `tests/omega/pass/facts/`, each binding `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and recording native exit 0.
The nine reject twins are pinned under `tests/omega/fail/facts/`, each rejecting with the exact arithmetic overflow diagnostic.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `bb5cbd034751445e906354ba119991b8`.
- **Requirement established:** End-to-end nine-place fact preservation and native realization for write-only borrow (`&write`) initialization and owner readability restoration. Specifically, passing a write-only borrow (`&write slot`) to a callee establishes definite initialization and the post-write domain fact on the destination, restores owner readability across call completion, and transports the initialized fact intact across all nine language places to prove exact integer arithmetic, with exact agreement between Terminal Psi and native execution across all four hosted targets.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Passing or holding a write-only borrow `&write slot` (with `destination: &write BoxedSlot`) establishes write-only reference authority over the caller's stack slot. `domain u64::SlotDomain(v) requires v >= 100u64 && v <= 100u64;` qualifies `BoxedSlot { value: u64 in SlotDomain; }`. Executing the checked non-observing store `destination.value = 100u64;` discharges `SlotDomain` membership at the assignment site. Upon callee completion, the destination slot is guaranteed definitely initialized with the static invariant `100u64`, introduced and established directly on the initialized value without requiring any post-hoc caller equality guard (`transition x == 100`).
  - **Identity and lifetime:** The write-only loan `&write slot` tracks the exact backing storage identity of the caller's stack-allocated `BoxedSlot` instance. For the duration of the loan, caller readability and exclusivity are suspended, and observation through `destination` is strictly forbidden. Once initialization completes and the callee returns, the write-only borrow ends and owner read/write authority is restored to the caller, carrying the exact backing storage identity and the newly initialized `SlotDomain` fact on `slot.value` without aliasing or unexpected copy.
  - **Composition:** The restored `100u64` initialized slot fact composes seamlessly through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction `current - 100u64 == 0`. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the initialization contract and assigned value (`destination.value = 99u64` under widened `v >= 99u64 && v <= 100u64` instead of `100u64`), leaving all allocation, lending, transport, and caller structures identical.
  - **Soundness boundaries:** The proof and checking system strictly enforces soundness boundaries for `&write`: paths attempting to read through `&write` before initialization, returning without definite initialization of the write target, or weakening the initialized value contract fail closed during compilation. All 9 invalid twins (`<place>/invalid/main.omg`) fail proof checking with `exact arithmetic in machine ... state ... terminal expression may overflow \`u64\`` (`exact subtraction underflow`), preventing uninitialized or contract-violating storage from escaping to runtime.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The static definite-initialization proof erases cleanly after checking, while `&write BoxedSlot` lowers into canonical target-neutral Terminal Psi (`PSITERM\0`) as a write-only pointer access with `StructuralScalarFieldStore` / `WriteOnlyPrimitiveStore`. Lowering enforces write-frame custody, materializes borrow restoration windows (`reborrow restored call uses`) that transition the home from write-locked to readable, and executes identically in the Terminal Psi interpreter (`omega run --both`), matching native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through Build companion bindings. The `BoxedSlot` record evaluates target layout geometry (`size_of == 8`, `align_of == 8`, field `value` offset 0), and target `CallPlan` and `StatePlan` lower `&write BoxedSlot` into standard ABI pointer register lanes (RCX on `windows_x86_64`, RDI on `linux_x86_64`, X0 on `macos_arm64` and `linux_arm64`) without runtime wrapper overhead.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backend lowering selects 64-bit indirect destination store instructions (e.g. `mov [rcx], 100` on x86-64, `str x1, [x0]` on ARM64) and subsequent caller reload instructions (`mov rax, [rsp + offset]` / `ldr x0, [sp, #offset]`), assigns register homes and disjoint stack-frame activation storage for `slot`, comports with worst-case stack bounds (WCSU), and inserts standard prologues and epilogues honoring the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve to concrete stack offsets for the backing `slot`, basic blocks lay out with short/near branch relaxation and offset reflow, machine-state footprint evidence validates against permitted limits, and direct PE/ELF/Mach-O executable images are emitted. Complete agreement between Terminal Psi interpretation and native execution (`omega run --both`, exit 0) is verified across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Callee `ensures` postcondition propagation on unrefined `&write` scalar parameters, multi-field write-only struct partial initialization matrices, and write-only dynamic slice view initialization.
