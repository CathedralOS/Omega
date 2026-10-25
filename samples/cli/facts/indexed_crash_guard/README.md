# Indexed entry crash guard

Fact: the exact selected element `items[index] == 7` discharges the callee's indexed zero crash guard and returns that same 7 without a caller crash ceiling. The independent bound is `index < 2`; the consuming operation is `maybe_zero(items, index)`.

This row advances **CRASH-CONTRACT**: entry-operand normalization and invocation-specific disproof for dynamic indexed integer observations. It does not close that outcome's other generic, structural, provider, or crash-execution obligations.

The root application and nine fact-place projects expose the public omitted-cause prohibition on `read_seven`: none adds a `crashes Trap` clause there. Every pair differs only by deleting its selected-element equality requirement.

## Places

| Place | Positive | Reject twin | Transfer |
| --- | --- | --- | --- |
| Root | [`main.omg`](main.omg) | — | Root entry baseline and crash-guard discharge |
| Local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | Immutable selector copy |
| Field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | Exact record selector field |
| Parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Original entry selector |
| Call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Checked normal `result == index` guarantee |
| Match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Both arms deliver the same selector; both execute |
| Const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Named seven retains exact value |
| A nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | Renamed collection and selector arrival |
| Another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | Forwarded same zero-guarded route |
| Another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Target-family forwarder preserves the route |

The root application and all nine place projects bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declare `Expected exit: 0`. Run any positive directly:

```sh
omega run --both samples/cli/facts/indexed_crash_guard/main.omg
omega run --both samples/cli/facts/indexed_crash_guard/local/main.omg
omega run --both samples/cli/facts/indexed_crash_guard/field/main.omg
omega run --both samples/cli/facts/indexed_crash_guard/parameter/main.omg
omega run --both samples/cli/facts/indexed_crash_guard/call_result/main.omg
omega run --both samples/cli/facts/indexed_crash_guard/match_join/main.omg
omega run --both samples/cli/facts/indexed_crash_guard/const_name/main.omg
omega run --both samples/cli/facts/indexed_crash_guard/other_state/main.omg
omega run --both samples/cli/facts/indexed_crash_guard/other_machine/main.omg
omega run --both samples/cli/facts/indexed_crash_guard/target_body/main.omg
```

Every positive passes with native and Terminal Psi agreement (`exit 0`).

Measured on macOS arm64 with the shared opt-level 1 producer (`9ff26a87021`):
the root, `controls/fixed_index`, `private_omission` and the nine places report
`native exit 0` and `interp exit: 0` under `omega run --both` (12/12 for this
row); the nine twins exit 1 from checking with their authored fragments (`call
from `read_seven` to `maybe_zero` / `forward`, `uncovered Trap crash route`).
`sample_native_gate.py` reports 21/21 samples at their documented oracle with
18/18 intended-invalid controls matching across this row and
`integer_multiple_remainder`. The pinned corpus rows were recorded with an
opt-level 1 corpus runner built from this tree at `595562a958b4+wt`, jobs=2: 9
`checked` with `check-target:macos_arm64`, 9 `rejected` carrying their own
diagnostic, 9 native `built ... exit:0` and 9 both-leg rows at
`interpreter-exit:0 interpreter:agrees`.

## Reject twins

Each place has an `invalid/` twin whose `main.omg` omits only the `requires items[index] == 7` precondition (or equivalent carrier constraint). Because `read_seven` is `pub` and declares no `crashes Trap` clause, public crash omission is forbidden. Each twin fails closed with:

```text
error: call from `read_seven` to `maybe_zero` at statement ... call 0 has an uncovered Trap crash route; publish a same-cause route whose guard covers this invocation
```

(or `call from read_seven to forward` for `other_machine` and `target_body`). Check a twin with `omega --check samples/cli/facts/indexed_crash_guard/<place>/invalid/main.omg`.

The root-level [`private_omission/`](private_omission/main.omg) is not a
reject twin: it keeps `read_seven` private while omitting the equality, so
the public omitted-cause prohibition does not apply and it compiles and runs
with exit 0. It stands as the recorded exposure of that private coverage gap,
not as a ninth-tenth control.

The corpus pairs are pinned by
`tests/omega/{pass,fail}/facts/indexed_crash_guard_through_<place>_{exit,rejected}`.

## Requirements and Evidence

Owning task: **CRASH-CONTRACT** (`TASKS.md`) under
[crash call composition](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#crash),
[guarded crashes and public omission](../../../../wiki/spec/language/effects.md#guarded-crashes),
and [independent numeric formation](../../../../wiki/spec/language/numeric_values.md).
This row establishes that an entry observation `items[index] == 7` with `index < 2` statically discharges the callee's indexed crash guard `crashes Trap items[index] == 0` without requiring a caller crash ceiling across all nine places.

- **Establishment:** The entry precondition `requires index < 2` combined with `requires items[index] == 7` establishes that `items[index] != 0` at the invocation of `maybe_zero(items, index)`. Because `7 != 0`, the callee's crash condition `items[index] == 0` is proved unreachable at this call site, discharging the crash obligation directly without requiring a caller crash ceiling or relying on post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The guarded fact `items[index] == 7` stays bound to the exact scalar subject and immutable collection length across local variable copy (`let selected = index`), record field projection (`selection.index`), explicit parameter, call result guarantee (`ensures result == index`), all-arm match joins (`match flag { true -> index, false -> index }`), named constants (`const SEVEN: u64 = 7`), successor machine transitions (`machine consume(values: &[u64; 2], selected: u64)`), forwarding machines (`forward`), and target-scoped bodies across all four targets. Storage versions, distinct element slots, and borrow lifetimes are preserved without invalidation.
- **Composition:** The crash-guarded index bound composes into the consuming `maybe_zero` invocation across all nine places. Calls substitute exact actuals (`items`, `index`), `match_join` preserves the selector identity on both branch paths (both executed at runtime), and state transitions prove their arrival obligations before invoking the callee. Each `<place>/main.omg` vs. `<place>/invalid/main.omg` differs solely in the presence versus deletion of the precondition `requires items[index] == 7`, maintaining identical carrier shapes, bounds, and operations.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time: omitting the equality leaves the potential zero-element crash route uncovered. Because `read_seven` is `pub`, the compiler refuses public crash omission with `error: call from read_seven to ... has an uncovered Trap crash route; publish a same-cause route whose guard covers this invocation`. Related contract fixtures (`tests/omega/fail/contracts/indexed_crash_*`) verify that changed selectors, collection writes, divergent matches, and missing bounds also reject.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source constructs lower without loss into canonical target-neutral Terminal Psi (`PSITERM\0`), translating guarded state transitions, crash edges, and indexed reads while erasing proof-only obligation tags (`proof-term-tags`). The Terminal Psi interpreter evaluates the guarded paths and validates agreement with native execution (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows join `ProgramEntry` declarations across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) to the Terminal catalog without reopening source. Fixed-array layout geometry for `[u64; 2]` (`size_of = 16`, align 8, stride 8) and record layout for `Selection` (`size_of = 8`, align 8) validate. Target `CallPlan`/`StatePlan` lower array references (`&[u64; 2]`), index arguments (`u64`), and return values across register lanes (`rcx`/`rdx`/`rax` on Windows x86_64, `rdi`/`rsi`/`rax` on Linux x86_64, `x0`/`x1` on AArch64) and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backend lowering selects 64-bit indexed memory loads (`mov` with scaled index on `x86_64`, `ldr` with shifted register on `arm64`), integer equality comparisons (`cmp`/`je`/`jne` or `cbz`/`cbnz`), and conditional crash branches. Register homes and disjoint stack-frame activation storage are assigned for the local `items` array, and entry prologues and multi-return epilogues are inserted honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve to concrete frame offsets for local array elements, function basic blocks lay out with branch relaxation and offset reflow around crash/exit routes, transitive machine-state footprint evidence is validated, and direct PE/ELF/Mach-O executables are emitted, achieving native execution agreement with Terminal Psi (`omega run --both`, exit 0) across host targets including `windows_x86_64` and `macos_arm64`.
