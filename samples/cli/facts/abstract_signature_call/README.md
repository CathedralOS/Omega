# Abstract signature call

A call inside an abstract signature is the premise. The signature records that
call's result, and the caller reads the record.

Owning task: **OPERATOR-MACHINE-SUPPLY** (`TASKS.md`). The exact requirement this
row establishes is that item's formation-checks bullet — the checks in
`omega-rust/psi/compilation/checking/src/proof_contracts/contract_entailment/specification_calls.rs`
and
`omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/checks/contracts/evidence.rs`
extended to **calls in abstract signatures** — together with the item's own
sentence for this row: "A call inside an abstract signature is the premise, and
the signature records that call's result: `samples/cli/facts/abstract_signature_call/local`
exits 0 on both engines, and the caller reads `direct == true`. Its twin changes
only that call's result from `true` to `false` and rejects `premise(kept)`. The
other eight places of that sentence remain open." This folder closes those eight
places; it does not close the rest of OPERATOR-MACHINE-SUPPLY.

Every place copies that sentence and the same operation. The abstract signature
`apply<machine Observe>(kept: bool) -> bool` carries the premise call
`requires premise(kept);` beside its own callable contract; instantiating
`apply<witness>` records that call's result and specializes the body to the
selected `witness` body. The entry machine reads the record through
`hold(direct)` (`requires direct == true`) and reaches `done()` (exit 0). The
reject twin changes only `premise`'s own body from `true` to `false`, so the
signature's premise is structurally false and the same call is refused. Only the
transport of the premise subject and its read changes.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the premise subject is an entry local and the caller reads the record locally |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the premise subject is stored in a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the premise subject arrives as an explicit parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a call result supplies the premise subject |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the premise subject |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant is the premise subject |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives the premise subject and reads the record there |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine supplies the premise subject |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | one body per hosted target uses the subject and reads the record |

The row has no root `main.omg`; `local` is its canonical local witness. The
landed local plus twin came from the item's pin commit `49bf309359c`.

## Execution

Every positive place binds `ProgramEntry` for all four hosted targets
(`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) in its
`build.omg` and declares `Expected exit: 0`. `omega run --both` reports Terminal
Psi interpreter and native execution agreement:

```sh
target/debug/omega run --both samples/cli/facts/abstract_signature_call/<place>/main.omg < /dev/null
target/debug/omega run --both samples/cli/facts/abstract_signature_call/<place>/invalid/main.omg < /dev/null
python3 tools/sample_native_gate.py --facts --filter cli/facts/abstract_signature_call
python3 tools/corpus_gate.py --filter facts/abstract_signature_call
```

Measured at base `c1c6bc8cb8ec886f645976a82e496d4191937339` on macOS arm64
(Apple Silicon) with the shared opt-level 1 producer built at `9ff26a870215`
(`omega --build-profile` reports `opt_level: 1`).

- All nine positives report `native exit 0` with `interp exit: 0` through
  `omega run --both` (18 of 18 engine runs exit 0).
- All nine twins exit 200 at compile time naming the row's own premise — the
  `premise(kept)` violation with `the instantiated fact is structurally false`
  — from `main` state `entry` (local, field, call result, match join, const
  name), `main` state `check` (another state), and `observe` state `entry`
  (parameter, another machine, and each of the four target bodies), each
  beside the specialized `apply$specialized$b37cdd08ebba4760$0` companion.
- `python3 tools/sample_native_gate.py --cli <producer> --facts --filter
  cli/facts/abstract_signature_call` reports 9/9 samples meeting their
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

`hold(direct)` carries `requires direct == true`, and the program reaches
`state done() {}` (exit 0) only through it; any other result takes
`state bad() { crash Trap; }`. The twin keeps the same caller, the same
obligation and the same subject, and changes only the premise call's own result:
the refusal is the fact's own negative, not an unrelated failure. A supplied
caller-side guard would be a different fact and is not used anywhere in this
row.

## Reject Twins

Each place has an `invalid/` twin whose `sample.json` carries
`"role": "intended-invalid"` and `rejection_contains`:
`["structurally false", "premise(kept)"]`. In each twin, only `premise`'s body
changes from `true` to `false` (one line; verified by `git diff` between the
positive and the twin). The call inside the abstract signature then instantiates
to a structurally false fact and proof checking fails closed:

| Place | Twin | Pinned diagnostic |
| --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | `machine `main` state `entry` value call to `apply` violates required fact `premise(kept)`: the instantiated fact is structurally false` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | same fragment from `main` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | same fragment from `observe` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | same fragment from `main` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | same fragment from `main` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | same fragment from `main` |
| another state | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | same fragment from `main` state `check` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | same fragment from `observe` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | same fragment per target body from `observe` |

Every twin also emits the companion diagnostic from the specialized application
`apply$specialized$…` state `entry` calling `witness` with the same required
fact, so the refusal names both the instantiation and the specialized body.

## Pinned Fixtures

The row is pinned by eighteen fixtures:
`tests/omega/pass/facts/abstract_signature_call_through_<place>_exit` (nine
positives, byte-identical copies of their place `main.omg`, each with a
`build.omg` binding the four hosted targets and `expected_native_exit.txt` of
`0`) and
`tests/omega/fail/facts/abstract_signature_call_through_<place>_rejected` (nine
twins, byte-identical copies of their twin `main.omg`, each with an
`expected.txt` carrying `structurally false` and `premise(kept)`).

Their golden rows were recorded by filtered splice at `c1c6bc8cb8ec+wt`:
+39 lines in `corpus_outcomes.txt` (nine `checked` rows with
`check-target:macos_arm64`, nine `rejected` rows with 21 diagnostic
continuation lines), +9 in `corpus_native_macos_arm64.txt` (nine `built` rows
with `exit:0`) and +9 in `corpus_both_macos_arm64.txt` (nine `built` `exit:0`
rows with `interpreter-exit:0` and `interpreter:agrees`); no other row moved
and no fixture key or diagnostic continuation line was lost.

## Requirements and Evidence

1. **Establishment:** the premise is established where it originates — the call
   `premise(kept)` inside the abstract signature — and its result is recorded on
   the instantiated application. The caller reads that record through
   `hold(direct)`; nothing re-establishes `direct == true` in the caller with a
   post-hoc equality guard.
2. **Identity and lifetime:** the premise subject is one exact `bool` identity
   per place (entry local, record field, explicit parameter, call result, match
   join, named constant, successor-state argument, forwarded parameter, target
   body local). The abstract signature `apply<machine Observe>` keeps its
   identity in every place and is specialized once
   (`apply$specialized$b37cdd08ebba4760$0` names the same specialized
   application in every twin diagnostic). No place writes the subject between
   establishment and the read.
3. **Composition:** the recorded result composes into the consuming obligation
   `hold(direct)` at all nine places. `<place>/main.omg` versus
   `<place>/invalid/main.omg` differ only in `premise`'s own body value
   (`git diff` shows one line), and no twin adds a caller guard or a second
   premise.
4. **Soundness boundaries:** all nine twins fail closed during compile-time
   proof checking with the row's own diagnostics listed above. The refusal is
   the abstract signature's instantiated premise, not an unrelated missing
   import or stale setup; a program that assumes the premise while it is false
   does not compile.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the abstract dispatch is
   specialized to the selected `witness` body and lowered to an ordinary
   selected call chain (`main`/`observe` → `apply` → `witness` → `premise`);
   proof-only obligations are erased. The Terminal Psi interpreter executes all
   nine positives (`interp exit: 0`) in agreement with native execution.
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
   every place and twin binds `ProgramEntry` for the four hosted targets; the
   corpus check leg realizes `macos_arm64` from those bindings. The premise
   subject and the recorded result are `bool` scalars passed in registers; the
   abstract application realizes its selected body as a direct call and creates
   no runtime dictionary.
7. **Instruction selection, register homes, and activation frame realization
   (`Omega 03–06`):** the executed images select the call/return path for the
   selected `witness` body and assign register homes and disjoint frame slots
   for the entry machine and its helpers, with the `true` arm routed to the
   `bad()` trap.
8. **Form encoding, branch relaxation, relocations, and native image emission
   (`Omega 07–09` + emission):** the emitted host image executes and agrees with
   Terminal Psi (`omega run --both`), and the twins reject before emission.

## What remains open

- The rest of **OPERATOR-MACHINE-SUPPLY** is untouched: the static
  callable/evidence argument and generic-dispatch formation checks, domain and
  default predicates, citation/induction guarantees, the
  `OpenIndexOperationSelection` frontier, and the `operator` introducer
  retirement.
- Native execution on the four authored hosted targets is measured through the
  host the run executes (macOS arm64 here); `macos_arm64::ProgramEntry` is one
  of the four bindings, and the other three remain realized-by-binding, not
  executed on this host.