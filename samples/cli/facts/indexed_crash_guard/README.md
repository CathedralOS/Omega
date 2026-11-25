# Indexed entry crash guard

Fact: the exact selected element `items[index] == 7` discharges the callee's indexed zero crash guard and returns that same 7 without a caller crash ceiling. The independent bound is `index < 2`; the consuming operation is `maybe_zero(items, index)`.

This row advances **CRASH-CONTRACT**: entry-operand normalization and invocation-specific disproof for dynamic indexed integer observations. It does not close that outcome's other generic, structural, provider, or crash-execution obligations.

The root application and nine fact-place projects expose the public omitted-cause prohibition on `read_seven`: none adds a `crashes Trap` clause there. Every pair differs only by deleting its selected-element equality requirement.

## Places

| Place | Positive | Reject twin | Transfer |
| --- | --- | --- | --- |
| Root | [`main.omg`](main.omg) | — | Root entry baseline and crash-guard discharge |
| Local | [`local/main.omg`](local/main.omg) | [`invalid_local/main.omg`](invalid_local/main.omg) | Immutable selector copy |
| Field | [`field/main.omg`](field/main.omg) | [`invalid_field/main.omg`](invalid_field/main.omg) | Exact record selector field |
| Parameter | [`parameter/main.omg`](parameter/main.omg) | [`invalid_parameter/main.omg`](invalid_parameter/main.omg) | Original entry selector |
| Call result | [`call_result/main.omg`](call_result/main.omg) | [`invalid_call_result/main.omg`](invalid_call_result/main.omg) | Checked normal `result == index` guarantee |
| Match join | [`match_join/main.omg`](match_join/main.omg) | [`invalid_match_join/main.omg`](invalid_match_join/main.omg) | Both arms deliver the same selector; both execute |
| Const name | [`const_name/main.omg`](const_name/main.omg) | [`invalid_const_name/main.omg`](invalid_const_name/main.omg) | Named seven retains exact value |
| A nested machine | [`other_state/main.omg`](other_state/main.omg) | [`invalid_other_state/main.omg`](invalid_other_state/main.omg) | Renamed collection and selector arrival |
| Another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`invalid_other_machine/main.omg`](invalid_other_machine/main.omg) | Forwarded same zero-guarded route |
| Another target body | [`target_body/main.omg`](target_body/main.omg) | [`invalid_target_body/main.omg`](invalid_target_body/main.omg) | Target-family forwarder preserves the route |

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

`parameter`, `local`, `const_name` and `other_machine` pass `omega --check` and run with native/Psi agreement on `windows_x86_64` (exit 0, empty stdout and stderr on both engines); `field`, `call_result` and `match_join` do the same on `linux_x86_64`. `target_body` passes `omega --check`; `omega run` on `windows_x86_64` stops at Build provider selection ("boundary requirement `SelectedForward::forward` has no provider"). `other_state` calls a nested machine whose omitted crash route is inferred as unconditional. The root application's own machines raise no error, but the root package also compiles every subdirectory, `invalid_*` twins included, as one of its modules, so it is refused for their errors. `TASKS.md` (**PROOF-KERNEL-CORE**, step 5) tracks these.

## Reject twins

Each place has an `invalid_<place>/` twin whose `main.omg` omits only the `requires items[index] == 7` precondition (or equivalent carrier constraint). Because `read_seven` is `pub` and declares no `crashes Trap` clause, public crash omission is forbidden. Each twin fails closed with:

```text
error: call from `read_seven` to `maybe_zero` has an uncovered Trap crash route; publish a same-cause route whose guard covers this invocation
```

(or `call from read_seven to forward` for `other_machine` and `target_body`, whose call is the boundary requirement `SelectedForward::forward`). Check a twin with `omega --check samples/cli/facts/indexed_crash_guard/invalid_<place>/main.omg`.

## Requirements and Evidence

Owning task: **CRASH-CONTRACT** (`TASKS.md`) under
[crash call composition](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#crash),
[guarded crashes and public omission](../../../../wiki/spec/language/effects.md#guarded-crashes),
and [independent numeric formation](../../../../wiki/spec/language/numeric_values.md).
This row establishes that an entry observation `items[index] == 7` with `index < 2` statically discharges the callee's indexed crash guard `crashes Trap items[index] == 0` without requiring a caller crash ceiling. Today that holds in `parameter`, `local`, `field`, `call_result`, `match_join`, `const_name`, `other_machine` and `target_body`; the paragraph after the run commands says what stops the other places. The bullets below describe those eight places only.

- **Establishment:** The entry precondition `requires index < 2` combined with `requires items[index] == 7` establishes that `items[index] != 0` at the invocation of `maybe_zero(items, index)`. Because `7 != 0`, the callee's crash condition `items[index] == 0` is proved unreachable at this call site, discharging the crash obligation directly without requiring a caller crash ceiling or relying on post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The guarded fact `items[index] == 7` stays bound to the exact scalar subject and immutable collection length across a local variable copy (`local`, `let selected = index`), the explicit parameter (`parameter`), a named constant (`const_name`, `const SEVEN: u64 = 7`), a forwarding machine (`other_machine`, `forward`) and a target-scoped forwarder (`target_body`). When the call selects the element with an equal value of another identity, a copied record field (`field`, `selection.index`) or a result guaranteed equal (`call_result`, `ensures result == index`), the fact still applies: an element of a shared input keeps its entry contents, so both the requirement and the call's route read it over the input. A value `match` that yields `index` on both arms (`match_join`) keeps the fact across its join, and the caller's guard `items[0] == 7` reaches the private nested `finish` that receives `@items` through private entry inference. The nested machine (`other_state`) place does not pass yet. Storage versions, distinct element slots, and borrow lifetimes are preserved without invalidation.
- **Composition:** In those eight places the crash-guarded index bound composes into the consuming `maybe_zero` invocation: the call substitutes the exact actuals (`items`, `index`), and a forwarder's own published route covers the call it forwards. Each `<place>/main.omg` vs. `invalid_<place>/main.omg` differs solely in the presence versus deletion of the precondition `requires items[index] == 7`, maintaining identical carrier shapes, bounds, and operations.
- **Soundness boundaries:** Every `invalid_<place>/main.omg` twin is refused at
  compile time. `invalid_parameter`, `invalid_local`, `invalid_field`,
  `invalid_call_result`, `invalid_match_join`, `invalid_const_name`,
  `invalid_other_machine` and `invalid_target_body` fail closed because omitting the equality leaves the
  zero-element crash route uncovered: `read_seven` is `pub`, so the compiler
  refuses public crash omission with `error: call from read_seven to ... has an
  uncovered Trap crash route; publish a same-cause route whose guard covers
  this invocation`. `invalid_target_body` is refused for the same route at
  its boundary call (`call from read_seven to forward`).
  `invalid_other_state` is refused for the
  call from `read_seven` to the nested `consume`, the same cause that stops
  its positive.
- **Checked-to-Terminal Psi lowering (`Psi 06–08`):** Source constructs lower without loss into canonical target-neutral Terminal Psi (`PSITERM\0`), translating guarded state transitions, crash edges, and indexed reads while erasing proof-only obligation tags (`proof-term-tags`). For `parameter`, `local`, `const_name` and `other_machine` on `windows_x86_64`, and for `field`, `call_result` and `match_join` on `linux_x86_64`, the Terminal Psi interpreter and the native program both exit 0 with empty stdout and stderr; `target_body` stops at Build provider selection before it runs.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows join `ProgramEntry` declarations across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) to the Terminal catalog without reopening source. Fixed-array layout geometry for `[u64; 2]` (`size_of = 16`, align 8, stride 8) and record layout for `Selection` (`size_of = 8`, align 8) validate. Target `CallPlan`/`StatePlan` lower array references (`@[u64; 2]`), index arguments (`u64`), and return values across register lanes (`rcx`/`rdx`/`rax` on Windows x86_64, `rdi`/`rsi`/`rax` on Linux x86_64, `x0`/`x1` on AArch64) and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backend lowering selects 64-bit indexed memory loads (`mov` with scaled index on `x86_64`, `ldr` with shifted register on `arm64`), integer equality comparisons (`cmp`/`je`/`jne` or `cbz`/`cbnz`), and conditional crash branches. Register homes and disjoint stack-frame activation storage are assigned for the local `items` array, and entry prologues and multi-return epilogues are inserted honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve to concrete frame offsets for local array elements, function basic blocks lay out with branch relaxation and offset reflow around crash/exit routes, transitive machine-state footprint evidence is validated, and direct PE/ELF/Mach-O executables are emitted. Native/Psi agreement has been observed only on `windows_x86_64`, for the four places named in the previous bullet; the other hosted targets have not run this revision.

## Build-owned target-body migration

The `target_body` place and its twins carry no source target prefix. `BUILD-OWNED-TARGET-SELECTION` replaces the prefixed `forward` family with ordinary named implementations (`X86Forward`, `ArmForward`) of the boundary-trait requirement `SelectedForward`, selected by explicit target-keyed Build rows. The requirement publishes the bodies' exact guarded crash route and an empty reach; the implementations add no tag check, because an unconditional `crashes Trap` would widen the published route this fact is about, and both implementations behave identically. The public caller names `invokes SelectedForward` in its synchronous-invocation ceiling and still publishes no crash ceiling. Observed on macOS x86-64 with optimized producers for base 4f87c9e08b1 and for the #1378 head: the migrated positive exits 0 under `omega run --both` (native and Terminal Psi agree) and every twin and control in the place still rejects with its authored fragment. No compiler change was needed. The other hosted targets keep their authored rows, unexecuted here.
