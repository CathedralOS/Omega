# Dynamic IEEE guarantee refinement

Inline exact IEEE value `0.25` guarantee refines public inclusive f64 interval
`[0, 0.5]`; the dynamic call observes `0.25`.

The unchanged root selects the named implementation `Quarter` as `dyn Fraction`,
calls the public `Fraction::get` slot and crashes unless its completed result is
`0.25`. The implementation separately proves its exact result guarantee.
The checker must prove that guarantee implies the public lower and upper bounds.
Neither an implementation body nor a selector's known target grants a caller a
stronger public contract.

At base `97fca7a18f744b0f7c0b54811b7d41b586d6bfb7`, the optimized compiler
rejected both public bounds before execution:

```text
machine `Quarter::get` realizes `Fraction::get` but does not guarantee its `ensures result >= 0.0`
machine `Quarter::get` realizes `Fraction::get` but does not guarantee its `ensures result <= 0.5`
```

## Running the fact row

Every positive place (and root `main.omg`) binds `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declares
`Expected exit: 0`, and passes `omega run --both`:

```bash
omega run --both samples/cli/facts/dynamic_float_refinement/main.omg
omega run --both samples/cli/facts/dynamic_float_refinement/local/main.omg
omega run --both samples/cli/facts/dynamic_float_refinement/field/main.omg
omega run --both samples/cli/facts/dynamic_float_refinement/parameter/main.omg
omega run --both samples/cli/facts/dynamic_float_refinement/call_result/main.omg
omega run --both samples/cli/facts/dynamic_float_refinement/match_join/main.omg
omega run --both samples/cli/facts/dynamic_float_refinement/const_name/main.omg
omega run --both samples/cli/facts/dynamic_float_refinement/other_state/main.omg
omega run --both samples/cli/facts/dynamic_float_refinement/other_machine/main.omg
omega run --both samples/cli/facts/dynamic_float_refinement/target_body/main.omg
```

## Places and witnesses

| Place | Positive | Reject twin |
| --- | --- | --- |
| root | [`main.omg`](main.omg) | - |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

### The oracle discriminates

The dynamic call to `Fraction::get` returns `0.25`. The transition guard
`transition observed == 0.25` branches to `done()` (exit 0) on match and `bad()`
(`crash Trap;`) on discrepancy. Both execution engines (Terminal Psi interpreter
and native target execution) agree on exit 0.

### Reject twins

Each twin changes only Quarter's own guarantee and returned value to `0.75`:
`machine get() -> f64 ensures result >= 0.75 && result <= 0.75 { 0.75 }`.
Its body still satisfies its own internal guarantee; however, the unchanged
public upper bound (`ensures result <= 0.5`) is violated because `0.75 <= 0.5` is
false. Implementation refinement rejects with:

```text
machine `Quarter::get` realizes `Fraction::get` but does not guarantee its `ensures result <= 0.5`
```

The match witness exercises both selector-producing arms. The const witness
stores the selector itself, preserving its exact implementation and closed trait
application, rather than naming a separate numeric constant.

## Pinned Fixtures

- Positives: `tests/omega/pass/facts/dynamic_float_refinement_through_<place>_exit` (9 places pinned, each byte-identical to `<place>/main.omg` with native exit 0).
- Reject twins: `tests/omega/fail/facts/dynamic_float_refinement_through_<place>_outside_interval` (9 places pinned, each byte-identical to `<place>/invalid/main.omg`, rejecting with `machine 'Quarter::get' realizes 'Fraction::get' but does not guarantee its 'ensures result <= 0.5'`).

### Recorded native evidence (macos_arm64)

Measured on `macos_arm64` at revision `bc9ef5d9a334` with the producer built
from that revision (`opt_level 1`):

```sh
python3 tools/corpus_gate.py --native --filter pass/facts/dynamic_float_refinement_through
```

Before the record the gate reported all nine positives as `new fixture
(built)`, and the run reported `9 fixtures ({'built': 9})`,
`executed 9 (exit codes {'0': 9})`, 0 rejected without their expected fragment
and 0 fail fixtures that did not reject — every positive place already built
and ran, so the gap was the durable record, not a compiler refusal. The nine
rows now read `built ... exit:0 @bc9ef5d9a334` in
`tests/omega/corpus_native_macos_arm64.txt`, spliced in as a pure addition
(+9 lines, 0 deletions, no timeout rows, LOST 0). Re-run on the integration
base `cf4879f2e8ee` (whose only incoming change was another fact's check-leg
rows, 0 Rust files): the same filter is `clean — 9 fixtures ({'built': 9})`,
`executed 9 (exit codes {'0': 9})`, 0 rejected without their expected
fragment, 0 fail fixtures that did not reject.

The mechanism is exercised by this host rather than merely compiled for it.
No positive place is target-scoped except `target_body`, whose four bodies each
invoke `Fraction::get`, so the realized `macos_arm64` body performs the dynamic
call and the `f64` comparison itself; the other eight places are target-neutral
and run the same dispatch on this host.

The recorded 0 is discriminating at the artifact level, not just at build
level. Changing the authored comparison in
`tests/omega/pass/facts/dynamic_float_refinement_through_local_exit/main.omg`
from `transition observed == 0.25` to `transition observed == 0.5` — a
comparison outcome, not a formatting difference — made the same gate report
`expected native 0, observed built {'exit': 'signal', 'signal': '5'}` and the
structural move `exit:0` to `exit:signal signal:5` (exit 3). Restoring the
fixture byte-for-byte returned the same filter to `clean — 1 fixtures
({'built': 1})`, `executed 1 (exit codes {'0': 1})`, so the recorded row is
compared against the artifact's real exit and a wrong value is caught.

The nine reject twins are unchanged by this record and were re-verified on the
check leg: `clean — 9 fixtures ({'rejected': 9})`, 0 rejected without their
expected fragment, 0 fail fixtures that did not reject.

Main then moved again to `4c81ee5883b4` with a compiler change (`compiler:
retain captured placed interpretations across reference returns`), so the
native leg was re-measured with a producer rebuilt from that base: 0
structural moves, `9 fixtures ({'built': 9})`, `executed 9 (exit codes
{'0': 9})`, 0 rejected without their expected fragment, 0 fail fixtures that
did not reject. The four `perf_slower` notes are host load and worker count —
the gate's own context records pre-run load and a jobs mismatch — and no row
moved, so the recorded revision stays `bc9ef5d9a334`.

One more integration step then landed five further compiler commits
(`cc6fcc708be7`, 34 Rust files, scalar qualification and source-evidence
scoping), and the leg was re-measured again with a producer rebuilt from that
base: `clean — 9 fixtures ({'built': 9})`, `executed 9 (exit codes {'0': 9})`,
0 rejected without their expected fragment, 0 fail fixtures that did not
reject, and no perf noise at all. The recorded revision still stands; the
re-measurements are what carry it across the landed compiler commits.

Remaining open on other hosts: `tests/omega/corpus_native_linux_x86_64.txt` and
`tests/omega/corpus_native_windows_x86_64.txt` carry no row for this fact yet
and need those hosts; `linux_arm64` has no native golden in this checkout. The
macOS rows recorded here do not stand in for them.

## Requirements and Evidence

- **Parent outcome:** `DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION` ([TASKS.md](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place IEEE floating-point contract refinement, copyable dynamic implementation selector transport, authenticated IEEE predicate implication, and native dynamic dispatch execution agreement.

Full 8-layer specification and lowering evidence:

- **Establishment:** The implementation `Quarter` proves its own return guarantee `ensures result >= 0.25 && result <= 0.25`. The compiler's IEEE implication judgment statically verifies that the proved interval `[0.25, 0.25]` implies the public trait requirement `ensures result >= 0.0 && result <= 0.5`. Establishment relies strictly on static contract entailment and declaration substitution rather than post-hoc caller equality guards.
- **Identity and lifetime:** Dynamic selector values `Quarter as dyn Fraction` represent authenticated implementation choices with static nominal identity and zero runtime allocation. The dynamic selector is copyable, has static type `dyn Fraction`, and maintains dispatch table integrity across copies and transmissions. Writes to unrelated storage do not invalidate or alias selector bindings.
- **Composition:** The dynamic selector and IEEE contract refinement compose across all 9 standard places:
  - `local`: selector stored in an intermediate local variable (`let selected: dyn Fraction = Quarter as dyn Fraction;`).
  - `field`: selector packed into and projected from a record field (`Holder { selected }.selected`).
  - `parameter`: selector passed as argument into a callee machine parameter (`machine invoke(selected: dyn Fraction)`).
  - `call_result`: selector returned from a helper function call (`machine make() -> dyn Fraction`).
  - `match_join`: selector selected and unified across match branches (`match choice { 1 -> Quarter as dyn Fraction, _ -> Quarter as dyn Fraction }`).
  - `const_name`: selector bound to a named constant (`const SELECTED: dyn Fraction = Quarter as dyn Fraction;`).
  - `other_state`: selector forwarded across state transitions (`transition { _ -> consume(selected) }`).
  - `other_machine`: selector passed through an inter-machine calling chain (`Consumer::run(selected)`).
  - `target_body`: selector received and invoked in target-scoped machine bodies for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`).
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in Quarter returning `0.75` outside `[0.0, 0.5]`.
- **Soundness boundaries:** If an implementation's guarantees do not imply the public contract (e.g. `0.75 <= 0.5` fails), the compiler strictly rejects during type and contract refinement checking with `machine 'Quarter::get' realizes 'Fraction::get' but does not guarantee its 'ensures result <= 0.5'`. Proof preserves ownership, carrier format, and strictness bounds, and unproved bounds cannot escape into Terminal Psi or native code.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Source dynamic trait conversions (`Quarter as dyn Fraction`) and contract guarantees lower into canonical Terminal Psi (`PSITERM\0`). Dynamic dispatch routes lower into explicit selector table indices and `CallDynamic` invocations with separate selector and data lanes. Erased contracts leave no runtime footprint, and programs execute identically in the Terminal Psi interpreter (`omega run --both`), observing `0.25`.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build rows bind `ProgramEntry` across all four hosted targets. Target layout plans evaluate geometry for `dyn Fraction` fat pointers/selectors and IEEE 8-byte `f64` scalar values. `CallPlan` and `StatePlan` map dynamic receiver fat pointers and return floating-point values into target ABI registers (`xmm0` on x86-64, `d0` on ARM64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend selects target ISA instructions (`x86_64` / `arm64`) for dynamic dispatch table loading and indirect calls (`call [rax + disp]` / `blr x16`), and floating-point comparison instructions (`ucomisd` / `fcmp`). Live-across-call spill slots and disjoint activation stack frames are reserved with WCSU bounded activation claims, inserting prologues and callee-saved restoration epilogues.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Read-only byte-pool dynamic dispatch tables, constant-pool float entries, and symbol relocations resolve. Basic blocks and branches relax with offset reflow. Direct PE/ELF/Mach-O images are emitted and execute with exit 0 agreeing with Terminal Psi across the four hosted targets.

## What remains open

Broader interval arithmetic chains, non-orderable float NaN handling across arithmetic domains, suspending dynamic traits, and product-source compiler parity remain governed under the wider `DYNAMIC-IMPLEMENTATION-SELECTION-MIGRATION` and `wiki/spec/language/numeric_values.md` roadmap.

The [named mathematical interval production](alias_contract/README.md) carries
the same IEEE guarantee through checked aliases and public caller preconditions.
