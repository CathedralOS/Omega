# Selected scalar initialization

Owning requirement: **TERMINAL-SLICE-VIEW-VOCABULARY**, specifically construction of nonempty restricted scalar arrays and primitive locals when zero does not satisfy their declared predicate. This row establishes a closed non-interval initializer predicate through the existing source-to-Terminal-to-native path.

An explicitly initialized `[i32 in Selected; 2]` retains the nonzero disjunctive domain **1 or 38**, and indexing its second element observes **38**.

The original [root](main.omg) is frozen unchanged. All positives bind the four hosted ProgramEntry targets and declare exit 0. Each place runs with both Boolean inputs; the match join executes both incoming array arms.

## Places

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| root | `.` | canonical root witness initializing `[i32 in Selected; 2] = [1, 38]` and observing `items[1] == 38` | [`main.omg`](main.omg) | - |
| local | `local/` | initialized array bound to a local variable `items` before indexing `items[1]` | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | initialized array stored in nominal struct field `Holder.items` before indexing | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | initialized array passed as explicit function parameter `consume(items)` before indexing | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | initialized array returned as call result from helper machine `produce()` before indexing | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both arms of match expression construct initialized `[i32 in Selected; 2]` and join before indexing | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | named constant `ITEMS: [i32 in Selected; 2] = [1, 38]` defines established array before indexing | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | `other_state/` | state transition arrival parameter `consume(items)` receives initialized array before indexing | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | forwarding machine routes initialized array before indexing | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | four hosted target bodies (`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`) construct and index array | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

Every twin substitutes one authored `[1, 38]` initializer with `[1, 2]`. The match twin changes only the false arm; the target-body twin changes only Windows. Array length 2 and index 1 remain unchanged. Const formation rejects the false const-leaf domain; other twins reject array-element domain establishment. Derived call/arrival refusals can repeat that one missing membership. A missing import or unrelated lowering refusal does not count.

## Execution

Run each positive place with `omega run --both`:

```sh
omega run --both samples/cli/facts/selected_scalar_initializer/main.omg
omega run --both samples/cli/facts/selected_scalar_initializer/local/main.omg
omega run --both samples/cli/facts/selected_scalar_initializer/field/main.omg
omega run --both samples/cli/facts/selected_scalar_initializer/parameter/main.omg
omega run --both samples/cli/facts/selected_scalar_initializer/call_result/main.omg
omega run --both samples/cli/facts/selected_scalar_initializer/match_join/main.omg
omega run --both samples/cli/facts/selected_scalar_initializer/const_name/main.omg
omega run --both samples/cli/facts/selected_scalar_initializer/other_state/main.omg
omega run --both samples/cli/facts/selected_scalar_initializer/other_machine/main.omg
omega run --both samples/cli/facts/selected_scalar_initializer/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/main.omg`. Check each twin with:

```sh
omega --check samples/cli/facts/selected_scalar_initializer/local/invalid/main.omg
omega --check samples/cli/facts/selected_scalar_initializer/field/invalid/main.omg
omega --check samples/cli/facts/selected_scalar_initializer/parameter/invalid/main.omg
omega --check samples/cli/facts/selected_scalar_initializer/call_result/invalid/main.omg
omega --check samples/cli/facts/selected_scalar_initializer/match_join/invalid/main.omg
omega --check samples/cli/facts/selected_scalar_initializer/const_name/invalid/main.omg
omega --check samples/cli/facts/selected_scalar_initializer/other_state/invalid/main.omg
omega --check samples/cli/facts/selected_scalar_initializer/other_machine/invalid/main.omg
omega --check samples/cli/facts/selected_scalar_initializer/target_body/invalid/main.omg
```

Every twin rejects with:
- Non-const places: `array literal element is not proven in domain \`i32::Selected\``
- Const place: `domain constraint \`Selected\` for the const leaf value is false`

## Requirements and Evidence

- **Parent outcome:** `TERMINAL-SLICE-VIEW-VOCABULARY` (`TASKS.md`).
- **Requirement established:** Construction of nonempty restricted scalar arrays and primitive locals when zero does not satisfy their declared predicate; establishing a closed non-interval disjunctive initializer domain (`Selected: value == 1 || value == 38`) through the full source-to-Terminal-to-native execution path with differential agreement between native and Terminal Psi execution across all hosted targets.
- **Pinned fixtures:** `tests/omega/pass/facts/selected_scalar_initializer_through_*` and `tests/omega/fail/facts/selected_scalar_initializer_through_*`.
- **Authority:** [Domains](wiki/spec/language/domains.md) and [Default domains and zero initialization](wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization).

Full 8-layer specification and lowering evidence:

- **Establishment:** The predicate-only domain `domain i32::Selected(value) requires value == 1 || value == 38` declares a closed non-interval refinement where 0 is excluded. Explicit array literal initialization `[1, 38]` separately establishes domain membership for each literal leaf at compile time before any readable value exists. The second element's exact value 38 is established directly from its explicit initializer leaf (38 satisfies `value == 1 || value == 38`), establishing the qualified indexed read `observed: i32 in Selected = items[1]` and proving `observed == 38` directly without any post-hoc caller equality guard (`transition x == 100`).
- **Identity and lifetime:** The initialized array binding `items: [i32 in Selected; 2]` and its scalar projection retain the joined static domain membership and exact element values across all nine places. Replacement or mutation re-establishes the receiving predicate: copying 38 before overwriting with 1 preserves the snapshot value 38 independently of subsequent writes to unrelated or related storage.
- **Composition:** The established scalar initializer bound composes through all 9 standard places into the consuming indexed read and equality verification (`observed == 38 && items[0] == 1`): local variable binding (`local`), nominal record field projection (`field`), explicit parameter passing (`parameter`), function call return value (`call_result`), match branch convergence join (`match_join`), named constant value definition (`const_name`), state transition arrival (`other_state`), forwarding machine routing (`other_machine`), and target-scoped body candidates (`target_body`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differs solely in substituting the authored initializer `[1, 38]` with `[1, 2]` (or the match twin's false arm and the target-body twin's Windows body) where `2` violates `Selected`, never in a caller guard.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with `array literal element is not proven in domain \`i32::Selected\`` (or for const, `domain constraint \`Selected\` for the const leaf value is false`). No runtime predicate tag, default zero bypass, or unverified literal can establish domain membership, maintaining strict safety across the language boundary.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The common primitive initialization retains an obligation for the complete receiving restriction over the completed SSA operand. In canonical `PSITERM\0`, domain obligations are proved and erased, while the array literal and indexing operations lower cleanly without synthetic wrapper types, hidden tags, or qualification-specific ABI changes. Terminal Psi interpreter executes identically to native execution (`omega run --both`), observing exit 0.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Concrete layout geometry evaluates `[i32 in Selected; 2]` with standard 4-byte element layout (8 bytes total, 4-byte alignment), matching unrefined `[i32; 2]`. `CallPlan` and `StatePlan` lower arrays and scalar components across standard ABI integer registers and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions for immediate array element stores, indexed loads, integer comparisons (`cmp`), and conditional branches (`je`/`jne` on x86-64, `b.eq`/`b.ne` on ARM64). Backend assigns register homes and disjoint stack-frame activation storage with worst-case stack usage (WCSU), inserting entry prologues and callee-saved restoration epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements and function offsets resolve, basic blocks and conditional branches relax with offset reflow, machine-state footprint evidence validates (`actual_transitive_footprint ⊆ permitted_transitive_use`), and direct PE/ELF/Mach-O images are emitted that execute and exit with code 0 on `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` in full differential agreement with Terminal Psi (`omega run --both`).

## What remains open

Remaining parent acceptance under `TERMINAL-SLICE-VIEW-VOCABULARY`: non-interval refined scalar parameter/state-arrival contracts, narrowed record-element view default-domain restoration, cyclic Unit-graph view forwarding, and the parent's remaining consuming scalar/record view combinations. This bounded row does not close the parent capability or the full multi-host release matrix.

## Run records

| Check | Command | Result |
| --- | --- | --- |
| native golden | `python3 tools/corpus_gate.py --native --record --filter facts/selected_scalar_initializer` | 9 rows `built ... exit:0` |
| engine agreement | `python3 tools/corpus_gate.py --both --record --filter facts/selected_scalar_initializer` | 9 rows `interpreter-exit:0 interpreter:agrees` |
| check leg | `python3 tools/corpus_gate.py --filter facts/selected_scalar_initializer` | 18 fixtures: 9 `checked`, 9 `rejected`, 0 structural moves |
| sample gate | `python3 tools/sample_native_gate.py --filter cli/facts/selected_scalar_initializer --facts` | 10/10 meet the oracle; 9/9 controls match |

Producer: the checkout's `omega` at opt-level 1 (`omega --build-profile` reports `opt_level: 1`), revision `f98259e86dd2` for the native rows and `67b7c6dfba3` for the agreement rows, host macOS ARM64.
