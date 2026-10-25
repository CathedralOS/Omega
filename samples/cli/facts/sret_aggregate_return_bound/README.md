# Indirect `sret` Aggregate Return Bound Realization

Parent task: **NORMALIZED-ABI-LOWERING** / **CONSERVATION-CONTRACT** (`TASKS.md`).

## Governing requirement

Returning a 24-byte three-field structural record (`Triple { a: u64, b: u64, c: u64 }`) through an indirect `sret` aggregate return slot preserves its postcondition bound (`result.c <= 90u64`), discharging exact subtraction `90u64 - result.c` across all nine places.

**Fact: returning a 24-byte three-field structural record (Triple { a: u64, b: u64, c: u64 }) through an indirect sret aggregate return slot preserves its postcondition bound (result.c <= 90u64), discharging exact subtraction 90u64 - result.c across all nine places.**

Every place performs the same operation: construct and return a 24-byte (three-`u64`-field) structural record `Triple` whose fields specify `a == 20u64`, `b == 30u64`, and `c == 40u64` (with bound `c <= 90u64`). Because 24 bytes exceeds the 16-byte register return pair limit across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`), returning `Triple` by value exercises indirect aggregate return (`sret`) lowering into caller-allocated stack frame return homes via target-mandated hidden destination pointer registers (`RCX` on `windows_x86_64`, `RDI` on `linux_x86_64`, and `X8` on `macos_arm64`/`linux_arm64`). Each place transports `Triple` by value across one of nine canonical program locations, performs exact subtraction `remaining = 90u64 - t.c` (`90 - 40 = 50`) followed by `remaining - (t.a + t.b)` (`50 - 50 = 0`), and transitions to `done()` with exit code `0`. Only the transport mechanism of the aggregate operand changes across places.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| Root | [`main.omg`](main.omg) | — | Direct indirect aggregate return and field subtraction |
| Local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | Aggregate returned by value is bound to a local before field projection and subtraction |
| Field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | Aggregate returned by value is stored into an enclosing record field before projection |
| Parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | Aggregate returned by value is passed by value as an explicit parameter |
| Call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | Aggregate returned by value is forwarded from an indirect helper call |
| Match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | Both match arms execute indirect aggregate returns and join into an aggregate |
| Const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | Named constants define the field values returned in the 24-byte aggregate |
| A nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | A successor machine receives the returned aggregate by value and projects its fields |
| Another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | An intermediate forwarding machine transports the returned aggregate by value |
| Another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | Target-scoped machine bodies execute indirect aggregate returns on each hosted target |

### Verification commands

Root `main.omg` and all nine place projects bind `ProgramEntry` for all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`), declare `Expected exit: 0`, and carry `expected_native_exit.txt` (`0`). Run any positive directly:

```sh
omega run --both samples/cli/facts/sret_aggregate_return_bound/main.omg
omega run --both samples/cli/facts/sret_aggregate_return_bound/local/main.omg
omega run --both samples/cli/facts/sret_aggregate_return_bound/field/main.omg
omega run --both samples/cli/facts/sret_aggregate_return_bound/parameter/main.omg
omega run --both samples/cli/facts/sret_aggregate_return_bound/call_result/main.omg
omega run --both samples/cli/facts/sret_aggregate_return_bound/match_join/main.omg
omega run --both samples/cli/facts/sret_aggregate_return_bound/const_name/main.omg
omega run --both samples/cli/facts/sret_aggregate_return_bound/other_state/main.omg
omega run --both samples/cli/facts/sret_aggregate_return_bound/other_machine/main.omg
omega run --both samples/cli/facts/sret_aggregate_return_bound/target_body/main.omg
```

Each place has an `invalid/` twin whose `c` field bound and value is `91u64` instead of `40u64` (violating `c <= 90u64`), causing exact subtraction `remaining = 90u64 - t.c` to fail proof checking with `may overflow `u64`` (`exact subtraction underflow`). Check each twin with:

```sh
omega --check samples/cli/facts/sret_aggregate_return_bound/<place>/invalid/main.omg
```

## Requirements and Evidence

Owning requirement: `wiki/spec/layouts/plans.md#structural-records`, `wiki/spec/build/calling_plans.md#indirect-aggregate-return-sret`, and `wiki/spec/terminal-psi/integer_certificates.md#canonical-scalar-goals`.
Parent task: **NORMALIZED-ABI-LOWERING** / **CONSERVATION-CONTRACT** (`TASKS.md`).
This row establishes that returning a 24-byte three-field structural record (`Triple`) by value preserves its refined postcondition field bounds across caller-allocated stack return homes via the target ABI's indirect `sret` calling plan, discharging exact subtraction `90u64 - t.c` across all nine places.

- **Establishment:** Constructing `Triple { a: 20, b: 30, c: 40 }` establishes field refinement bounds `a == 20`, `b == 30`, and `c == 40`, fulfilling postcondition `ensures result.c <= 90u64 && result.c == 40u64`. This bound directly proves that exact subtraction `remaining = 90u64 - t.c` (`90 - 40 = 50`) and subsequent subtraction `remaining - (t.a + t.b)` (`50 - 50 = 0`) cannot underflow `u64` without requiring any post-hoc caller equality guard.
- **Identity and lifetime:** The 24-byte structural record `Triple` is passed and returned by value across machine boundaries. Because 24 bytes exceeds the 16-byte register return limit across all hosted ABIs, the aggregate is returned into a caller-allocated stack frame return home via an indirect hidden pointer. Field offsets (`a` at +0, `b` at +8, `c` at +16) and bit-exact scalar identities are preserved across caller storage without memory corruption.
- **Composition:** The indirect `sret` return value composes through all nine standard program places (`local`, `field`, `parameter`, `call_result`, `match_join`, `const_name`, `other_state`, `other_machine`, `target_body`), feeding into the exact subtraction `90u64 - t.c` and returning exit code 0. In every place, the positive and reject twin differ solely in the value and bound of `c` (`40u64` vs. `91u64`), keeping all return mechanisms identical.
- **Soundness boundaries:** All nine `<place>/invalid/main.omg` reject twins fail closed at compile time: setting `c = 91u64` violates the subtraction precondition (`90u64 - t.c` would underflow `u64`), causing the compiler's range check and integer proof checker to reject with `error: exact arithmetic in machine ... may overflow `u64`` (`exact subtraction underflow`). Static verification enforces that unproved subtraction obligations cannot be admitted or executed.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Target-neutral Terminal Psi representation (`PSITERM\0`) models aggregate construction and returns as structured value expressions, preserving field access projections and integer certificates while erasing proof-only obligation tags. The Terminal Psi interpreter evaluates aggregate return by value and exact subtraction, agreeing identically with native execution (`omega run --both`, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows join `ProgramEntry` declarations across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Layout geometry for `Triple` calculates `size_of = 24`, `align_of = 8`. Because 24 > 16, target `CallPlan` classifies the return as indirect aggregate return (`sret`), assigning hidden destination pointer registers (`RCX` on `windows_x86_64`, `RDI` on `linux_x86_64`, and `X8` on `macos_arm64`/`linux_arm64`).
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** The backend allocates caller stack space for the 24-byte return home, passes its address in the target's designated `sret` pointer register, and emits callee stores writing each 8-byte field into `[sret + 0]`, `[sret + 8]`, and `[sret + 16]`. Entry prologues and return epilogues honor the `StatePlan` ceiling, computing worst-case stack usage (WCSU).
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic displacements resolve, basic blocks and branch offsets reflow across entry and return epilogues, transitive machine-state footprint evidence is validated (`actual_transitive_footprint ⊆ permitted_transitive_use`), and direct PE/ELF/Mach-O images are emitted, achieving native execution agreement with Terminal Psi (`omega run --both`, exit 0) across all hosted targets.

The row is verified by `samples/cli/facts/sret_aggregate_return_bound` where every positive records native exit 0 and Terminal Psi execution agreement (`omega run --both`), and every invalid twin rejects with `may overflow `u64``.
What remains open: Dynamic stack frame elision for tail-call forwarded indirect aggregate return destinations.

## Measured evidence

Producer revision `ab9a94dfb6a5` (fixture commit), macOS arm64 host, optimized dev-profile `omega`.
All ten projects (root plus nine places) were measured with `omega run --both` and reported
`native exit 0` with `interp exit: 0`; no place refused or disagreed.

- `python3 tools/sample_native_gate.py --filter cli/facts/sret_aggregate_return_bound` —
  10/10 samples meet their documented oracle on `macos_arm64` (0 unscored),
  9/9 intended-invalid controls match their authored rejection.
- `python3 tools/corpus_gate.py --native --filter facts/sret_aggregate_return_bound` —
  nine fixtures, all `built` with `exit:0`, executed 9 (`exit codes {'0': 9}`),
  0 rejected without the expected fragment, 0 fail fixtures failing to reject.
  The nine rows are recorded in `tests/omega/corpus_native_macos_arm64.txt`.

### Oracle discrimination

The oracle was proved to discriminate before it was recorded. The positive place shape
returns its `Triple` through a caller-supplied member and consumes it in the
defined-overflow `Trapping` domain, so the value that crosses the indirect `sret` home
determines the observed result without any caller equality guard:

- `probe(40u64)` — `native exit 0`, `interp exit: 0`.
- `probe(41u64)` — only the crossing value changed: `native signal 5`,
  `interp crash: Trap (host presentation agrees)`.

Being a static layer on top, the `where` domain gate and the body-derived postcondition
also reject member corruption at compile time: a caller-visible member changed to a value
outside the callee's declared postcondition is reported as
`the instantiated fact is structurally false (500 == 40)`. The proof therefore derives the
returned member from the callee's body rather than trusting the declared `ensures`.

### Fixtures

Nine positive places are pinned under `tests/omega/pass/facts/sret_aggregate_return_bound_through_<place>_exit`
(with `expected_native_exit.txt` `0`), and nine reject twins under
`tests/omega/fail/facts/sret_aggregate_return_bound_through_<place>_rejected`
(with `expected.txt` `may overflow `u64``). Each twin differs from its positive only in the
construct's own returned member value and declared bound (`40u64` versus `91u64`), never in a
caller guard.
