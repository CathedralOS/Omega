# Read-only byte literal subslice window preservation

A checked subslice of an immutable byte-view literal preserves its exact window length of 10u64 when multiplied by 10u64 and subtracting 100u64.

Every place establishes an immutable byte-view literal of 14 bytes (`"0123456789ABCD"`), takes a checked 10-byte subslice window `[2u64..12u64]` satisfying ordered proof legs `start <= end` and `end <= length`, observes its exact `.len` (`10u64`), transports that subslice or window length across the place, scales by `10u64` to `100u64`, and executes exact subtraction `total - 100u64` returning `0`.

## Places

| Place | Positive | Reject twin | Transport / Carrier |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | root immutable byte literal subslice window |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | local variable binding |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | record field storage |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | parameter passing with required contract |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | callee invocation returning subslice length |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | pattern match branch convergence |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | named constants defining subslice window bounds |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | state machine transition forwarding length |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | forwarding machine evaluating the product |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | target-scoped machine bodies on hosted targets |

Run each positive place or root with:

```sh
omega run --both samples/cli/facts/readonly_byte_literal_subslice/main.omg
omega run --both samples/cli/facts/readonly_byte_literal_subslice/<place>/main.omg
```

Each place has an `invalid/` twin that takes a 9-byte subslice (`[2u64..11u64]`), causing the product `90u64` to underflow unsigned exact subtraction `total - 100u64`. Each invalid twin rejects during proof checking with `may overflow \`u64\``. Check each twin with:

```sh
omega --check samples/cli/facts/readonly_byte_literal_subslice/<place>/invalid/main.omg
```

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `607450e1cd264d4f836091eeaf41f295`.
- **Requirement established:** End-to-end nine-place fact row preservation for immutable byte-view literal subslice window bounds, length folding, and exact arithmetic subtraction.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** The 14-byte literal `"0123456789ABCD"` establishes an immutable byte-view backing with exact length 14. The subslice operation `bytes[2u64..12u64]` proves both range-formation obligations `2u64 <= 12u64` and `12u64 <= 14u64` at introduction, producing a valid 10-byte subslice window descriptor without requiring any post-hoc caller equality guard (`transition ... == 0`).
- **Identity and lifetime:** The byte literal resides in the immutable read-only image literal pool; its descriptor borrows immutable backing that outlives every function activation. The static read-only byte view descriptor `(ptr, len)` retains its exact byte-pool backing and length `10u64` across all 9 places without storage aliasing or unexpected copy.
- **Composition:** The read-only byte subslice extent fact composes through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming exact arithmetic operation (`total - 100u64`). Each positive place `<place>/main.omg` vs. its reject twin `<place>/invalid/main.omg` differs only in the subslice range (`[2u64..12u64]` of extent 10 vs `[2u64..11u64]` of extent 9).
- **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed with compile-time proof diagnostics (`error: exact arithmetic in machine ... may overflow \`u64\``) when the 9-byte subslice length causes `90u64 - 100u64` to underflow unsigned exact arithmetic.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Read-only byte literals and subslice operations lower without semantic loss into canonical target-neutral Terminal Psi (`PSITERM\0`) `ByteSequenceLength` and `ByteSequenceSubslice` entries with explicit proof legs `start <= end` and `end <= length`, while erasing proof-only obligations and executing with full agreement in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans validate 16-byte `&[u8]` byte view layout geometry (two-word fat pointer: 64-bit base pointer and 64-bit length), and `CallPlan`/`StatePlan` lower byte view carriers across register pairs, stack slots, or indirect `sret` homes.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects RIP/PC-relative byte-pool address materialization (`lea` on x86_64, `adrp`+`add` on ARM64), pointer offset addition, length arithmetic, and integer multiplication and subtraction instructions. Backend assigns register homes and disjoint stack-frame storage, inserting entry prologues and multi-return epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Read-only byte literals are placed in the `.rdata` (PE) or `.rodata` (ELF/Mach-O) section, PC-relative relocations and symbolic stack-frame displacements resolve, basic blocks and branches relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across hosted targets including `windows_x86_64` and `macos_arm64`.

### Verification Evidence

- `sample_native_gate.py --filter cli/facts/readonly_byte_literal_subslice`: 10/10 samples meet their documented oracle on `windows_x86_64` (root + 9 places pass with exit 0); 9/9 intended-invalid controls match their authored rejection (`may overflow \`u64\``).
- The oracle discriminates: mutating the expectation in `main()` causes `probe() == 0` to fail, taking the `_ -> bad()` branch and trapping with `crash Trap`, verifying that exit 0 discriminates computation errors.
- Remaining open: dynamic runtime-indexed byte subslice mutable projections, non-literal slice views under content mutation, and inter-package dynamic slice provider composition under `CONSERVATION-CONTRACT`.

