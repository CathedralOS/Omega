# Runtime window length

A runtime window length names the last byte of that window: for a window
`items[0..count]` with `count > 0`, indexing at `length - 1` reads the byte at
`count - 1`, provided `length` is proved to be that window's length.

Every place copies that fact and performs the same last-byte read on a
four-byte `[4, 7, 8, 9]` array with `count == 3`; the oracle is exit 0 only
when the byte is 8. Only the transport of the window length changes.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness indexing last byte through window length |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the length is copied into a local variable |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the length is stored in a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the length arrives as an explicit parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a call result returns the window length |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms of a match yield the window length |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant is the window length |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the length |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine supplies the length |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies uses the length |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and native execution agreement (`interp exit: 0` and `native exit 0`):

```sh
omega run --both samples/cli/facts/length/main.omg
omega run --both samples/cli/facts/length/local/main.omg
omega run --both samples/cli/facts/length/field/main.omg
omega run --both samples/cli/facts/length/parameter/main.omg
omega run --both samples/cli/facts/length/call_result/main.omg
omega run --both samples/cli/facts/length/match_join/main.omg
omega run --both samples/cli/facts/length/const_name/main.omg
omega run --both samples/cli/facts/length/other_state/main.omg
omega run --both samples/cli/facts/length/other_machine/main.omg
omega run --both samples/cli/facts/length/target_body/main.omg
python tools/sample_native_gate.py --facts --filter cli/facts/length
```

### The oracle discriminates

The exit code `0` is strictly discriminative. Corrupting the array byte under the proved last-byte read (`[4, 7, 8, 9]` to `[4, 7, 1, 9]`) leaves the index proved but changes the read result from 8 to 1. The machine transitions on `last_byte(...) == 8` to `done()` (exit 0); any mismatch branches to `bad()` which executes `crash Trap;`. Both native and Terminal Psi engines abort non-zero on failure (`native signal 5` with `interp crash: Trap` agreeing).

## Reject Twins

Each place has an `invalid/` reject twin with authored `sample.json` (`"role": "intended-invalid"` and `"rejection_contains": ["cannot prove index"]`). In each twin, the transported length names the backing view (`items.len`) rather than the runtime window (`window.len`). The last-byte index is then no longer proved, and proof checking fails closed during compile-time verification:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `cannot prove index` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `cannot prove index` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `cannot prove index` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `cannot prove index` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `cannot prove index` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `cannot prove index` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `cannot prove index` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `cannot prove index` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `cannot prove index` |

## Pinned Fixtures

The row is pinned by `tests/omega/{pass,fail}/facts/length_through_*` fixtures; every positive records native exit 0. The nine pass fixtures are byte-identical copies of their place programs, so the sample and the fixture cannot drift apart.

## Requirements and Evidence

- **Parent outcome:** `CANONICALIZE-SCALAR-RANGE-CONTRACTS` (`TASKS.md`).

### Specification and Lowering Layers

1. **Establishment:** The collection/slice length fact is established where it originates: `window.len` on the narrowed slice view `&items[0..count]`, a `measure` call's contract `ensures result == window.len`, or a static `const` matched by the entry `requires count > 0 && count <= items.len` bound. The index `length - 1` is proved in-bounds directly from the established length proposition `count > 0 ==> length - 1 < length`, discharging array/slice indexing without any post-hoc caller equality guard (`transition x == 100`).
2. **Identity and lifetime:** The length fact remains bound to the exact collection/slice identity (`window: &[u8] = &items[0..count]`) across local copy, record field, parameter passing, call result, match join, const bound, state transition, forwarding machine, and target body. The narrowed view retains its exact extent and storage reference throughout execution; no intervening mutation invalidates the window slice between length establishment and the last-byte index read.
3. **Composition:** The length fact composes through all 9 places into the consuming index operation `window[length - 1]`. In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in the source of the transported length (`window.len` representing the runtime window vs `items.len` representing the entire backing collection), isolating the exact length proposition under test without altering caller control flow or state structures.
4. **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed during compile-time proof checking when the transported length names the backing array (`items.len == 4`) rather than the active window (`window.len == 3`). Under the invalid bound, `length - 1` evaluates to 3 which equals or exceeds the window extent `count == 3`. Proof checking rejects with exit code 1 and diagnostic `error: cannot prove index ...`, preventing out-of-bounds slice indexing from compiling or reaching code generation.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Slice construction `&items[0..count]`, length queries `.len`, integer arithmetic `length - 1`, and proved index dereferences lower without information loss into canonical target-neutral Terminal Psi (`PSITERM\0`). Mathematical proof obligations and bounds certificates are checked and erased, while preserving explicit slice descriptors, affine cleanup, and state transitions. The Terminal Psi interpreter executes root and all nine positive places in complete agreement with native execution (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Target realization joins Build companion rows (`ProgramEntry`, `ProviderPlan`, `CompilerBuiltinExecution`, `BoundaryExecutionBinding`) across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Slice views `&[u8]` (pointer + length pair) and backing array `[u8; 4]` evaluate target layout geometry (`size_of` and `align_of` with target-specific pointer widths). `CallPlan` and `StatePlan` lower slice arguments and return values across ABI registers and stack slots.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for slice pointer/length unpacking, integer subtraction (`sub`), and indexed memory byte loads (`movzx` / `ldrb`). Register allocator assigns register homes and disjoint activation frame spill slots, verifying bounded worst-case stack usage (WCSU) and emitting platform-standard entry prologues and epilogues conforming to `StatePlan` ceilings.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Final encoding resolves symbolic displacements, lays out basic blocks with short/near branch relaxation, validates transitive machine-state footprints, and emits direct PE/ELF/Mach-O images. Direct native emission produces executables that execute natively with exit code 0 and agree with Terminal Psi (`omega run --both` exit 0) across platforms.
