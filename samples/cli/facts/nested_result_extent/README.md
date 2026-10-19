# Nested Result Extent

Fact: Two contracted minima bound the exclusive end of a mutable subslice by the same view's length: `minimum(minimum(count, buffer.len), 2147483647)`, forming `&mut buffer[0..capacity]` and observing `prefix.len`.

The root witness and nine place witnesses call `trial(&mut bytes[..], &other[..], 13)` and `trial(&mut bytes[..], &other[..], 0)`.
Two contracted `minimum` machine invocations compute `capacity: u64 = minimum(minimum(count, buffer.len), 2147483647)`.
The compiler uses the contracted postconditions (`result <= left && result <= right`) to establish statically that
`capacity <= buffer.len`, enabling the formation of the mutable subslice `&mut buffer[0..capacity]` without runtime bounds checks.

Every positive place (and the root `main.omg`) binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declares `Expected exit: 0`,
and carries an authored `expected_native_exit.txt` containing `0` beside its entry. Both engines
(`omega run --both`) agree: Terminal Psi and native execution report exit 0.

## Places

| Place | Positive Program | Fact-sensitive Reject Twin | Carrier / Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | [`invalid/main.omg`](invalid/main.omg) | canonical root witness computing contracted nested minima and taking mutable prefix `&mut buffer[0..capacity]` |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | local variable `capacity` holds the nested minimum result before slicing `buffer[0..capacity]` |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | nominal record field `bound.end` holds the nested minimum result before slicing `buffer[0..bound.end]` |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | nested minimum result passed as argument to machine parameter `consume(buffer, capacity)` with requires contract `end <= buffer.len` |
| call_result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | nested minimum expression passed directly as subslice range upper bound `0..minimum(...)` without intermediate binding |
| match_join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of match expression compute the nested minimum and join into `capacity` before slicing |
| const_name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | constant `native_cap: u64 = 2147483647` used as outer clamping limit in nested minimum before slicing |
| other_state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | state transition arrival parameter `consume(buffer: &mut [u8], end: u64)` with requires contract `end <= buffer.len` consumes nested minimum |
| other_machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | associated machine method `Bound::consume(bound, buffer)` with requires contract `bound.end <= buffer.len` receives record holding nested minimum |
| target_body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | four target-scoped machine bodies (`linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`) perform nested minimum and subslice |

## Execution

Every positive place has an authored native exit oracle of zero and verifies that `first == 8 && second == 0`,
crashing `Trap` otherwise. Run with `omega run --both`:

```sh
omega run --both samples/cli/facts/nested_result_extent/main.omg
omega run --both samples/cli/facts/nested_result_extent/local/main.omg
omega run --both samples/cli/facts/nested_result_extent/field/main.omg
omega run --both samples/cli/facts/nested_result_extent/parameter/main.omg
omega run --both samples/cli/facts/nested_result_extent/call_result/main.omg
omega run --both samples/cli/facts/nested_result_extent/match_join/main.omg
omega run --both samples/cli/facts/nested_result_extent/const_name/main.omg
omega run --both samples/cli/facts/nested_result_extent/other_state/main.omg
omega run --both samples/cli/facts/nested_result_extent/other_machine/main.omg
omega run --both samples/cli/facts/nested_result_extent/target_body/main.omg
```

All 10 samples report `native exit 0` and `interp exit: 0`.

### Run record

Measured on producer revision `8d0f98ca089` (`--build-profile` reports
`opt_level` 1), host `macos_arm64`, library root `<worktree>/source/library`:

```sh
python3 tools/sample_native_gate.py --filter nested_result_extent
# 10/10 samples meet their documented oracle on macos_arm64
# 10/10 intended-invalid controls match their authored rejection

python3 tools/corpus_gate.py --native --filter facts/nested_result_extent
# clean — 11 fixtures ({'built': 11}); executed 11 (exit codes {'0': 11})
```

The golden rows live in `tests/omega/corpus_native_macos_arm64.txt`: the nine
`pass/facts/nested_result_extent_through_<place>_exit` fixtures plus
`nested_result_extent_after_disjoint_write_exit` and
`nested_result_extent_captured_scalar_exit`, each stamped `built <ms> exit:0`.
The two sample-side sibling witnesses are scored by the same gate; the
sample-side `expected_native_exit.txt` alone is inert for
`sample_native_gate.py` without an adjacent `input.txt`, so the corpus golden,
not that file, is the measurement.

The oracle discriminates. Shifting the extent carried through the nested result
by one (`prefix.len + 1u64`, which leaves checking green because the proved
bound is untouched) traps in **both** engines — `native signal 5` and
`interp crash: Trap (host presentation agrees)` — while the one-token control
reports `native exit 0` / `interp exit: 0`. Substituting `other.len` for
`buffer.len` fails earlier still, at proof checking, with
`cannot prove subslice range end bound`.

Each place carries a fact-sensitive reject twin at `<place>/invalid/` (and root `invalid/`) that
replaces `buffer.len` with an unrelated `other.len`. That bound cannot justify accessing `buffer`;
the larger other view must not donate its extent. All reject twins fail closed at proof checking
before native execution:

```sh
omega --check samples/cli/facts/nested_result_extent/<place>/invalid/main.omg
```

Rejection controls reject with:
- Subslice range end check (root, `local`, `field`, `call_result`, `match_join`, `const_name`, `target_body`):
  `cannot prove subslice range end bound ... is within unknown slice length of buffer`
- Parameter/call requires contract check (`parameter`, `other_state`, `other_machine`):
  `cannot prove requires contract for call consume from trial ... buffer.len`

## Requirements and Evidence

- **Parent outcome:** `FILESYSTEM-PLATFORM-SERVICES` (`TASKS.md`).
- **Requirement established:** Exact descriptor identity across ordered call results: two contracted minima bound the exclusive end of a mutable subslice by the same view's length (`minimum(minimum(count, buffer.len), 2147483647)`), and only the same view's extent may justify its mutable prefix across all nine places.
- **Pinned fixtures:**
  - Pass: `tests/omega/pass/facts/nested_result_extent_through_*_exit`, `tests/omega/pass/facts/nested_result_extent_after_disjoint_write_exit`, and `tests/omega/pass/facts/nested_result_extent_captured_scalar_exit`.
  - Fail: `tests/omega/fail/facts/nested_result_extent_through_*_rejected`, `tests/omega/fail/facts/nested_result_extent_disjoint_write_wrong_view_rejected`, `tests/omega/fail/facts/nested_result_extent_later_sibling_bound_rejected`, `tests/omega/fail/facts/nested_result_extent_one_bad_match_arm_rejected`, and `tests/omega/fail/facts/nested_result_extent_replaced_descriptor_rejected`.
- **Authority:** [Structural access](wiki/spec/terminal-psi/structural_access.md) and [Loans](wiki/spec/terminal-psi/loans.md).

Full 8-layer specification and lowering evidence:

- **Establishment:** The `minimum` machine specifies `ensures result <= left && result <= right`. Nested composition `minimum(minimum(count, buffer.len), 2147483647)` transitively establishes `capacity <= buffer.len` by virtue of contracted postconditions. Subslice formation `&mut buffer[0..capacity]` directly consumes this established inequality to prove that the slice range `0..capacity` is within `buffer.len` without requiring runtime bound assertions or post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The mutable slice descriptor `buffer: &mut [u8]` maintains exact identity throughout the nested minimum calls and subslice projection. The fact `capacity <= buffer.len` specifically and exclusively describes the extent of `buffer`, not unrelated descriptors such as `other`. Overlapping writes or modifications to other storage (as verified in the disjoint-write fixtures) preserve this extent guarantee, while descriptor substitution or view confusion invalidates the subslice bound.
- **Composition:** The bound composes across calls (contract substitution `minimum` -> `trial`), across local variables (`local`), through nominal struct fields (`Bound.end`), through call result expressions (`call_result`), across match branch convergence joins (`match_join`), via named constants (`const_name`), across state transitions (`other_state`), across associated machine invocations (`other_machine`), and through target-scoped body candidates (`target_body`). In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differs solely in referencing `other.len` instead of `buffer.len`, demonstrating that only the matching view's length establishes the subslice precondition.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins (and root `invalid/main.omg`) fail closed with targeted compile-time diagnostics (`cannot prove subslice range end bound ... is within unknown slice length` or `cannot prove requires contract for call consume from trial: ... buffer.len`). Unrelated slice lengths cannot donate extent to a distinct descriptor, preserving borrow and slice projection safety across the language boundary.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Contracts and subslice range checks are verified statically during checking. Slicing lowers into target-neutral Terminal Psi (`PSITERM\0`) pointer arithmetic and length calculations with explicit affine cleanup and loan restoration windows. Terminal Psi interpreter executes identically to native code (`omega run --both`), observing `first == 8 && second == 0`.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Slice views `&mut [u8]` lower to pointer-and-length fat pointers (16 bytes, 8-byte alignment). Call plans pass fat pointer components and integer parameters in standard ABI registers (e.g. `rcx`/`rdx`/`r8` on Windows x86-64, `rdi`/`rsi`/`rdx` on System V x86-64, `x0`-`x2` on ARM64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowers comparisons, conditional moves/branches for `minimum`, subslice pointer offsets, and length extractions. Stack-frame activation storage accommodates locals and spill slots with validated worst-case stack usage (WCSU).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Emits native images (PE/ELF/Mach-O) that execute and exit with code 0 on all hosted platforms, in full differential agreement with the Terminal Psi reference interpreter.

## What remains open

Full filesystem platform services (`FILESYSTEM-PLATFORM-SERVICES`), directory/read-link buffer clamping in OS adapter layers, and Windows raw breadth/metadata fixtures remain governed under the parent task roadmap.
