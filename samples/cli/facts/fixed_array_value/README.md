# Fixed array value carrier and in-bounds index preservation

A fixed-extent array `[u64; 2]` preserves its in-bounds index contract `index < 2`
and independent element values across all nine places.

Owning Task: `CONSERVATION-CONTRACT` / `TERMINAL-CONTENT-CLAIMS`
Reserved Claim Ticket: `26cce69dcf0e413baec3f321436e739b`

## Requirements and Evidence

Owning Task: `CONSERVATION-CONTRACT` / `TERMINAL-CONTENT-CLAIMS` ([TASKS.md](../../../../TASKS.md))
Reserved Claim Ticket: `26cce69dcf0e413baec3f321436e739b`

- **Establishment:** Constructing the fixed-size array `[u64; 2]` (`[19, 42]`) and entry precondition `requires index < 2` (or record constraint `where index < 2` / named constant `LIMIT = 2`) establish proven in-bounds element access and compile-time extent `2` directly without any post-hoc caller equality guard.
- **Identity and lifetime:** Independent element values `values[0] == 19` and `values[1] == 42` and their static index bounds retain their exact positional identity across value copies, moves, and projections while unrelated writes do not invalidate unchanged elements across all 9 places.
- **Composition:** Fixed-size array value facts compose through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming index operation (`values[index]`); `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in weakening the index contract (e.g. `index <= 2` or `LIMIT = 3`), leaving the index access unproved without modifying caller control flow.
- **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time (`cannot prove index ... is within length 2`), strictly preventing out-of-bounds array element indexing.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Fixed-size array aggregate construction `[19, 42]` and element indexing lower into canonical target-neutral `PSITERM\0` while erasing proof-only obligations, executing with identical semantics in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), fixed-size array layout geometry (`size_of == 2 * 8 = 16`, `align_of == 8`) evaluates and validates, and `CallPlan`/`StatePlan` lower `[u64; 2]` carriers across target registers, stack slots, and indirect `sret` homes.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects constant and scaled element loads/stores and integer arithmetic, assigns register homes and disjoint stack-frame activation storage, and inserts entry prologues and epilogues honoring the `StatePlan`.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements and element offsets resolve, basic blocks and branch offsets relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across hosted targets including `windows_x86_64` and `macos_arm64`.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root (anchor) | [source](main.omg) | - | Root reference row reading array elements with an in-bounds index. |
| local | [source](local/main.omg) | [source](local/invalid/main.omg) | The array and bounded index are copied into locals before indexing. |
| field | [source](field/main.omg) | [source](field/invalid/main.omg) | A record field carries the bounded index alongside the fixed-extent array. |
| parameter | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) | The reading machine receives the bounded index and array directly as parameters. |
| call result | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) | The index arrives from a helper call with an ensures result < 2 contract. |
| match join | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) | Both branches of a match expression produce in-bounds indices. |
| const name | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) | A named constant bounds the index contract for the fixed-extent array. |
| other state | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) | A transition to a successor machine forwards the bounded index and array. |
| other machine | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) | A forwarding machine transports the bounded index and array to the reader. |
| target body | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) | Each of the four target bodies reads the array element using the bounded index. |

Each place is an application that binds `ProgramEntry` for `windows_x86_64`,
`linux_x86_64`, `macos_arm64` and `linux_arm64` in its `build.omg`, declares
`Expected exit: 0`, carries a measured `expected_native_exit.txt`, and passes
`omega run --both`:

```sh
omega run --both samples/cli/facts/fixed_array_value/local/main.omg
omega run --both samples/cli/facts/fixed_array_value/field/main.omg
omega run --both samples/cli/facts/fixed_array_value/parameter/main.omg
omega run --both samples/cli/facts/fixed_array_value/call_result/main.omg
omega run --both samples/cli/facts/fixed_array_value/match_join/main.omg
omega run --both samples/cli/facts/fixed_array_value/const_name/main.omg
omega run --both samples/cli/facts/fixed_array_value/other_state/main.omg
omega run --both samples/cli/facts/fixed_array_value/other_machine/main.omg
omega run --both samples/cli/facts/fixed_array_value/target_body/main.omg
```

Each place has an `invalid/` twin that weakens the index bound (e.g. to `index <= 2` or `LIMIT = 3`),
so the array index access is no longer proved in bounds. Every twin rejects at compile time with
`cannot prove index ... is within length 2`. Check a twin with `omega --check <place>/invalid/main.omg`;
`sample_native_gate --facts` reports 9/9 intended-invalid controls matching their authored rejection.

The row is pinned by `tests/omega/{pass,fail}/facts/fixed_array_value_through_*` fixtures; every
positive records native exit 0. `corpus_gate --filter facts/fixed_array_value` is clean:
18 fixtures — 9 checked, 9 rejected — with 0 rejected without their expected fragment and 0 fail
fixtures failing to reject.
