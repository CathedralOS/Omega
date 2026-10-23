# Field `where` index

A data `where` roster carries standing field facts. `0 <= k, k <= 3`
holds for every `Main` value, so `arr[k + 1]` into an array of length 5
compiles with no guard. The indexing machine does not see the literal
store of `k`; the where fact is the proof.

All nine places verify at runtime (`Expected exit: 0` with `crashes Trap`) that
`arr[k + 1]` reads array elements 40 (at `k = 2`) and 50 (at `k = 3`).

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) with [`build.omg`](build.omg) | — | canonical root witness indexing array through standing field bound |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the field is copied into a local variable |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the field itself, from the entry and again after a transition |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the field passed to a parameter whose `requires` is the bound |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a call that returns the field and `ensures` the bound |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | a match whose arms both evaluate to the field |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | the ceiling is named by `const LIMIT: u64 = 3` |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a later state parameter carrying the bound |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | another machine producing and consuming the bound |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | the same field index in every target-scoped machine body |

## Execution

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declares `Expected exit: 0`. `omega run --both` reports Terminal Psi interpreter and native execution agreement (`interp exit: 0` and `native exit 0`):

```sh
omega run --both samples/cli/facts/field_where_index/main.omg
omega run --both samples/cli/facts/field_where_index/local/main.omg
omega run --both samples/cli/facts/field_where_index/field/main.omg
omega run --both samples/cli/facts/field_where_index/parameter/main.omg
omega run --both samples/cli/facts/field_where_index/call_result/main.omg
omega run --both samples/cli/facts/field_where_index/match_join/main.omg
omega run --both samples/cli/facts/field_where_index/const_name/main.omg
omega run --both samples/cli/facts/field_where_index/other_state/main.omg
omega run --both samples/cli/facts/field_where_index/other_machine/main.omg
omega run --both samples/cli/facts/field_where_index/target_body/main.omg
```

### The oracle discriminates

The exit code `0` is strictly discriminative. Corrupting array values or evaluating an unexpected index causes the state transition check `first == 40 && second == 50` to branch to `bad()`, executing `crash Trap;`. Both native and Terminal Psi engines abort non-zero on failure.

## Reject Twins

Each place has an `invalid/` reject twin with authored `sample.json` (`"role": "intended-invalid"` and `"rejection_contains": ["cannot prove index", "is within length 5"]`) and `expected.txt`. Raising the upper bound to `k <= 4` (or `LIMIT = 4`) allows `index + 1 == 5` out of bounds for `[i32 in Wrapping; 5]` and rejects during compile-time proof checking:

| Place | Twin | Pinned Diagnostic Fragment |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `cannot prove index `index + 1` is within length 5` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | `cannot prove index `main.k + 1` is within length 5` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | `cannot prove index `index + 1` is within length 5` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | `cannot prove index `current(main) + 1` is within length 5` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | `cannot prove index `index + 1` is within length 5` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | `cannot prove index `main.k + 1` is within length 5` |
| another state | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | `cannot prove index `index + 1` is within length 5` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | `cannot prove index `index + 1` is within length 5` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | `cannot prove index `main.k + 1` is within length 5` |

## Pinned Fixtures

- Positive passes: `tests/omega/pass/facts/field_where_index_through_<place>_exit` (all 9 places pinned, each carrying `build.omg` and `expected_native_exit.txt` = 0, recording native exit 0).
- Negative controls: `tests/omega/fail/facts/field_where_index_through_<place>_rejected` (all 9 places pinned, rejecting with `cannot prove index ... is within length 5`).

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).

### Specification and Lowering Layers

1. **Establishment:** The field-level `where 0 <= k, k <= 3` refinement on `data Main` establishes the invariant on field `k` directly at construction and mutation. When projected as `main.k` (or into an index expression `main.k + 1`), the compiler proves `0 <= index && index <= 4 < 5` directly from the standing field fact, discharging array indexing into `[i32 in Wrapping; 5]` with no post-hoc caller equality guard (`transition x == 100`) after the operation completes.
2. **Identity and lifetime:** The refined bound `0 <= k, k <= 3` stays attached to the exact storage version of `main.k`. Passing `&Main` or mutating unrelated fields preserves the standing fact; writing to `main.k = 2` or `main.k = 3` re-verifies that the stored value satisfies the `where` contract, while conflicting writes violating `0 <= k && k <= 3` fail closed at source checking before any invalid storage version can be formed.
3. **Composition:** The field `where` index bound composes across all 9 places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`) to prove safe array indexing into `main.arr[index + 1]`. Each place's positive `<place>/main.omg` vs. its reject twin `<place>/invalid/main.omg` differs only in the construct's field refinement (`k <= 3` vs `k <= 4` or `LIMIT = 3` vs `LIMIT = 4`), isolating the exact fact under test without caller guard divergence.
4. **Soundness boundaries:** All 9 reject twins fail closed during source proof checking when the field `where` bound is widened to `k <= 4` (`LIMIT = 4`), allowing `index + 1 == 5` which exceeds array length 5. Proof checking rejects with exit code 1 and diagnostic `error: cannot prove index ... is within length 5`, proving that unsafe or out-of-bounds array access cannot compile or reach backend code emission.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The field-level `where` proof obligation is fully checked and erased during typechecking. The record definition, field projections, arithmetic `index + 1`, and array index operations lower cleanly into target-neutral canonical Terminal Psi (`PSITERM\0`). The Terminal Psi interpreter executes every positive place in complete agreement with native execution (`omega run --both`).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` settles uniformly across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Record `Main` (containing `arr: [i32; 5]`, `k: u64`, `r: i32`, `hits: i32`) evaluates target layout geometry (`size_of` and `align_of` respecting field alignments and padding). `CallPlan` and `StatePlan` assign record reference parameters (`&Main`, `&mut Main`) and return values to standard ABI calling convention registers and stack slots.
7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend lowering selects target ISA instructions (`x86_64` and `arm64`) for record field offset loads, integer addition (`k + 1`), and scaled array indexing (`base + index * 4`). Lowering assigns register homes and disjoint activation stack-frame slots for `Main` storage and caller/callee frames, inserting standard function prologues and epilogues honoring `StatePlan` ceilings.
8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements to record fields and array elements resolve in final instruction encodings, basic blocks relax with offset reflow, machine-state footprint evidence validates transitive usage, and direct native PE/ELF/Mach-O emission produces native executables agreeing with Terminal Psi (`omega run --both` exit 0) across platforms.
