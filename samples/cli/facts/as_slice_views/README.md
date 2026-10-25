# as_slice_views

The ordinary `&main.entries[..]` range lends a shared view of noncopy entries
through the imported core `Slice::range` requirement. The sample reads its
extent and records whether it still covers all four entries.

The root `main.omg` runs with `// Expected exit: 0` and `crashes Trap` on failure:

```text
omega run --both samples/cli/facts/as_slice_views/main.omg
```

The range uses selected checked-provider supply and retains the bounds contract
and backing loan. The runtime root and fixtures expect exit 0 in both engines.

## Nine places

A whole-array shared view of four noncopy Entries has length 4; reading its extent satisfies require_four(view.len).

The checked operation is the call to `require_four`, whose requirement is
`length == 4`. Every positive reads the borrowed descriptor's extent rather
than supplying a literal. Each `<place>/invalid/` (and `<place>/controls/short_extent/`)
twin changes the backing array extent and matching transport contracts to 3,
retaining the operation and its requirement of 4. The twin must fail at that call.
The root application and nine place projects all verify that extent.


| Place | Positive | Invalid twin | Control twin | Fact transport |
| --- | --- | --- | --- | --- |
| root | [source](main.omg) | — | — | whole-array shared view in root main |
| local | [source](local/main.omg) | [source](local/invalid/main.omg) | `local/controls/short_extent/` | whole-array shared view in a local |
| field | [source](field/main.omg) | [source](field/invalid/main.omg) | `field/controls/short_extent/` | whole-array shared view in a record field |
| parameter | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) | `parameter/controls/short_extent/` | view parameter with exact extent requirement |
| call result | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) | `call_result/controls/short_extent/` | borrowed result with exact extent guarantee |
| match join | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) | `match_join/controls/short_extent/` | both arms return the whole-array view; both execute |
| const name | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) | `const_name/controls/short_extent/` | named constant fixes the backing array extent |
| a nested machine | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) | `other_state/controls/short_extent/` | view arrives under an exact extent requirement |
| another machine | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) | `other_machine/controls/short_extent/` | helper receives the view under its extent requirement |
| another target body | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) | `target_body/controls/short_extent/` | all four hosted bodies read the extent |

Each executable positive expects exit 0 and traps if its measured extent differs
from 4. Cross-target checking is distinct from matching-host execution.
The mirrored corpus pairs are
`tests/omega/pass/facts/as_slice_views_through_<place>_exit/` and
`tests/omega/fail/facts/as_slice_views_through_<place>_short_extent/`.

## Requirements and Evidence

Owning task: **TERMINAL-SLICE-VIEW-VOCABULARY** (TASKS.md, P3). This row
discharges one requirement of its acceptance: a consuming non-byte `&[T]` view
call executes natively with Terminal interpreter agreement across the nine
places. The task's wider acceptance (restricted scalar-array constructors,
cyclic view forwarding, and its named sample obligations) remains open there.

- **Establishment:** The four-entry extent `view.len == 4` is established by construction of the fixed array `[Entry; 4]` and carried by the whole-array range borrow `&entries[..]` through the selected provider `Slice::range`. At the consuming call `require_four(view.len)`, the precondition `requires length == 4` is discharged directly from the view's proven extent without relying on any post-hoc caller equality guard (`transition x == 100`).
- **Identity and lifetime:** The slice view `&[Entry]` borrows `entries` under a shared loan (`&`); descriptor construction cannot waive the backing loan. The fact `view.len == 4` tracks the exact array version across local binding, record field (`Held<'a>`), explicit parameter, call return, match branches, named constant, state transition, forwarding machine, and target bodies while `entries` remains live and unmodified in place.
- **Composition:** Each place substitutes the view through its carrier into the same `require_four` call. Calls substitute exact actuals, match-arm joins preserve the view extent on both branch paths (both executed at runtime), and state transitions prove arrival contracts before reading `view.len`. In every place, `<place>/main.omg` vs. `<place>/invalid/main.omg` (and `<place>/controls/short_extent/`) differ solely in the backing array extent and matching transport contracts (`4` vs. `3`), keeping the construct, bound, and operation identical.
- **Soundness boundaries:** Weakening the extent in the reject twins causes static verification to fail closed with `cannot prove requires contract for call require_four` (or `cannot prove index ...`), distinguishing proof failure from lowering refusal. Shared borrow exclusivity and lifetime boundaries prevent access after loan retirement, and descriptor construction cannot synthesize authority over unborrowed storage.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Non-byte `&[T]` range formation `&entries[..]` and extent projection `view.len` lower into canonical target-neutral Terminal Psi (`PSITERM\0`) two-word slice descriptors (pointer + element count), preserving erased proof-formal tag lanes while materializing borrowed-storage restoration windows and evaluation schedules. The Terminal Psi interpreter evaluates the view extent and executes in agreement with native execution (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows join `ProgramEntry` declarations across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) to the Terminal catalog without reopening source. Fixed-array layout for `[Entry; 4]` (`Entry { value: i32 }`, `size_of = 16`, align 4, stride 4) and two-word slice descriptors (`size_of = 16`, align 8) validate. Target `CallPlan`/`StatePlan` lower slice descriptor pointers and lengths across register pairs (`rcx`/`rdx` on Windows x86_64, `rdi`/`rsi` on Linux x86_64, `x0`/`x1` on AArch64) and stack slots.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Target backend lowering selects 64-bit integer comparisons for extent checks, assigns register homes and disjoint stack-frame activation storage for `entries`, computes bounded activation claims (`committed <= bound`), and inserts standard function prologues and epilogues honoring the `StatePlan` ceiling.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements resolve to concrete frame offsets for array storage and slice descriptors, basic blocks lay out with branch relaxation and offset reflow around comparison jumps, transitive machine-state footprint evidence is validated, and direct PE/ELF/Mach-O images are emitted, executing natively and agreeing with Terminal Psi (`omega run --both`, exit 0) across host targets including `windows_x86_64` and `macos_arm64`.

Canonical anchors: [borrowed range access and endpoints](../../../../wiki/spec/language/expressions.md#borrowed-range-access),
[exact dependency transport](../../../../wiki/spec/language/dependent_values.md#establishment-and-witness-loans),
and [call requirements](../../../../wiki/spec/proofs/contracts.md#machines-and-bundles).
The four-entry extent supplies a mathematical fact; descriptor construction
cannot waive its backing loan or supply missing authority.

## Measured run record

Producer: the factsr1 wave's opt-level 1 `omega` built at base
`9ff26a870215d1a351e830cacd1255a852e3dd1e`, plus a corpus runner linked from
this worktree at the same base; host `macos_arm64`.

- Root plus nine places, scratch copies, `omega run --both`: ten
  `native exit 0` rows, each with `interp exit: 0`, zero traps.
- `python3 tools/sample_native_gate.py --facts --filter
  cli/facts/as_slice_views,cli/facts/borrowed_projection_cleanup --cli
  <omega> --jobs 2`: 20/20 samples meet their documented oracle (10 here:
  root + nine places), 27/27 intended-invalid controls match their authored
  rejection (18 here: the nine `invalid/` twins and the nine
  `controls/short_extent/` twins).
- `python3 tools/corpus_gate.py --filter as_slice_views_through_` (jobs=4):
  18 fixtures — nine `checked`, nine `rejected` at the pinned fragment
  `cannot prove requires contract for call require_four`; 0 rejected without
  their expected fragment, 0 fail fixtures that did not reject, 0 lost
  successes, 0 structural moves against `tests/omega/corpus_outcomes.txt`
  (the timed-slower rows are jobs=4/host-load noise the gate labels as such).

```sh
python3 tools/sample_native_gate.py --filter cli/facts/as_slice_views --cli <omega> --jobs 2
python3 tools/corpus_gate.py --filter as_slice_views_through_
python3 tools/corpus_gate.py --native --filter as_slice_views_through_
```
