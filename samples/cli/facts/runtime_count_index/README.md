# Runtime Count Index

A byte view is indexed by a runtime count. Bytes `[9, 8, 7, 6]` indexed at
`1` produce `8`. Exit 0 means that read matched. A mismatch traps.

The count is one value-binder argument. Each directory is one place, the same
row as `samples/cli/facts/length/`: a local, a field, a parameter, a call
result, a match join, a const name, another state, another machine, and
another target body.

| Place | Positive Project | Reject Twin (invalid) | Out of Bounds Control | Transport Mechanism |
| --- | --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | — | canonical root witness indexing byte view at index 1 |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | [`local/controls/out_of_bounds/main.omg`](local/controls/out_of_bounds/main.omg) | the count is copied into a local variable |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | [`field/controls/out_of_bounds/main.omg`](field/controls/out_of_bounds/main.omg) | the count is stored in and read from a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | [`parameter/controls/out_of_bounds/main.omg`](parameter/controls/out_of_bounds/main.omg) | the count arrives as an explicit parameter with bounded contract |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | [`call_result/controls/out_of_bounds/main.omg`](call_result/controls/out_of_bounds/main.omg) | contracted helper call result supplies the bounded count |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | [`match_join/controls/out_of_bounds/main.omg`](match_join/controls/out_of_bounds/main.omg) | both match arms preserve and join the runtime count |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | [`const_name/controls/out_of_bounds/main.omg`](const_name/controls/out_of_bounds/main.omg) | named constant supplies the count to the value binder |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | [`other_state/controls/out_of_bounds/main.omg`](other_state/controls/out_of_bounds/main.omg) | a successor state receives the count across a transition |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | [`other_machine/controls/out_of_bounds/main.omg`](other_machine/controls/out_of_bounds/main.omg) | a forwarding machine supplies the count to the consumer machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | [`target_body/controls/out_of_bounds/main.omg`](target_body/controls/out_of_bounds/main.omg) | target-scoped machine bodies execute on each hosted platform |

## Execution

Every positive place (and the root `main.omg`) binds `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`.
`omega run --both` reports Terminal Psi interpreter and native execution agreement:

```sh
omega run --both samples/cli/facts/runtime_count_index/main.omg
omega run --both samples/cli/facts/runtime_count_index/local/main.omg
omega run --both samples/cli/facts/runtime_count_index/field/main.omg
omega run --both samples/cli/facts/runtime_count_index/parameter/main.omg
omega run --both samples/cli/facts/runtime_count_index/call_result/main.omg
omega run --both samples/cli/facts/runtime_count_index/match_join/main.omg
omega run --both samples/cli/facts/runtime_count_index/const_name/main.omg
omega run --both samples/cli/facts/runtime_count_index/other_state/main.omg
omega run --both samples/cli/facts/runtime_count_index/other_machine/main.omg
omega run --both samples/cli/facts/runtime_count_index/target_body/main.omg
```

Every positive place and the root witness observe `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The exit code `0` is not vacuous. Corrupting the array bytes (e.g. `[9, 7, 7, 6]` instead of
`[9, 8, 7, 6]`) or changing the expected result from `8` makes the positive branch fail:
the transition guard takes the `bad()` path and executes `crash Trap;`. Both engines
agree with `native signal 5` and `interp crash: Trap`, exiting non-zero.

### Invalid controls and reject twins

Each place has an `invalid/` twin and a `controls/out_of_bounds/` twin that keep the exact same
program structure but supply an out-of-bounds count (`count == 4 == items.len`, or drop the
guard proof). The `read_at` contract `requires Index < items.len` cannot be satisfied, and proof
checking strictly refuses compilation with exit code 1 and diagnostic:
`error: cannot prove requires contract for call read_at... (needs fact Index < items.len here)`.

The unknown-field control is `tests/omega/fail/generics/value_binder_unknown_field`. A `const` binder
still rejects a runtime name, as in `tests/omega/fail/generics/const_generic_runtime_argument`.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Pinned fixtures:**
  - Positive passes: `tests/omega/pass/facts/runtime_count_index_through_<place>_exit` (9 places pinned, each byte-identical to `<place>/main.omg`, recorded in `tests/omega/corpus_native_macos_arm64.txt` with native exit 0).
  - Negative controls: `tests/omega/fail/facts/runtime_count_index_through_<place>_rejected` (9 places pinned under the canonical reject name, each byte-identical to `<place>/controls/out_of_bounds/main.omg` and `<place>/invalid/main.omg`, rejecting with `cannot prove requires contract`).

### Specification and Lowering Layers

1. **Establishment:** The runtime value-binder count `Index: u32` and the entry contract `requires Index < items.len` on generic `read_at<Index: u32>(items: &[u8]) -> u8` establish memory-safe indexing into the 4-byte slice `[9u8, 8u8, 7u8, 6u8]` at index `1` producing `8u8`. Safe access is established by the static contract discharge at the call site, without any post-hoc caller equality guard (`transition x == 100`) after the operation completes.
2. **Identity and lifetime:** The dependent value-binder `Index` names the exact runtime scalar value passed at the call site while the borrowed byte view `&[u8]` retains its length `4` and backing storage lifetime throughout the function activation. No mutating write intervenes to invalidate the slice's length or buffer identity between contract discharge and element access.
3. **Composition:** The runtime count and its bound proof travel through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into `read_at<index>(items)`. Each place's positive `<place>/main.omg` and reject twin `<place>/invalid/main.omg` differ only in the count's value or missing bound proof (`count == 4` vs `count == 1`, or dropping the `requires` contract), isolating the exact fact under test.
4. **Soundness boundaries:** All 18 reject controls (`invalid/` and `controls/out_of_bounds/` across all 9 places) reject at source checking with `cannot prove requires contract` when `Index < items.len` is not proved. Value-binder arguments remain strictly distinguished from compile-time `const` generics: runtime values cannot masquerade as compile-time constants, and unknown fields/out-of-domain binders are caught before code emission.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Runtime value-binder arguments lower cleanly to executable value lanes in canonical target-neutral `PSITERM\0`. Proof-only contracts (`requires Index < items.len`) are erased after proof verification, while byte-slice view descriptors (`&[u8]`) with base pointer and length are preserved. Both engines execute the identical program in `omega run --both` with matching exit codes.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles uniformly across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). The backing array `[u8; 4]` and slice descriptor `&[u8]` validate target layout plans (`size_of` 1-byte elements, 8-byte pointer + 8-byte length view). `CallPlan` and `StatePlan` allocate runtime value-binder arguments into standard ABI parameter registers or stack locations according to calling conventions.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects byte-indexed load instructions (`movzx` with base+index displacement on `x86_64`, `ldrb` on `arm64`), assigns register homes and disjoint stack-frame storage for the `[u8; 4]` backing array, and inserts entry prologues and multi-return epilogues respecting the `StatePlan` frame ceiling.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve for the local array buffer, basic blocks and branch targets relax with offset reflow, machine-state footprint evidence validates transitive usage, and direct native binary emission produces functional executables agreeing with Terminal Psi (`omega run --both` exit 0).
