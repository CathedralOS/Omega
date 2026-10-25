# Generic comparison body

A generic comparison body returns false. The call records that boolean, and the
caller reads the record.

Owning task: **OPERATOR-MACHINE-SUPPLY** (`TASKS.md`). The exact requirement this
row establishes is that item's `call_binding.rs` / body-result bullet — "A
generic comparison body that returns false is recorded on the call" — together
with the item's own sentence for this row:
"`samples/cli/facts/generic_comparison_body/local` exits 0 on both engines, and
the caller reads `direct == false`. Its twin changes only that body to `true`
and rejects. The other eight places of that sentence remain open." This folder
closes those eight places; it does not close the rest of
OPERATOR-MACHINE-SUPPLY.

Every place copies that sentence and the same operation. The generic body
`same<T>(left: T, right: T) -> bool { false }` returns a constant boolean; the
generic `compare<T>` forwards to `same<T>`, the call `compare<u64>(…)` records
that boolean as its own result, and the caller reads the record through
`hold(direct)` (`requires direct == false`) to reach `done()` (exit 0). The
reject twin changes only `same`'s own body from `false` to `true`, so the
recorded boolean flips and the same obligation is unprovable. Only the transport
of the comparison's operands changes.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the comparison operands are literals at a local site |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | both operands arrive as record fields |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | both operands arrive as explicit parameters |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a call result returns each operand |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield an operand |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants are the operands |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives both operands |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine supplies both operands |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | one body per hosted target uses the operands |

The row has no root `main.omg`; `local` is its canonical local witness. The
landed local plus twin came from the item's pin commit `037fe3b9a8b`.

## Execution

Every positive place binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its
`build.omg` and declares `Expected exit: 0`. `omega run --both` reports Terminal
Psi interpreter and native execution agreement:

```sh
target/debug/omega run --both samples/cli/facts/generic_comparison_body/<place>/main.omg < /dev/null
target/debug/omega run --both samples/cli/facts/generic_comparison_body/<place>/invalid/main.omg < /dev/null
python3 tools/sample_native_gate.py --facts --filter cli/facts/generic_comparison_body
python3 tools/corpus_gate.py --filter facts/generic_comparison_body
```

Measured at base `c1c6bc8cb8ec886f645976a82e496d4191937339` on macOS arm64
(Apple Silicon) with the shared opt-level 1 producer built at `9ff26a870215`
(`omega --build-profile` reports `opt_level: 1`).

- All nine positives report `native exit 0` with `interp exit: 0` through
  `omega run --both` (18 of 18 engine runs exit 0).
- All nine twins exit 200 at compile time with the row's own obligation —
  `cannot prove requires contract for call hold ... direct == false` — from
  `main` state `entry` (local, field, parameter, call result, match join,
  const name, another machine), `main` state `check` (a nested machine), and
  `observe` state `entry` for each of the four target bodies.
- `python3 tools/sample_native_gate.py --cli <producer> --facts --filter
  cli/facts/generic_comparison_body` reports 9/9 samples meeting their
  documented oracle on `macos_arm64` with 0 unscored, and 9/9 intended-invalid
  controls matching their authored rejection.

Main advanced to `8d202634513` before landing; the row was rebased onto it and
re-measured with an opt-level 1 CLI and corpus runner built from that tree:
18 of 18 engine runs exit 0, nine of nine twins reject with the same
fragments, `sample_native_gate.py` again reports 9/9 documented oracles and
9/9 matched intended-invalid controls, and the corpus re-diff is clean over
the 36 fixtures (18 `checked`, 18 `rejected`, 0 rejected without their
expected fragment; the direct `--both` records carry `interpreter-exit:0` and
`interpreter:agrees` for all 18 positives). The recorded golden rows were
unchanged and keep their `@c1c6bc8cb8ec+wt` stamp.

### The oracle discriminates

`hold(direct)` carries `requires direct == false`, and the program reaches
`machine done() {}` (exit 0) only through it; a true recorded boolean takes
`machine bad() { crash Trap; }`. The twin keeps the same caller, the same
obligation and the same operands, and changes only the generic body's own
returned literal: the refusal is the fact's own negative, not an unrelated
failure. The comparison's operands never select the result — the body-wide join
does — so no place proves the obligation from an operand equality guard.

## Reject Twins

Each place has an `invalid/` twin whose `sample.json` carries
`"role": "intended-invalid"` and `rejection_contains`:
`["cannot prove requires contract for call hold", "direct == false"]`. In each
twin, only `same`'s body changes from `false` to `true` (one line; verified by
`git diff` between the positive and the twin). The recorded boolean flips to
`true`, and the caller's `direct == false` obligation is unprovable:

| Place | Twin | Pinned diagnostic |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `cannot prove requires contract for call hold from main: direct == false … in state `entry`` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | same fragment, `from main`, state `entry` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | same fragment, `from main`, state `entry` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | same fragment, `from main`, state `entry` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | same fragment, `from main`, state `entry` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | same fragment, `from main`, state `entry` |
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | same fragment, `from main`, state `check` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | same fragment, `from main`, state `entry` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | same fragment per target body, `from observe` |

## Pinned Fixtures

The row is pinned by eighteen fixtures:
`tests/omega/pass/facts/generic_comparison_body_through_<place>_exit` (nine
positives, byte-identical copies of their place `main.omg`, each with a
`build.omg` binding the four hosted targets and `expected_native_exit.txt` of
`0`) and
`tests/omega/fail/facts/generic_comparison_body_through_<place>_rejected` (nine
twins, byte-identical copies of their twin `main.omg`, each with an
`expected.txt` carrying `cannot prove requires contract for call hold` and
`direct == false`).

Their golden rows were recorded by filtered splice at `c1c6bc8cb8ec+wt`:
+30 lines in `corpus_outcomes.txt` (nine `checked` rows with
`check-target:macos_arm64`, nine `rejected` rows with 12 diagnostic
continuation lines), +9 in `corpus_native_macos_arm64.txt` (nine `built` rows
with `exit:0`) and +9 in `corpus_both_macos_arm64.txt` (nine `built` `exit:0`
rows with `interpreter-exit:0` and `interpreter:agrees`); no other row moved
and no fixture key or diagnostic continuation line was lost.

## Requirements and Evidence

1. **Establishment:** the boolean is established where it originates — the
   generic body `same<T>` returns `false`, and the body-wide join records that
   constant on the call `compare<u64>(…)`. The caller reads the record through
   `hold(direct)`; nothing re-establishes `direct == false` in the caller with a
   post-hoc equality guard.
2. **Identity and lifetime:** the operands are exact `u64` values
   (`1u64`, `1u64`) with exact identities per place (literals, record fields,
   explicit parameters, call results, match joins, named constants,
   successor-state arguments, forwarded parameters, target-body operands). The
   generic instantiation `compare<u64>` keeps its identity in every place, and
   no place writes an operand between the call and the read.
3. **Composition:** the recorded boolean composes into the consuming obligation
   `hold(direct)` at all nine places. `<place>/main.omg` versus
   `<place>/invalid/main.omg` differ only in `same`'s own body value
   (`git diff` shows one line), and no twin adds a caller guard.
4. **Soundness boundaries:** all nine twins fail closed during compile-time
   proof checking with the row's own obligation (`direct == false`), listed
   above; a program that reads a true recorded boolean through the same
   obligation does not compile.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the generic comparison
   lowers to an ordinary selected call chain
   (`main`/`observe` → `compare` → `same`) with the recorded constant result;
   proof-only obligations are erased. The Terminal Psi interpreter executes all
   nine positives (`interp exit: 0`) in agreement with native execution.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
   every place and twin binds `ProgramEntry` for the four hosted targets; the
   corpus check leg realizes `macos_arm64` from those bindings. The operands and
   the recorded result are `u64`/`bool` scalars passed in registers; the generic
   application realizes its selected body as a direct call and creates no
   runtime dictionary.
7. **Instruction selection, register homes, and activation frame realization
   (`Omega 03–06`):** the executed images select the call/return path for the
   comparison chain and assign register homes and disjoint frame slots for the
   entry machine and its helpers, with the true arm routed to the `bad()` trap.
8. **Form encoding, branch relaxation, relocations, and native image emission
   (`Omega 07–09` + emission):** the emitted host image executes and agrees with
   Terminal Psi (`omega run --both`), and the twins reject before emission.

## What remains open

- The rest of **OPERATOR-MACHINE-SUPPLY** is untouched: the static
  callable/evidence argument and generic-dispatch formation checks, calls in
  abstract signatures beyond `abstract_signature_call`, domain and default
  predicates, citation/induction guarantees, the
  `OpenIndexOperationSelection` frontier, and the `operator` introducer
  retirement.
- Native execution on the four authored hosted targets is measured through the
  host the run executes (macOS arm64 here); `macos_arm64::ProgramEntry` is one
  of the four bindings, and the other three remain realized-by-binding, not
  executed on this host.