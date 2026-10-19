# Fact: Mutable Slice View Element Write Bound and Extent Preservation

> Writing an element through a mutable slice view (`&mut [u64]`) whose length is
> guarded (`view.len >= 2u64`) proves safe indexed element mutation at
> `view[1u64]` while preserving the slice extent lower bound (`view.len >= 2u64`)
> to discharge exact subtraction `view.len - 2u64` across all nine places. The
> written value is observable afterwards through the same view.

## Places & witnesses

| Place | Witness (main.omg) | Reject twin (invalid/main.omg) | Note |
|---|---|---|---|
| root | [main.omg](main.omg) | - | Root package probe exercising mutable slice view write and extent subtraction |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | mutable slice view is transported through an intermediate local binding |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | mutable slice view is stored within a nominal record field (`Holder`) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | guarded mutable slice view is operated on directly through its parameter binding |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | helper call returns the guarded mutable slice view with preserved length contract |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | mutable slice view joins across conditional match branches |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | named constant `MIN_LEN: u64 = 2` defines the extent guard obligation |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | successor state `active(view: &mut [u64])` receives and mutates the view |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | forwarding machine passes the mutable view through an intermediate call |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | target-scoped implementations across all four platforms perform mutation and extent verification |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
windows_x86_64, linux_x86_64, macos_arm64, and linux_arm64 and declares
`Expected exit: 0`. Every place's `build.omg` was read before pinning: this
construct is not target-gated, so four hosted targets is its real contract.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/mutable_slice_element_write_bound/main.omg
omega run --both samples/cli/facts/mutable_slice_element_write_bound/local/main.omg
omega run --both samples/cli/facts/mutable_slice_element_write_bound/field/main.omg
omega run --both samples/cli/facts/mutable_slice_element_write_bound/parameter/main.omg
omega run --both samples/cli/facts/mutable_slice_element_write_bound/call_result/main.omg
omega run --both samples/cli/facts/mutable_slice_element_write_bound/match_join/main.omg
omega run --both samples/cli/facts/mutable_slice_element_write_bound/const_name/main.omg
omega run --both samples/cli/facts/mutable_slice_element_write_bound/other_state/main.omg
omega run --both samples/cli/facts/mutable_slice_element_write_bound/other_machine/main.omg
omega run --both samples/cli/facts/mutable_slice_element_write_bound/target_body/main.omg
```

Measured on macos_arm64 with the optimized producer: all 10 applications report
`native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The backing storage consists of 3 elements `[10u64, 20u64, 30u64]`, loaned as a mutable slice view `&mut items[..]`.
- Mutating index 1 (`view[1] = 45u64`) writes through the exclusive borrow into backing storage while preserving the static slice extent knowledge `view.len >= 2`.
- Exact subtraction `view.len - 2u64` evaluates to `1u64` and is statically proved not to underflow under the preserved extent bound.
- The caller verifies both that `items[1] == 45u64` (in-place mutation observed) and `diff == 1u64` (extent subtraction correct), branching to `pass()` (exit 0) or `fail()` (`crash Trap;`, exit 255) on discrepancy.

Both halves were probed on macos_arm64. The probes are recorded here rather than
committed, because each is a deliberately corrupted copy of a place:

| Probe | Corruption | Outcome |
| --- | --- | --- |
| written value observable | caller observes `items[1] == 44u64` instead of `45u64` | reached runtime and trapped: `native signal 5` / `interp crash: Trap (host presentation agrees)` |
| written element bound | `view[2] = 45u64` under `requires view.len >= 2` | refused statically before native production: ``cannot prove index `2` is within unknown slice length of `view` `` |
| runtime-variable index | index seeded from a provider parameter bounded `<= 2`, three-element backing | refused statically: ``cannot prove index `index` is within unknown slice length of `view` `` |

The first row is the runtime half of the discrimination proof: the value written
through the mutable view is exactly what the consumer reads afterwards, so
corrupting the observation traps in both engines. The second and third rows are
the static half: the proved bound is what licenses the write, and the checker
refuses before native production, so an unprovable index never reaches an
executable.

The third row also records a real boundary of the current mechanism, not a
defect in this row: a write index that is genuine runtime data is refused even
when its range is constrained by `requires`, because the unknown-length index
proof accepts only a folded constant, a proven carried fact, an ensured
call-result bound, or an ordering fact. This fact therefore pins **statically
proved** element bounds; it does not claim dynamic-bounds-checked runtime
indexing, which stays open below.

Each invalid twin attempts an out-of-bounds mutation `view[2] = 45u64` under
`requires view.len >= 2`. Because a slice of length 2 only guarantees indices 0
and 1, the compiler rejects compilation with `cannot prove index`. None reaches
native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/mutable_slice_element_write_bound/<place>/invalid/main.omg
```

### Acceptance Gate

```sh
python3 tools/sample_native_gate.py --filter cli/facts/mutable_slice_element_write_bound
python3 tools/corpus_gate.py --native --filter facts/mutable_slice_element_write_bound
```

Measured on macos_arm64 with the optimized producer:

- `sample_native_gate.py`: **10/10** samples meet their documented oracle and
  **9/9** intended-invalid controls match their authored rejection
  (`cannot prove index`).
- `corpus_gate.py --native`: **clean** — 9 fixtures `({'built': 9})`, executed 9
  with exit codes `{'0': 9}`, 0 fail fixtures did not reject, and 9 golden rows
  stamped in `tests/omega/corpus_native_macos_arm64.txt`.

### Pinned fixtures

Nine positive places and nine reject twins are pinned under `tests/omega/`. The
fixture family is `mutable_slice_element_write_bound_through_<place>_exit` /
`_unproved`, which does not spell the sample folder name:

| Place | Positive fixture | Reject twin |
| --- | --- | --- |
| local | [`..._through_local_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_local_exit/main.omg) | [`..._through_local_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_local_unproved/main.omg) |
| field | [`..._through_field_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_field_exit/main.omg) | [`..._through_field_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_field_unproved/main.omg) |
| parameter | [`..._through_parameter_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_parameter_exit/main.omg) | [`..._through_parameter_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_parameter_unproved/main.omg) |
| call result | [`..._through_call_result_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_call_result_exit/main.omg) | [`..._through_call_result_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_call_result_unproved/main.omg) |
| match join | [`..._through_match_join_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_match_join_exit/main.omg) | [`..._through_match_join_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_match_join_unproved/main.omg) |
| const name | [`..._through_const_name_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_const_name_exit/main.omg) | [`..._through_const_name_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_const_name_unproved/main.omg) |
| another state | [`..._through_other_state_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_other_state_exit/main.omg) | [`..._through_other_state_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_other_state_unproved/main.omg) |
| another machine | [`..._through_other_machine_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_other_machine_exit/main.omg) | [`..._through_other_machine_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_other_machine_unproved/main.omg) |
| another target body | [`..._through_target_body_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_target_body_exit/main.omg) | [`..._through_target_body_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_target_body_unproved/main.omg) |

Each twin differs from its positive only in the mutated index (1 vs. 2) and the
header's oracle declaration; no caller guard differs.

## Requirements and Evidence

- **Parent outcome:** TERMINAL-SLICE-VIEW-VOCABULARY ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place mutable slice view (`&mut [u64]`) extent guarding, safe indexed element mutation with observable write-back, post-write extent preservation, exact subtraction underflow prevention, and native realization agreement.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** A 3-element mutable array `[u64; 3]` lends a mutable slice view `&mut [u64]` via full-extent mutable loan `&mut items[..]`. The contract guard `requires view.len >= 2` establishes the static bound for indexed mutation at index 1 (`1 < view.len`) and the underflow-free lower bound for exact subtraction `view.len - 2u64` directly from the length precondition. No caller-side post-hoc equality guard establishes the fact; the construct's own `requires` contract does.
  - **Identity and lifetime:** The mutable slice view `&mut [u64]` holds exclusive borrow access over backing storage. Element mutation `view[1] = 45u64` updates backing storage elements without altering the slice descriptor's base address or length, so `view.len >= 2` remains invariant across the write. The caller observes the written value through `items[1]` after return, which is exactly the runtime half proven above. Storage identity is maintained across local variables, record fields (`Holder`), parameter bindings, returned views, branch joins, constant bounds, state transitions, machine calls, and target bodies.
  - **Composition:** The mutable slice view composes through all 9 fact places. Callees receive exact references, branch joins unify mutable descriptors, and state transitions preserve length contracts (`requires view.len >= 2`). In all 9 places, positive witnesses and reject twins differ strictly in the mutated index offset (1 vs 2), never in a caller guard.
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time when attempting `view[2] = 45u64`: the compiler detects that index 2 is not provably within bounds under `view.len >= 2` and emits `cannot prove index`. No unverified write or out-of-bounds pointer offset is admitted to native code generation.
  - **Checked-to-Terminal Psi lowering (Psi 05–07):** Mutable slice formation `&mut items[..]`, indexed element write `view[1] = val`, and extent subtraction `view.len - 2u64` lower into canonical target-neutral Terminal Psi (`PSITERM\0`), representing the mutable slice view as a two-word descriptor (pointer + element length). Proof obligations are discharged and erased while preserving memory safety. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), observing `interp exit: 0`.
  - **Provider settlement, layout geometry, and ABI calling plans (Omega 00–02):** `ProgramEntry` settles across all four hosted targets (windows_x86_64, linux_x86_64, macos_arm64, linux_arm64). Layout geometry for fixed array `[u64; 3]` (size_of = 24, align 8) and mutable slice view `&mut [u64]` (size_of = 16, align 8, 8-byte pointer + 8-byte length) validates. Calling plans allocate slice descriptor register pairs (`rcx`/`rdx` on Windows x86_64, `rdi`/`rsi` on Linux x86_64, `x0`/`x1` on AArch64) according to each target's calling convention.
  - **Instruction selection, register homes, and activation frame realization (Omega 03–06):** Backend lowering selects scaled-index memory stores (`mov qword ptr [rcx + 8], 45` on x86_64, `str x-reg, [x0, #8]` on arm64), integer subtraction, and register transfers. Register homes and stack frame activation slots are assigned with bounded worst-case stack usage (WCSU). Measured on macos_arm64: 9 fixtures built and all 9 executed with exit 0.
  - **Form encoding, branch relaxation, relocations, and native image emission (Omega 07–09 + emission):** Target ISA machine instructions encode into native binary bytes, basic blocks relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates within host allocations, and direct PE/ELF/Mach-O images are emitted. Native execution and Terminal Psi interpretation achieve exact outcome agreement (`omega run --both`, exit 0) across all hosted targets.

## What remains open

- Dynamic runtime slice indices for mutable indexed writes under dynamic bounds
  checks (`view[dynamic_idx] = val` with a runtime guard). Probed and refused
  statically today even with a `requires` range constraint; see the third probe
  row above. This is a genuine capability boundary, not a coverage miss.
- Mutable slice subslice window mutation (`&mut view[start..end]`).
- Multi-dimensional nested mutable slice views (`&mut [&mut [T]]`).
- Native execution on windows_x86_64 and linux_x86_64 is not measured on this
  macos_arm64 host; those targets are bound and checked here, and their native
  golden rows belong on their own hosts.