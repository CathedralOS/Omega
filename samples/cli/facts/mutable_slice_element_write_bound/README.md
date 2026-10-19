# Fact: Mutable Slice View Element Write Bound and Extent Preservation

> Writing an element through a mutable slice view (&mut [u64]) whose length is guarded (iew.len >= 2u64) proves safe indexed element mutation at iew[1u64] while preserving the slice extent lower bound (iew.len >= 2u64) to discharge exact subtraction iew.len - 2u64 across all nine places.

## Places & witnesses

| Place | Witness (main.omg) | Reject twin (invalid/main.omg) | Note |
|---|---|---|---|
| root | [main.omg](main.omg) | - | Root package probe exercising mutable slice view write and extent subtraction |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | mutable slice view is transported through an intermediate local binding |
| field | [ield/main.omg](field/main.omg) | [ield/invalid/main.omg](field/invalid/main.omg) | mutable slice view is stored within a nominal record field (Holder) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | guarded mutable slice view is operated on directly through its parameter binding |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | helper call returns the guarded mutable slice view with preserved length contract |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | mutable slice view joins across conditional match branches |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | named constant MIN_LEN: u64 = 2 defines the extent guard obligation |
| another state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | successor state ctive(view: &mut [u64]) receives and mutates the view |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | forwarding machine passes the mutable view through an intermediate call |
| another target body | [	arget_body/main.omg](target_body/main.omg) | [	arget_body/invalid/main.omg](target_body/invalid/main.omg) | target-scoped implementations across all four platforms perform mutation and extent verification |

Every positive place and the root main.omg probe binds ProgramEntry for
windows_x86_64, linux_x86_64, macos_arm64, and linux_arm64 and declares
Expected exit: 0.

## Execution

Each positive place is an application that exits 0 in both Terminal Psi and native execution:

`sh
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
`

All 10 applications pass with 
ative exit 0 and interp exit: 0.

### The oracle discriminates

The recorded 0 exit is discriminating:
- The backing storage consists of 3 elements [10u64, 20u64, 30u64], loaned as a mutable slice view &mut items[..].
- Mutating index 1 (iew[1] = 45u64) writes through the exclusive borrow into backing storage while preserving the static slice extent knowledge iew.len >= 2.
- Exact subtraction iew.len - 2u64 evaluates to 1u64 and is statically proved not to underflow under the preserved extent bound.
- The caller verifies both that items[1] == 45u64 (in-place mutation observed) and diff == 1u64 (extent subtraction correct), branching to pass() (exit 0) or ail() (crash Trap;, exit 255) on discrepancy.
- Each invalid twin attempts an out-of-bounds mutation iew[2] = 45u64 under 
equires view.len >= 2. Because a slice of length 2 only guarantees indices 0 and 1, the compiler rejects compilation with cannot prove index. None reaches native execution.

Check all nine invalid twins with:

`sh
omega --check samples/cli/facts/mutable_slice_element_write_bound/<place>/invalid/main.omg
`

### Acceptance Gate

`sh
python tools/sample_native_gate.py --cli C:\SoftwareDevelopmentKits\Omegauild\omega-latest.exe --filter cli/facts/mutable_slice_element_write_bound --jobs 4
`

sample_native_gate.py reports:
- 10/10 samples meet their documented oracle on windows_x86_64 (and linux_x86_64).
- 9/9 intended-invalid controls match their authored rejection (cannot prove index).

## Requirements and Evidence

- **Parent outcome:** TERMINAL-SLICE-VIEW-VOCABULARY ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place mutable slice view (&mut [u64]) extent guarding, safe indexed element mutation, post-write extent preservation, exact subtraction underflow prevention, and native realization agreement.

- **Obligations accounted for across the 8 specification and lowering layers:**
  - **Establishment:** A 3-element mutable array [u64; 3] lends a mutable slice view &mut [u64] via full-extent mutable loan &mut items[..]. The contract guard 
equires view.len >= 2 establishes the static bound for indexed mutation at index 1 (1 < view.len) and the underflow-free lower bound for exact subtraction iew.len - 2u64 directly from the length precondition.
  - **Identity and lifetime:** The mutable slice view &mut [u64] holds exclusive borrow access over backing storage. Element mutation iew[1] = 45u64 updates backing storage elements without altering the slice descriptor's base address or length, ensuring that iew.len >= 2 remains invariant across mutation. Storage identity is maintained across local variables, record fields (Holder), parameter bindings, returned views, branch joins, constant bounds, state transitions, machine calls, and target bodies.
  - **Composition:** The mutable slice view composes cleanly through all 9 fact places. Callee functions receive exact references, branch joins unify mutable descriptors, and state transitions preserve length contracts (
equires view.len >= 2). In all 9 places, positive witnesses and reject twins differ strictly in the mutated index offset (1 vs 2).
  - **Soundness boundaries:** All 9 <place>/invalid/main.omg reject twins fail closed at compile time when attempting iew[2] = 45u64: the compiler statically detects that index 2 is not provably within bounds under iew.len >= 2 and emits cannot prove index. No unverified writes or out-of-bounds pointer offsets are admitted to native code generation.
  - **Checked-to-Terminal Psi lowering (Psi 05–07):** Mutable slice formation &mut items[..], indexed element write iew[1] = val, and extent subtraction iew.len - 2u64 lower into canonical target-neutral Terminal Psi (PSITERM\0), representing the mutable slice view as a two-word descriptor (pointer + element length). Proof obligations are discharged and erased while preserving memory safety. All 10 positive entries execute in the Terminal Psi interpreter (omega run --both), observing exit 0.
  - **Provider settlement, layout geometry, and ABI calling plans (Omega 00–02):** ProgramEntry settles across all four hosted targets (windows_x86_64, linux_x86_64, macos_arm64, linux_arm64). Layout geometry for fixed array [u64; 3] (size_of = 24, align 8) and mutable slice view &mut [u64] (size_of = 16, align 8, 8-byte pointer + 8-byte length) validates. Calling plans allocate slice descriptor register pairs (
cx/
dx on Windows x86_64, 
di/
si on Linux x86_64, x0/x1 on AArch64) according to target calling conventions.
  - **Instruction selection, register homes, and activation frame realization (Omega 03–06):** Backend lowering selects scaled-index memory stores (mov qword ptr [rcx + 8], 45 on x86_64, str x-reg, [x0, #8] on arm64), integer subtraction, and register transfers. Register homes and stack frame activation slots are assigned with bounded worst-case stack usage (WCSU).
  - **Form encoding, branch relaxation, relocations, and native image emission (Omega 07–09 + emission):** Target ISA machine instructions encode into native binary bytes, basic blocks relax with offset reflow, machine-state footprint evidence (ctual_transitive_footprint ⊆ permitted_transitive_use) validates within host allocations, and direct PE/ELF/Mach-O images are emitted. Native execution and Terminal Psi interpretation achieve exact outcome agreement (omega run --both, exit 0) across all hosted targets.

## What remains open

- Dynamic runtime slice indices for mutable indexed writes under dynamic bounds checks (iew[dynamic_idx] = val with runtime guard).
- Mutable slice subslice window mutation (&mut view[start..end]).
- Multi-dimensional nested mutable slice views (&mut [&mut [T]]).
