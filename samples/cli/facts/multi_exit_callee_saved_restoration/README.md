# Multi-Exit Callee-Saved Register Restoration

A multi-return callee with interior call pressure restores callee-saved registers on every exit path to preserve 100u64 when subtracting 100u64.

Every place invokes a multi-exit helper machine `multi_exit_probe(selector: u64, x: u64) -> u64` across three separate return states (`path_zero`, `path_one`, and `path_two`) selected by `selector`.
Each path holds eight live scalar bindings across an interior clobber helper call `touch_registers(1u64)` before returning `40u64` under contract `ensures result >= 40u64 && result <= 40u64`.
To satisfy this call pressure, the backend register allocator allocates callee-saved registers, requiring `frame_application/insertion` to emit an entry prologue saving them and insert a matching restoration epilogue at every retained return site while reflowing block/row coordinates and branch offsets.
In the caller, three scalar values (`k1 = 10u64`, `k2 = 20u64`, `k3 = 30u64`, summing to `60u64`) are held live across calls exercising the early and late return paths (`multi_exit_probe(0u64, 40u64)`, `multi_exit_probe(1u64, 40u64)`, and `multi_exit_probe(2u64, 40u64)`).
The preserved caller live sum (`60u64`) and normalized probe results (`(r0 + r1 + r2) - 80u64 == 40u64`) are combined into `100u64`, transported through each respective place, and exact subtraction `total - 100u64` is performed, returning `0` as `i32`.

| Place | Positive | Reject Twin |
| --- | --- | --- |
| local | `local/main.omg` | `local/invalid/main.omg` |
| field | `field/main.omg` | `field/invalid/main.omg` |
| parameter | `parameter/main.omg` | `parameter/invalid/main.omg` |
| call result | `call_result/main.omg` | `call_result/invalid/main.omg` |
| match join | `match_join/main.omg` | `match_join/invalid/main.omg` |
| const name | `const_name/main.omg` | `const_name/invalid/main.omg` |
| another state | `other_state/main.omg` | `other_state/invalid/main.omg` |
| another machine | `other_machine/main.omg` | `other_machine/invalid/main.omg` |
| another target body | `target_body/main.omg` | `target_body/invalid/main.omg` |

All nine positive places bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and pass `omega run --both` with exit 0 (Terminal Psi and native execution agreement):

```sh
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/local/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/field/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/parameter/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/call_result/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/match_join/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/const_name/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/other_state/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/other_machine/main.omg
omega run --both samples/cli/facts/multi_exit_callee_saved_restoration/target_body/main.omg
```

Each place has an `invalid/` twin whose caller live value is broken (`k3 = 29u64` instead of `30u64`, or `BASE_K1 = 9u64` instead of `10u64`), causing the combined sum to be `99u64` and exact subtraction `total - 100u64` to fail compilation with `exact arithmetic in machine ... may overflow \`u64\`` (`exact subtraction underflow`). Check each twin with:

```sh
omega --check samples/cli/facts/multi_exit_callee_saved_restoration/<place>/invalid/main.omg
```

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `ef38e603ddd04e6ca8065e8e62083781`.
- **Requirement established:** Callee-saved register restoration across all distinct return exits of a multi-exit function under interior call pressure, preserving caller live scalar values across function calls and exact subtraction obligations through all nine program places.
- **Obligations accounted for:**
  1. **Establishment:** The multi-exit probe establishes `ensures result >= 40u64 && result <= 40u64` on every distinct exit path (`path_zero`, `path_one`, and `path_two`), and the caller establishes 60u64 across pre-call bindings, forming exactly 100u64.
  2. **Identity and lifetime:** Callee-saved register state is strictly preserved across calls to `multi_exit_probe` regardless of which return path executes; caller live bindings retain their bit-exact identities across early, middle, and late exit returns without memory corruption or clobbering.
  3. **Composition:** Multi-exit return paths join cleanly at the caller call site, composing exact integer guarantees across all nine standard places (local, field, parameter, call result, match join, const name, other state, other machine, target body) to discharge exact subtraction without underflow.
  4. **Soundness boundaries:** Reject twins establish that breaking the caller's preserved live sum (to 99u64) leaves `total - 100u64` unprovable and strictly rejected at compile time with exact subtraction underflow.
  5. **Terminal Psi and carrier lowering:** Multi-exit machine transitions lower into discrete Terminal Psi basic blocks with multiple distinct `Return` terminators. Each path carries joined contract evidence, preserving scalar value results, live caller frontiers, and exact control-flow topology across all target selections.
  6. **Native realization, frame/layout encoding, and image emission:** The backend derives callee-saved preservation storage groups (`frame_layout`), encodes platform-specific prologue/epilogue sequences (`frame_protocol`), and applies them via `frame_application/insertion` by placing one prologue at the entry block and a full restoration epilogue before every retained return site while correctly reflowing block coordinates, row indices, fixup spans, and branch displacements. Native emission and Terminal Psi interpretation achieve exact execution agreement (`omega run --both`) across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`).
- **What remains open:** Dynamic stack realignment for over-aligned frame objects across multi-exit unwinding boundaries and exception-edge restoration profiles.
