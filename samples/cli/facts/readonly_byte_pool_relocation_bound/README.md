# Read-Only Byte-Pool Relocation Bound Fact Row

Fact: a read-only byte-pool slice literal ("omega") establishes a static 5-byte length bound (bytes.len >= 5u64 && bytes.len <= 5u64), proving safe indexed byte projection bytes[4u64] and exact length subtraction bytes.len - 5u64 across all nine places.

Every place establishes or transports an immutable read-only byte-pool slice literal `"omega"` with static length 5 (`bytes.len >= 5u64 && bytes.len <= 5u64`). This static bound establishes two distinct verification obligations across all nine language places:
1. **Safe indexed projection:** `bytes[4u64]` requires `4u64 < bytes.len`. Since `bytes.len >= 5u64`, the index is statically proved in range, projecting byte `97u8` (`'a'`) without runtime bounds checks.
2. **Exact length subtraction:** `bytes.len - 5u64` requires `bytes.len >= 5u64`, which is statically proved by the slice bound, discharging exact unsigned subtraction to `0u64` without underflow.

Only the transport mechanism of the read-only byte-pool slice literal changes across the nine places.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness inspecting the read-only byte-pool slice literal |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the slice literal is assigned to a local variable before inspection |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the slice literal is stored into and read from a struct field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the slice literal is passed through a machine parameter with required length contract |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a helper machine returns the slice literal with guaranteed length contract |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms evaluate the slice literal and join before inspection |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants supply the projection index (`4u64`) and length subtrahend (`5u64`) |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a state transition forwards the slice literal to a successor machine |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine routes the slice literal to the inspector |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | dedicated target-scoped machine bodies evaluate the slice literal on each target |

## Execution

Each positive place is an application that exits 0, binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), and carries an authored `expected_native_exit.txt` declaring `0`. `omega run --both` verifies agreement between Terminal Psi interpreter and native compilation:

```sh
omega run --both samples/cli/facts/readonly_byte_pool_relocation_bound/main.omg
omega run --both samples/cli/facts/readonly_byte_pool_relocation_bound/local/main.omg
omega run --both samples/cli/facts/readonly_byte_pool_relocation_bound/field/main.omg
omega run --both samples/cli/facts/readonly_byte_pool_relocation_bound/parameter/main.omg
omega run --both samples/cli/facts/readonly_byte_pool_relocation_bound/call_result/main.omg
omega run --both samples/cli/facts/readonly_byte_pool_relocation_bound/match_join/main.omg
omega run --both samples/cli/facts/readonly_byte_pool_relocation_bound/const_name/main.omg
omega run --both samples/cli/facts/readonly_byte_pool_relocation_bound/other_state/main.omg
omega run --both samples/cli/facts/readonly_byte_pool_relocation_bound/other_machine/main.omg
omega run --both samples/cli/facts/readonly_byte_pool_relocation_bound/target_body/main.omg
```

All ten positive places produce `native exit 0` and `interp exit: 0`.

### The Oracle Discriminates

The authored exit code `0` is strictly discriminative. A 1-token perturbation of the demanded byte value in `inspect_bytes()` (from `transition b == 97u8` to `transition b == 98u8`) or of the caller expectation (`transition probe() == 0` to `transition probe() == 1`) routes execution to `machine bad() { crash Trap; }`, trapping both engines with `native signal 5` and `interp crash: Trap` (exit code 255).

### Reject Twins

Each place has an `invalid/` twin (`sample.json` + `expected.txt`) that breaks only the construct's own contract, input, or domain (never a post-hoc caller guard):
- **Obligation class (`local`, `field`, `parameter`, `call_result`, `match_join`, `other_state`, `other_machine`, `target_body`):** Call/transition contract precondition (`cannot prove requires contract`) when the read-only byte-pool literal is shortened from 5 bytes (`"omega"`) to 4 bytes (`"omeg"`), failing `bytes.len >= 5u64 && bytes.len <= 5u64`.
- **Obligation class (`const_name`):** Exact unsigned subtraction range obligation (`may overflow `u64``) when the named subtrahend constant is increased from `5u64` to `6u64`, failing `bytes.len - SUBTRAHEND` under `bytes.len >= 5u64 && bytes.len <= 5u64`.

None reaches native execution.

Check each twin with:
```sh
omega --check samples/cli/facts/readonly_byte_pool_relocation_bound/<place>/invalid/main.omg
```

| Place | Reject Twin | Obligation Class | Authored Diagnostic Fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Exact arithmetic range | `may overflow `u64`` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Contract precondition | `cannot prove requires contract` |

## Pinned Fixtures

All 18 positive and reject places are pinned in the compiler corpus:
- **Pass fixtures (`expected_native_exit.txt` = `0`):**
  - `tests/omega/pass/facts/readonly_byte_pool_relocation_bound_through_local_exit/`
  - `tests/omega/pass/facts/readonly_byte_pool_relocation_bound_through_field_exit/`
  - `tests/omega/pass/facts/readonly_byte_pool_relocation_bound_through_parameter_exit/`
  - `tests/omega/pass/facts/readonly_byte_pool_relocation_bound_through_call_result_exit/`
  - `tests/omega/pass/facts/readonly_byte_pool_relocation_bound_through_match_join_exit/`
  - `tests/omega/pass/facts/readonly_byte_pool_relocation_bound_through_const_name_exit/`
  - `tests/omega/pass/facts/readonly_byte_pool_relocation_bound_through_other_state_exit/`
  - `tests/omega/pass/facts/readonly_byte_pool_relocation_bound_through_other_machine_exit/`
  - `tests/omega/pass/facts/readonly_byte_pool_relocation_bound_through_target_body_exit/`
- **Fail fixtures (`expected.txt`):**
  - `tests/omega/fail/facts/readonly_byte_pool_relocation_bound_through_local_rejected/`
  - `tests/omega/fail/facts/readonly_byte_pool_relocation_bound_through_field_rejected/`
  - `tests/omega/fail/facts/readonly_byte_pool_relocation_bound_through_parameter_rejected/`
  - `tests/omega/fail/facts/readonly_byte_pool_relocation_bound_through_call_result_rejected/`
  - `tests/omega/fail/facts/readonly_byte_pool_relocation_bound_through_match_join_rejected/`
  - `tests/omega/fail/facts/readonly_byte_pool_relocation_bound_through_const_name_rejected/`
  - `tests/omega/fail/facts/readonly_byte_pool_relocation_bound_through_other_state_rejected/`
  - `tests/omega/fail/facts/readonly_byte_pool_relocation_bound_through_other_machine_rejected/`
  - `tests/omega/fail/facts/readonly_byte_pool_relocation_bound_through_target_body_rejected/`

## Requirements and Evidence

- **Parent outcome:** `READONLY-BYTE-POOL-RELOCATION` / `BYTE-VIEW-BOUND-DISCHARGE` (`TASKS.md`)
- **Requirement established:** Static length bounds on read-only byte-pool slice literals (`bytes.len >= 5u64 && bytes.len <= 5u64`) are established at literal definition and preserved across all nine language places, proving safe indexed byte projection (`bytes[4u64]`) without bounds checks and discharging exact unsigned length subtraction (`bytes.len - 5u64`) without underflow across all four hosted native targets.

### Full 8-Layer Specification and Lowering Evidence

- **Establishment:** Read-only byte literal `"omega"` introduces static bounds `bytes.len >= 5u64 && bytes.len <= 5u64` into the proof context at definition. Safe index projection `bytes[4u64]` is proved by `4u64 < bytes.len` directly from `bytes.len >= 5u64`, and exact subtraction `bytes.len - 5u64` is proved non-negative without requiring post-hoc caller equality guards.
- **Identity and lifetime:** The byte slice literal is backed by immutable constant pool storage with indefinite lifetime. Slice descriptors `(address, length)` preserve their storage identity and exact length across local variables, record fields, parameters, return values, match joins, state transitions, machine calls, and target-scoped bodies without aliasing corruption.
- **Composition:** The static byte-pool length bound composes through all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), supplying the required preconditions of helper machines and state transitions. Positive places and their reject twins differ only in the construct's own input (`"omega"` vs `"omeg"`) or constant subtrahend (`5u64` vs `6u64`).
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time with static proof verification diagnostics (`cannot prove requires contract` or `may overflow `u64``), proving that out-of-bounds indexing and underflowing subtraction are soundly rejected before native emission.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Slice literals lower into canonical `EstablishByteSequenceLiteral` operations and borrow parameters in Terminal Psi (`PSITERM\0`). Proof-formal propositions are verified and erased while retaining runtime descriptors and checked integer bounds, executing identically in the Terminal Psi interpreter (`omega run --both` exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` binds across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Slice views lower to `(ptr, len)` descriptor pairs passed across target ABI calling plans conforming to standard register and stack calling conventions.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backends emit instructions to load byte-pool relocations (RIP-relative addressing on x86_64, `ADRP` + `ADD` / `PAGE` + `PAGEOFF` on ARM64), perform 8-bit byte loads (`movzx` / `ldrb`), and execute 64-bit integer subtractions with register assignment and spill management adhering to WCSU bounds.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Read-only byte literal contents (`b"omega"`) are placed in dedicated read-only data sections (`.rdata` on PE/COFF Windows, `.rodata` on ELF Linux, `__TEXT,__cstring` on Mach-O macOS). Relocations resolve symbolically, and final executables run natively and agree with Terminal Psi (`omega run --both` exit 0).

### Verification Evidence

- `sample_native_gate.py --filter cli/facts/readonly_byte_pool_relocation_bound`: 10/10 samples meet their documented oracle on `windows_x86_64` (root + 9 places pass with exit 0); 9/9 intended-invalid controls match their authored rejection (`cannot prove requires contract` or `may overflow `u64``).
- Discrimination: changing expected return value from 0 to 1 causes execution to trap through `crash Trap;` with exit code 255 (`native signal 5` and `interp crash: Trap`), demonstrating strict oracle discrimination.
- What remains open: mutable byte slice relocation bounds, variable-length subslice slicing bounds propagation, and wide UTF-8 character decode relocation bounds.
