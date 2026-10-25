# Scoped scalar initializer

A const-generic scoped domain family proves a literal initializer against the
exact instance it was declared with, and the consumed value is 3.

Owning requirement: the domain-family instance a scalar initializer must
establish. A **scoped** domain is a const-generic family:

```omega
domain<const Bound: u64> u64::BelowIndex<Bound>(value) requires value < Bound;
```

One declaration whose predicates mention an index binder, so membership is a
property of an exact *application* (`BelowIndex<4>`), not of the family symbol.

Every positive place copies the fact sentence and establishes `u64 in BelowIndex<4>`
from the literal `3`, then consumes the established value as `3`. Only the
transport changes. The root [`main.omg`](main.omg) provides the baseline positive
witness. All positives bind the four hosted `ProgramEntry` targets and declare
`Expected exit: 0`; each passes `omega run --both` at native exit 0 with the
admitted Terminal program agreeing at exit 0.

## Running the fact row

Every positive place (and root [`main.omg`](main.omg)) binds `ProgramEntry` for all
four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`),
declares `Expected exit: 0`, and passes `omega run --both`:

```bash
omega run --both samples/cli/facts/scoped_scalar_initializer/main.omg
omega run --both samples/cli/facts/scoped_scalar_initializer/local/main.omg
omega run --both samples/cli/facts/scoped_scalar_initializer/field/main.omg
omega run --both samples/cli/facts/scoped_scalar_initializer/parameter/main.omg
omega run --both samples/cli/facts/scoped_scalar_initializer/call_result/main.omg
omega run --both samples/cli/facts/scoped_scalar_initializer/match_join/main.omg
omega run --both samples/cli/facts/scoped_scalar_initializer/const_name/main.omg
omega run --both samples/cli/facts/scoped_scalar_initializer/other_state/main.omg
omega run --both samples/cli/facts/scoped_scalar_initializer/other_machine/main.omg
omega run --both samples/cli/facts/scoped_scalar_initializer/target_body/main.omg
```

## Places and witnesses

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | - | baseline scoped scalar initializer establishes `BelowIndex<4>` |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the scoped initializer itself establishes the instance |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the declared record field establishes it |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | it arrives as an explicit parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | the callee completes it and returns |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both arms establish the same instance |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant carries it |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives it |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine supplies it |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target bodies establishes it |

### The oracle discriminates

The consuming operation reads the established value `3` and verifies `observed == 3`
(or `observed - 3 == 0`), transitioning to `done()` (exit 0) on match and branching
to `bad()` (`crash Trap;`) on discrepancy. Both execution engines (Terminal Psi
interpreter and native execution) agree on exit 0.

### Reject twins

Every twin changes exactly one thing: the construct's own initializer, `3` to
`4`, so `BelowIndex<4>` no longer holds (`4 < 4` is false). `git diff <place>/main.omg
<place>/invalid/main.omg` is that one literal. The field twin changes only the
field's literal, and the target-body twin changes only the `windows_x86_64`
body while the other three keep `3`. No twin adds a caller equality guard,
narrows a parameter type, strengthens a state contract or drops an import; the
consuming operation is identical in both directions.

Eight twins reject at the scoped initializer obligation:

```text
cannot prove initializer of `scoped` in exercise is in domain `BelowIndex`;
an annotation cannot establish routed qualification
```

The const-name twin reports the same obligation at the constant's result
domain, and the field twin reports it through the field's declared interval
(`value 4 is outside its declared range 0..=3`). Both are the same obligation
on the same instance; no twin is rejected for an unrelated reason.

## Pinned Fixtures

- Positives: `tests/omega/pass/facts/scoped_scalar_initializer_through_<place>_exit`
  (9 places pinned, each byte-identical to `<place>/main.omg` with native exit 0).
- Reject twins: `tests/omega/fail/facts/scoped_scalar_initializer_through_<place>_rejected`
  (9 places pinned, each byte-identical to `<place>/invalid/main.omg`, rejecting
  with their authored diagnostic).
- Golden corpus registrations pinned in `tests/omega/corpus_outcomes.txt` and
  `tests/omega/corpus_native_macos_arm64.txt` (commit `098d21972b2` on `origin/main`).

## Requirements and Evidence

- **Parent outcome:** `STRUCTURAL-GENERIC-MATCHING` ([`TASKS.md`](../../../../TASKS.md)).
- **Requirement established:** End-to-end nine-place const-generic scoped domain family qualification for scalar initializers, closed index roster preservation, literal membership proof, and native execution agreement.

Full 8-layer specification and lowering evidence:

- **Establishment:** The const-generic scoped domain family `domain<const Bound: u64> u64::BelowIndex<Bound>(value) requires value < Bound;` defines membership over an exact index parameter. Initializing `let scoped: u64 in BelowIndex<4> = 3;` statically proves literal `3 < 4` by evaluating the declaration's predicate against the closed index argument roster (`Bound = 4`). Establishment relies strictly on static predicate evaluation and index substitution rather than assuming qualifications or relying on post-hoc caller equality guards.
- **Identity and lifetime:** The qualified scalar binding retains its static type `u64 in BelowIndex<4>` across value copies, moves, and storage transitions. In accordance with canonical domain indexing rules, distinct applications (e.g. `BelowIndex<4>` and `BelowIndex<9>`) maintain distinct interned instance identities and are not collapsed to the base family symbol. Any inner block-scoped temporaries retire at scope exit, leaving the outer binding's established qualification intact. Writes to unrelated storage do not invalidate or alias the qualified scalar.
- **Composition:** The scoped scalar initializer fact composes across all 9 standard places:
  - `local`: scoped initializer directly establishes the local binding (`let scoped: u64 in BelowIndex<4> = 3;`).
  - `field`: record field carries the scoped domain instance (`struct Holder { scoped: u64 in BelowIndex<4> }`).
  - `parameter`: callee machine parameter receives the scoped domain value (`machine exercise(scoped: u64 in BelowIndex<4>)`).
  - `call_result`: helper function returns the scoped domain value (`machine make() -> u64 in BelowIndex<4>`).
  - `match_join`: multiple branches unify under the same scoped domain instance (`match choice { 1 => 3, _ => 3 }`).
  - `const_name`: named constant carries the scoped domain instance (`const SCALAR: u64 in BelowIndex<4> = 3;`).
  - `other_state`: state transition carries the scoped value across states (`transition next(scoped)`).
  - `other_machine`: forwarding machine passes the scoped value across machine boundaries (`Helper::forward(scoped)`).
  - `target_body`: each of four target-scoped machine bodies establishes and consumes the scoped value.
  In each place, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ solely in the initializer literal (`3` vs `4`).
- **Soundness boundaries:** If the initializer literal violates the domain bound (`4 < 4` is false), the compiler strictly refuses to establish qualification without an authorized proof route (`cannot prove initializer ... an annotation cannot establish routed qualification`). The proof system strictly enforces carrier matching, non-empty rosters, and unviolated predicates, preventing invalid qualifications from reaching Terminal Psi or native code.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The domain qualification `u64 in BelowIndex<4>` uses the primitive `u64` carrier and is completely erased during lowering to canonical Terminal Psi (`PSITERM\0`). No runtime wrappers, dynamic assertions, or metadata overhead are introduced. All positive entries execute in the Terminal Psi interpreter (`omega run --both`), observing exit 0.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Companion build declarations bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans assign ordinary `u64` 8-byte scalar geometry (size 8, align 8). `CallPlan` and `StatePlan` place scalar values into standard integer calling registers (`rcx`/`rax` on x86-64 Windows, `rdi`/`rax` on x86-64 Linux, `x0` on ARM64).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend instruction selection emits standard integer move and subtraction/comparison instructions (`mov`, `sub`, `cmp`). Register allocation assigns general-purpose register homes and activation stack frame spill slots. Activation storage bounds (WCSU) are validated, and standard entry prologues and restoration epilogues are emitted.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Basic blocks and branch displacements relax with offset reflow. Transitive machine footprint evidence (`actual_transitive_footprint ⊆ permitted_transitive_use`) validates within platform limits. Native PE/ELF/Mach-O binary images are emitted directly and execute with native exit 0, matching the Terminal Psi interpreter.

## What remains open

- Direct field projection passing where a scoped-domain field is passed as a qualified call argument without an intermediate local binding (`Holder.scoped` directly into `exercise(scoped: u64 in BelowIndex<4>)`).
- Arithmetic operations on indexed domain bounds and dependent type equations over non-literal const-generic parameters.