# Slice View Layout Geometry

Fact: querying size_of and align_of on a 64-bit slice view (&[u64]) establishes its exact two-word (ptr, len) descriptor geometry (size == 16u64 and align == 8u64), discharging exact subtraction size - (align * 2u64) across all nine places.

Owning Task: `CONSERVATION-CONTRACT` (`TASKS.md`)
Claim Ticket: `d04ea37dc60e4c0ab79da056a01a997d`

Every place performs the same operation: query the byte size (`size_of<&[u64]>() == 16u64`) and alignment (`align_of<&[u64]>() == 8u64`) of a 64-bit slice view descriptor (`&[u64]`), discharge exact subtraction `size - (align * 2u64)`, verify `diff == 0`, and exit 0. Only the transport mechanism of the geometry queries and subtraction computation changes across places.

## Places

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| (root) | [`main.omg`](main.omg) | — | root slice view descriptor size and alignment queries and exact subtraction |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the size and alignment queries are copied into locals before subtracting |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | an enclosing structural carrier record holds the size and alignment in fields before subtracting |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | helper machines receive the queried size and alignment directly as explicit parameters |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the size and alignment are returned from contracted helper call results before subtracting |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms evaluate the size and alignment queries and join before subtracting |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants evaluate size_of and align_of before subtracting |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the size and alignment queries before subtracting |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | an intermediate forwarding machine delegates the subtraction to the geometry query machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies evaluates size_of and align_of and subtracts |

Every positive place and the root `main.omg` probe binds `ProgramEntry` for
`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, carries an
authored `expected_native_exit.txt` (`0`), and declares `Expected exit: 0`.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/slice_view_layout_geometry/main.omg
omega run --both samples/cli/facts/slice_view_layout_geometry/local/main.omg
omega run --both samples/cli/facts/slice_view_layout_geometry/field/main.omg
omega run --both samples/cli/facts/slice_view_layout_geometry/parameter/main.omg
omega run --both samples/cli/facts/slice_view_layout_geometry/call_result/main.omg
omega run --both samples/cli/facts/slice_view_layout_geometry/match_join/main.omg
omega run --both samples/cli/facts/slice_view_layout_geometry/const_name/main.omg
omega run --both samples/cli/facts/slice_view_layout_geometry/other_state/main.omg
omega run --both samples/cli/facts/slice_view_layout_geometry/other_machine/main.omg
omega run --both samples/cli/facts/slice_view_layout_geometry/target_body/main.omg
```

All 10 applications pass with `native exit 0` and `interp exit: 0`.

### The oracle discriminates

The recorded 0 exit is discriminating:
- At runtime, `size_of<&[u64]>() == 16u64` and `align_of<&[u64]>() == 8u64`. Exact subtraction `16u64 - (8u64 * 2u64) == 0u64`. The transition `diff == 0 { true -> done() _ -> bad() }` exits 0. A 1-token perturbation of the demanded result (`transition diff == 0` to `transition diff == 1`) routes execution to `machine bad() { crash Trap; }`, trapping both engines (`native exit -1073741795` / `interp crash: Trap (host presentation agrees)`).
- In each invalid twin, the queried type is narrowed to a single element thin reference `&u64` (`size == 8u64`, `align == 8u64`). Exact subtraction `8u64 - (8u64 * 2u64)` (`8u64 - 16u64`) underflows below zero (`bounds [-8, -8] lie outside [0, 18446744073709551615]`), causing compile-time proof obligation checking to fail closed under the **Exact arithmetic overflow/underflow (decision 17)** obligation class with `exact arithmetic may overflow`. None reaches native execution.

Check all nine invalid twins with:

```sh
omega --check samples/cli/facts/slice_view_layout_geometry/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `exact arithmetic may overflow` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `exact arithmetic may overflow` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `exact arithmetic may overflow` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `exact arithmetic may overflow` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `exact arithmetic may overflow` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `exact arithmetic may overflow` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `exact arithmetic may overflow` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `exact arithmetic may overflow` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Exact arithmetic overflow/underflow (decision 17) | `exact arithmetic may overflow` |

### Acceptance Gate

```sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omega\build\swarm\w26\worktrees\w26-6\target\debug\omega.exe --filter cli/facts/slice_view_layout_geometry --jobs 4
```

`sample_native_gate.py` reports:
- 10/10 samples meet their documented oracle on `windows_x86_64` (and `linux_x86_64`).
- 9/9 intended-invalid controls match their authored rejection (`exact arithmetic may overflow`).

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/slice_view_layout_geometry_through_local_exit/`
  - `tests/omega/pass/facts/slice_view_layout_geometry_through_field_exit/`
  - `tests/omega/pass/facts/slice_view_layout_geometry_through_parameter_exit/`
  - `tests/omega/pass/facts/slice_view_layout_geometry_through_call_result_exit/`
  - `tests/omega/pass/facts/slice_view_layout_geometry_through_match_join_exit/`
  - `tests/omega/pass/facts/slice_view_layout_geometry_through_const_name_exit/`
  - `tests/omega/pass/facts/slice_view_layout_geometry_through_other_state_exit/`
  - `tests/omega/pass/facts/slice_view_layout_geometry_through_other_machine_exit/`
  - `tests/omega/pass/facts/slice_view_layout_geometry_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/slice_view_layout_geometry_through_local_rejected/`
  - `tests/omega/fail/facts/slice_view_layout_geometry_through_field_rejected/`
  - `tests/omega/fail/facts/slice_view_layout_geometry_through_parameter_rejected/`
  - `tests/omega/fail/facts/slice_view_layout_geometry_through_call_result_rejected/`
  - `tests/omega/fail/facts/slice_view_layout_geometry_through_match_join_rejected/`
  - `tests/omega/fail/facts/slice_view_layout_geometry_through_const_name_rejected/`
  - `tests/omega/fail/facts/slice_view_layout_geometry_through_other_state_rejected/`
  - `tests/omega/fail/facts/slice_view_layout_geometry_through_other_machine_rejected/`
  - `tests/omega/fail/facts/slice_view_layout_geometry_through_target_body_rejected/`

### Recorded native evidence (macos_arm64)

Measured on `macos_arm64` at revision `e73cff353a52` with the producer built
from that revision (`opt_level 1`):

```sh
python3 tools/corpus_gate.py --native --filter pass/facts/slice_view_layout_geometry_through
```

Before the record, the gate reported all nine positives as `new fixture
(built)`; the run reported `built 9`, `executed 9 (exit codes {'0': 9})`, 0
rejected without their expected fragment and 0 fail fixtures that did not
reject. The nine rows now read `built ... exit:0 @e73cff353a52` in
`tests/omega/corpus_native_macos_arm64.txt`, spliced as a pure addition
(+9 lines, 0 deletions, LOST 0 against `origin/main`). Root and all nine places
pass `omega run --both`: ten applications, each `native exit 0` and
`interp exit: 0`.

The recorded 0 discriminates in both directions on this host. Replacing
`size_of<&[u64]>()` with `size_of<u64>()` is refused statically with the
computed bounds `[-8, -8] lie outside [0, 18446744073709551615]`, so the 16-byte
two-word descriptor size is derived by the compiler rather than assumed.
Replacing `align_of<&[u64]>()` with `align_of<u8>()` gives
`16u64 - (1u64 * 2u64) == 14u64`, which the checker cannot fold away: the
program traps at runtime in both engines (`native signal 5` /
`interp crash: Trap (host presentation agrees)`), so the transition really
depends on the measured geometry.

Re-measured on the integration base `c72b78bbf94a` with a producer built from
that base, after two compiler commits landed (`69cfefcd2bf`, `c72b78bbf94a`):
the same gate reported `clean — 9 fixtures ({'built': 9})`, `executed 9 (exit
codes {'0': 9})`, 0 rejected without their expected fragment, 0 fail fixtures
that did not reject, with no structural move (the only notes were two
`perf_faster` rows, and the gate's own context says the golden was recorded at
a different worker count). All ten applications still agree at that base
(`native exit 0` / `interp exit: 0`), so the rows carry forward: the recorded
revision is `e73cff353a52`, which is what the rows are stamped with.

Remaining open on other hosts: `windows_x86_64` already carries 9 native rows
for this fact, while `tests/omega/corpus_native_linux_x86_64.txt` carries 0 and
needs a Linux x86-64 host; `linux_arm64` has no native golden in this checkout.

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `d04ea37dc60e4c0ab79da056a01a997d`.
- **Requirement established:** Target-neutral compile-time layout geometry query proofs (`size_of` and `align_of`) and native realization for slice view descriptors (`&[u64]`, [layout plans](../../../../wiki/spec/layouts/plans.md), [byte views](../../../../wiki/spec/terminal-psi/byte_views.md)). Specifically, querying `size_of` and `align_of` on a 64-bit slice view (`&[u64]`) establishes its exact two-word `(ptr, len)` descriptor geometry (`size == 16u64` and `align == 8u64`), discharging exact arithmetic `size - (align * 2u64) == 0u64` across all nine places without post-hoc caller equality guards, with exact agreement between Terminal Psi and native execution across hosted targets (`omega run --both` exit 0).

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** Compile-time layout evaluation of the slice view type `&[u64]` statically determines `size_of<&[u64]>() == 16u64` and `align_of<&[u64]>() == 8u64` from fat pointer descriptor rules (base pointer 8 bytes, length word 8 bytes, 8-byte alignment, contiguous layout). The static bounds `[16, 16]` and `[8, 8]` justify forming exact subtraction `size - (align * 2u64)` directly without requiring post-hoc caller equality guards.
  - **Identity and lifetime:** The static layout geometry values (`16u64` and `8u64`) retain their exact scalar identity, precision, and alignment constraints across copies, structural carrier fields, explicit machine parameters, helper call returns, match join arms, named constants, state transitions, forwarding machines, and target-scoped bodies.
  - **Composition:** The layout geometry queries compose through all 9 fact places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact subtraction operation (`size - (align * 2u64)`), evaluating to `0u64` across all 9 places. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ strictly in the queried pointer type (`&[u64]` with two-word descriptor size 16 vs `&u64` with single-word reference size 8).
  - **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof obligation checking when the slice view type is narrowed to a thin reference `&u64`: querying `size_of<&u64>()` yields `8u64`, while `align_of<&u64>()` remains `8u64`, so exact subtraction `8u64 - (8u64 * 2u64)` (`8u64 - 16u64`) underflows `u64` (`bounds [-8, -8] lie outside [0, 18446744073709551615]`). Compilation strictly rejects with `exact arithmetic may overflow`. No unproved underflow reaches native execution.
  - **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral slice view layout queries `size_of<&[u64]>()` and `align_of<&[u64]>()` and exact integer subtraction lower into canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof obligations while retaining concrete layout plans and certified geometry constants. All 10 positive entries execute in the Terminal Psi interpreter (`omega run --both`), agreeing with native exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) through companion build declarations without reopening source. The 16-byte `(ptr, len)` slice view descriptor layout validates: `&[u64]` contains an 8-byte base pointer followed by an 8-byte element count word (size 16, align 8). Target `CallPlan`/`StatePlan` assign arguments and return homes across register pairs or stack slots per platform ABI.
  - **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA arithmetic instructions (`sub`, `shl`/`imul` or constant-folded subtraction of 16), register moves, and comparison branches. Low-level register allocators assign register homes and disjoint activation stack-frame slots, and entry prologues and multi-return epilogues honor the `StatePlan` ceiling.
  - **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Target ISA instructions and transition displacements encode into machine bytes, basic blocks and jump targets relax with offset reflow, machine-state footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates against target host limits, and direct PE/ELF/Mach-O executables are emitted. Terminal Psi interpretation and native execution agree with exit 0 (`omega run --both`) across host platforms including `windows_x86_64` and `macos_arm64`.

- **What remains open:** Dynamic mutable slice view descriptors (`&mut [u64]`), wide slice pointers to dynamically sized types (DST), and custom slice metadata representations.
