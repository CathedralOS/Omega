# Write-Only Borrow Initialization Fact

// Fact: a write-only borrow (&write) initializes a destination slot to 100u64 and restores owner readability across call completion when subtracting 100u64.

Every place performs the same operation: a caller allocates a destination slot `slot: BoxedSlot`, lends it via a write-only borrow `&write slot` to `initialize_slot(destination: &write BoxedSlot)` which initializes `destination.value = 100u64`, restores owner readability across call completion, transports the initialized `100u64` value across that place, and performs exact subtraction `v - 100u64` returning `0`.

The original `main.omg` remains the starting witness. The nine places are:

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | The initialized slot value is bound to a local variable before exact subtraction. |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | The initialized slot value is stored in and read from a record field before exact subtraction. |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | The initialized slot value is passed as an explicit machine parameter before exact subtraction. |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | The initialized slot value is returned from a call before exact subtraction. |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | The initialized slot value is unified across both match arms before exact subtraction. |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | The subtrahend is governed by a named constant before exact subtraction. |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | The initialized slot value is passed as a successor state parameter before exact subtraction. |
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

Each place has an `invalid/` twin whose `initialize_slot` stores `99u64` instead of `100u64`, causing exact subtraction `v - 100u64` to underflow `u64`. Check each twin with:

```sh
omega --check samples/cli/facts/write_only_borrow_initialization/<place>/invalid/main.omg
```

Each twin strictly rejects during proof checking with `exact arithmetic in machine ... may overflow \`u64\`` (`exact subtraction underflow`).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `bb5cbd034751445e906354ba119991b8`.
- **Requirement established:** End-to-end nine-place fact preservation and native realization for write-only borrow (`&write`) initialization and owner readability restoration. Specifically:
  1. **Establishment**: Calling `initialize_slot(destination: &write BoxedSlot)` establishes write-only reference authority over the caller's stack slot (`&write slot`) and executes a non-observing store `destination.value = 100u64`.
  2. **Identity and lifetime**: The borrowed place denotes the caller's stack-allocated `BoxedSlot` storage. The write-only loan suspends caller readability and exclusivity for the duration of the call, and normal return restores owner read authority without copying or re-allocating backing storage.
  3. **Composition**: The restored value `100u64` is preserved across all nine standard places (local binding, carrier record field, explicit machine parameter, call result return, match-expression join, named constant, successor state parameter, forwarding machine, and target-scoped bodies) before exact subtraction `v - 100u64` returning 0.
  4. **Soundness boundaries**: The reject twin (`invalid/`) initializes `slot.value = 99u64`, causing exact subtraction `v - 100u64` to fail proof checking with `exact arithmetic in machine ... may overflow \`u64\`` (`exact subtraction underflow`) across all nine places.
  5. **Terminal Psi and carrier lowering**: Source `&write BoxedSlot` lowers without loss to Terminal Psi `WriteOnlyBorrow` access with `StructuralScalarFieldStore` / `WriteOnlyPrimitiveStore`. Lowering enforces write-frame custody, forbids destination observation during the loan, and restores owner readability upon normal return completion.
  6. **Native realization, frame/layout encoding, and image emission**: Stage 07 resolves the symbolic stack-frame address for `slot` and emits indirect pointer passing in target registers (RCX on `windows_x86_64`, RDI on `linux_x86_64`, X0 on `macos_arm64` and `linux_arm64`). The callee realizes the indirect 64-bit store, and upon return the caller frame reloads `slot.value`, verified by complete agreement (`omega run --both`) between Terminal Psi interpretation and native binary execution across all four hosted targets.
- **What remains open**: Multi-field write-only struct partial initialization matrices, write-only dynamic slice view initialization, and uninitialized aggregate memory layouts without zero-fill guarantees.
