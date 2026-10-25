# Named conformance dispatch

Parent task: `NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION`

## Governing requirement

Dispatching inline conformance members of a named implementation satisfying a generic trait application (`Selected satisfies Arithmetic<i32>`), preserving exact member contracts, bounds, and values across all nine places.

**A named implementation Selected satisfies Arithmetic<i32> dispatches its inline combine member on explicit i32 operands and preserves the bounded result across all nine places.**

Every place performs the same operation: dispatch the inline `combine` member of named implementation `Selected satisfies Arithmetic<i32>` through generic helper `apply<Selected>(20, 22)` under precondition bounds `0 <= left && left <= 50` and `0 <= right && right <= 50`, returning the exact sum under `ensures result == left + right`. The positive oracle asserts `result == 42` and exits 0 only when the bound and value hold. Only the transport of the operands changes.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| Root | [`main.omg`](main.omg) | — | Root baseline direct generic conformance dispatch |
| Local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | Both bounded parameters are copied into locals before dispatching combine |
| Field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | Bounded record fields carry both operands into the combine dispatch |
| Parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | The dispatching machine reads its two bounded parameters directly |
| Call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Both operands arrive from calls with declared result ranges |
| Match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Both arms keep the operands inside their bounds for combine dispatch |
| Const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | A named ceiling bounds both operands into the combine dispatch |
| A nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | A successor machine receives both bounded operands and dispatches combine |
| Another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | A forwarding machine transports both bounded operands to the combine dispatch |
| Another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Each of four target bodies dispatches combine on the bounded operands |

### Verification commands

Root `main.omg` and all nine place projects bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and declare `Expected exit: 0`. Run any positive directly:

```sh
omega run --both samples/cli/facts/named_conformance_dispatch/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/local/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/field/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/parameter/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/call_result/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/match_join/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/const_name/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/other_state/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/other_machine/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/target_body/main.omg
```

Each place has an `invalid/` twin whose transported right operand lacks the upper bound `<= 50` required by `combine`. The control must reject with `cannot prove requires contract for call apply`. Check each twin with:

```sh
omega --check samples/cli/facts/named_conformance_dispatch/<place>/invalid/main.omg
```

## Requirements and Evidence

Owning task: **NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION** (`TASKS.md`).
This row establishes that named implementation conformance dispatch (`Selected satisfies Arithmetic<i32>`) preserves member contracts, bounds, and arithmetic results across all nine places.

- **Establishment:** The named implementation `Selected satisfies Arithmetic<i32>` defines inline member `combine` with contract `requires 0 <= left && left <= 50 && 0 <= right && right <= 50 ensures result == left + right`. Invoking generic caller `apply<Selected>(20, 22)` binds the conformance witness `Operations = Selected` and discharges member preconditions from the verified operand bounds, establishing the postcondition `result == left + right` directly at the call site without relying on post-hoc caller equality guards (`transition x == 100`).
- **Identity and lifetime:** The named conformance witness `Selected` maintains package-scoped implementation identity and evidence validity across all 9 places without carrying data subjects or `Self` receivers. The explicit scalar argument operands (`20`, `22`) are immutable 32-bit values whose bounds and identities survive local copies, record fields, parameters, call results, match joins, named constants, state transitions, forwarding machines, and target-scoped bodies without storage aliasing or mutation.
- **Composition:** Named conformance member call `Operations::combine(left, right)` strictly checks preconditions at call sites and rejoins its guaranteed equality `result == left + right` into the consuming verification across all nine places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`). In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differs solely in removing the operand upper bound (`right <= 50`) required by the conformance contract, keeping the conformance selection and consuming operations identical.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time: omitting the right operand's upper bound in the caller prevents discharging `combine`'s precondition `requires 0 <= right && right <= 50`, rejecting with `cannot prove requires contract for call apply`. Static verification enforces conformance precondition satisfaction prior to realization, preventing unproved obligations from bypassing trait boundaries.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Static named conformance dispatch resolves the generic trait call `Operations::combine` directly to the specialized monomorphic conformance machine symbol `Selected::combine` in canonical target-neutral Terminal Psi (`PSITERM\0`), erasing proof-only obligation tags (`proof-term-tags`) while preserving direct call edges. The Terminal Psi interpreter executes the monomorphized conformance dispatch and agrees with native execution (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows join `ProgramEntry` declarations across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) to the Terminal catalog without reopening source. Primitive `i32` scalar carriers (`size_of = 4`, align 4) and record field layouts validate. Target `CallPlan`/`StatePlan` lower 32-bit function arguments and return carriers across platform registers (`rcx`/`rdx`/`rax` on Windows x86_64, `rdi`/`rsi`/`rax` on Linux x86_64, `w0`/`w1` on AArch64) and stack frame slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backend lowering selects 32-bit addition instructions (`add eax, edx` on `x86_64`, `add w0, w0, w1` on `arm64`), integer equality comparisons (`cmp`/`je`), and direct relative calls to the resolved conformance implementation. Activation frame realization assigns register homes and spill slots, composing worst-case stack usage (WCSU), and inserts standard function entry prologues and epilogues honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Direct function call relocations to `Selected::combine` and symbolic stack-frame displacements resolve, basic blocks lay out with branch relaxation and offset reflow around exit transitions, transitive machine-state footprint evidence is validated, and direct PE/ELF/Mach-O images are emitted, achieving native execution agreement with Terminal Psi (`omega run --both`, exit 0) across host targets including `windows_x86_64` and `macos_arm64`.

The row is pinned by `tests/omega/{pass,fail}/facts/named_conformance_dispatch_through_*` fixtures; every positive records native exit 0.

## Run record

Producer `7fc0404c1bcc`, target `macos_arm64`. Fresh: `omega run --both` on the
root and all nine places returns native `exit 0` with interpreter `exit: 0`;
`sample_native_gate --programs-only --facts` scores 10/10 against their
documented oracles (259s wall). `corpus_gate --native --filter
facts/named_conformance_dispatch` measures nine `built ... exit:0` rows and
records them in `tests/omega/corpus_native_macos_arm64.txt`; the same filter on
the check tier measures `{'checked': 9, 'rejected': 9}` with 0 structural moves
and 0 fixtures rejected without their expected fragment.

The oracle discriminates three ways under `omega run --both`: the authored
`apply<Selected>(20, 22)` exits 0; the same caller and operands against a
different named implementation whose member computes `left + right + 1` (43)
diverges in both engines (native signal 5, interpreter `crash: Trap`, host
presentation agrees); the same swapped implementation against an oracle of 43
exits 0, so the oracle is not always-trap; and moving only the operand to
`apply<Selected>(20, 23)` also diverges.

Retained: the nine reject twins were pinned before this record and their
`expected.txt` fragment `cannot prove requires contract for call apply` still
matches today's diagnostic. The check-tier timings in the golden are noise from
a loaded host (run started at load 125.93 on 10 CPUs); they are not a
regression.

Remaining open under `NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION`: dynamic trait object dispatch (`dyn Trait`), cross-package visibility export of named conformances, and complex mixed lifetime telescopes.
