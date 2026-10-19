# Compound index guard

The live fact `0 <= b * 20 + a < 200` permits storing `7` at
`grid[b * 20 + a]`.

The root and nine place projects bind `ProgramEntry` and declare an `Expected exit: 0`
runtime oracle. Each place verifies that the guarded store executes and that the write
lands (`grid[75] == 7`), exiting 0 natively and under the Terminal interpreter.

Each place repeats the same fact sentence in its source. The index remains a
two-variable `i32 in Wrapping` computation, the grid has 200 elements, and the
operation remains an indexed store. With `a = 15` and `b = 3`, `b * 20 + a == 75`,
which satisfies `0 <= 75 < 200`.

| Place | Positive | Reject twin | Evidence carried to the store |
| --- | --- | --- | --- |
| root (anchor) | [source](main.omg) | - | Root reference row proving the compound index guard. |
| `local` | [source](local/main.omg) | [source](local/invalid/main.omg) | Copied coordinates and their compound guard cross the transition. |
| `field` | [source](field/main.omg) | [source](field/invalid/main.omg) | The guard bounds the same field computation at the store. |
| `parameter` | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) | Explicit coordinate parameters carry the compound requirement. |
| `call_result` | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) | A checked coordinate result equals the guarded coordinate. |
| `match_join` | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) | Both arms preserve the guarded coordinate. |
| `const_name` | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) | Named stride 20 and ceiling 200 preserve the same computation. |
| `other_state` | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) | The guarded field computation survives arrival in `put`. |
| `other_machine` | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) | A separate store machine requires the same compound bound. |
| `target_body` | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) | Every hosted target body checks the same guard and store. |

Each `invalid/` twin removes only the strict upper-bound proposition, including
its corresponding parameter requirement where present. The lower bound stays;
these controls must reject for the store's missing index upper bound.

```sh
omega run --both samples/cli/facts/compound_index_guard/<place>/main.omg
omega --check samples/cli/facts/compound_index_guard/<place>/invalid/main.omg
```

The baseline call-result place rejects with `cannot prove index
main.b * 20 + a is within length 200 in Main::main::put` (diagnostic quoting
omitted here). The checked coordinate equality must survive in the compound
wrapping observation.

Execution acceptance covers root and all nine places across both native and
Terminal engines, and all nine reject twins reject under `--check`.

```sh
python3 tools/sample_native_gate.py --cli <omega> --facts --filter compound_index_guard
```

The sample gate reports ten passing programs (root plus nine places), nine
intended-invalid rejections, and zero unscored.

## Discrimination

The oracle was proved to discriminate before the `macos_arm64` golden rows were
recorded. Each corruption changed one component of the compound index, and the
engines were observed rather than assumed.

| Corruption | Component changed | Observed result | Class |
| --- | --- | --- | --- |
| grid extent `200 -> 74` with guard ceiling `200 -> 74` | collection length and guard ceiling together | `cannot prove index 75 is within length 74`, check exits 1 | static refusal |
| index written as the fixed constant `250` into a length-`200` grid | access coordinate | `cannot prove index 250 is within length 200`, check exits 1 | static refusal |
| stride `20 -> 10`, oracle `grid[75] == 7` retained | stride feeding the compound index | `native signal 5` + `interp crash: Trap`, exit 255 | runtime trap |
| stored element `7 -> 8` | stored value read back by the oracle | `native signal 5` + `interp crash: Trap`, exit 255 | runtime trap |

The two classes split along whether the checker can constant-fold the
corruption. Changing the extent or writing a literal coordinate is decided at
check time, so native production never runs. Corrupting the stride or the
stored element keeps a well-typed guard and a provable index, so the program
compiles and the disagreement surfaces at execution, where the native engine
traps (`signal 5`) and the Terminal Psi interpreter raises `Trap`. Both engines
agree in both classes. The oracle therefore measures the compound index guard
rather than merely the program's ability to compile.

## Requirements and Evidence

Owning task: **INDEX-INDUCTIVE-BOUNDS** ([TASKS.md](../../../../TASKS.md)). This row establishes that a compound boolean arithmetic guard (`0 <= b * 20 + a < 200`) proves in-bounds indexed array access across all nine transport places.

- **Establishment:** The compound conjunction/arithmetic guard (`main.b * 20 + main.a >= 0 && main.b * 20 + main.a < 200`) on the transition edge establishes safe compound indexing into `grid: [i32; 200]` at `put` directly from static arithmetic bounds, without any post-hoc caller equality guard.
- **Identity and lifetime:** The compound index relationship between stride `20`, base coordinate `b`, offset `a`, and collection extent `200` remains bound to the exact scalar variables and array collection subjects across copies, parameter passes, and field projections in all 9 places.
- **Composition:** The compound index guard composes across all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) into the consuming array store operation (`grid[b * 20 + a] = 7`); `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in omitting the upper-bound conjunct (`main.b * 20 + main.a < 200`), leaving the indexed store unproved without modifying caller control flow.
- **Soundness boundaries:** All 9 `<place>/invalid/main.omg` reject twins fail closed at compile time (`cannot prove index ... is within length 200`), enforcing that weakening or omitting any conjunct of the compound guard strictly prevents unverified memory indexing.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Compound boolean guards, exact index arithmetic in `Wrapping` carrier mode, and array indexed stores lower into canonical target-neutral `PSITERM\0` while erasing proof-only obligations, executing with identical semantics in the Terminal Psi interpreter (`omega run --both`).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build bindings (`ProgramEntry`) settle across the four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), array/record layout geometry (`[i32; 200]` 800-byte extent, 4-byte element stride) evaluates and validates, and `CallPlan`/`StatePlan` lower carriers across target registers and stack frames.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects comparison instructions, integer arithmetic (`b * 20 + a`), and scaled-index stores, assigns register homes and disjoint stack-frame activation storage, and inserts entry prologues and epilogues honoring the `StatePlan`.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements and scaled-index addressing modes encode, basic blocks and guard branches relax with offset reflow, machine-state footprint evidence validates, and direct PE/ELF/Mach-O emission agrees with Terminal Psi (`omega run --both` exit 0) across hosted targets including `windows_x86_64` and `macos_arm64`.
