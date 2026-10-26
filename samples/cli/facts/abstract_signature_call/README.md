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
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the premise subject and reads the record there |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine supplies the premise subject |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | one body per hosted target uses the subject and reads the record |

The row has no root `main.omg`; `local` is its canonical local witness. The
landed local plus twin came from the item's pin commit `49bf309359c`.

## Constant-initializer production

The same fact also originates in the closed application
`const DIRECT: bool = apply<witness>(true)`. The
[`constant_initializer/main.omg`](constant_initializer/main.omg) root preserves
its frozen authored source, including `hold(DIRECT)` through its local read;
its [twin](constant_initializer/invalid/main.omg) changes only `premise`'s result
to `false`. Source SHA-256:
`63c626098e6ab12f1f0b5d4e887202f43fe1b6df2895350878b1b45605e83555`.

This production carries the resulting value through the same nine places.
Each positive and twin keeps the exact constant application, selected `witness`
body, abstract signature premise, and `hold` operation. Parameter and relay
contracts transport the exact `direct == true` obligation; they do not add a
caller guard. Nested machines declare their own crash routes explicitly.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [`constant_initializer/local/main.omg`](constant_initializer/local/main.omg) | [`constant_initializer/local/invalid/main.omg`](constant_initializer/local/invalid/main.omg) |
| field | [`constant_initializer/field/main.omg`](constant_initializer/field/main.omg) | [`constant_initializer/field/invalid/main.omg`](constant_initializer/field/invalid/main.omg) |
| parameter | [`constant_initializer/parameter/main.omg`](constant_initializer/parameter/main.omg) | [`constant_initializer/parameter/invalid/main.omg`](constant_initializer/parameter/invalid/main.omg) |
| call_result | [`constant_initializer/call_result/main.omg`](constant_initializer/call_result/main.omg) | [`constant_initializer/call_result/invalid/main.omg`](constant_initializer/call_result/invalid/main.omg) |
| match_join | [`constant_initializer/match_join/main.omg`](constant_initializer/match_join/main.omg) | [`constant_initializer/match_join/invalid/main.omg`](constant_initializer/match_join/invalid/main.omg) |
| const_name | [`constant_initializer/const_name/main.omg`](constant_initializer/const_name/main.omg) | [`constant_initializer/const_name/invalid/main.omg`](constant_initializer/const_name/invalid/main.omg) |
| other_state | [`constant_initializer/other_state/main.omg`](constant_initializer/other_state/main.omg) | [`constant_initializer/other_state/invalid/main.omg`](constant_initializer/other_state/invalid/main.omg) |
| other_machine | [`constant_initializer/other_machine/main.omg`](constant_initializer/other_machine/main.omg) | [`constant_initializer/other_machine/invalid/main.omg`](constant_initializer/other_machine/invalid/main.omg) |
| target_body | [`constant_initializer/target_body/main.omg`](constant_initializer/target_body/main.omg) | [`constant_initializer/target_body/invalid/main.omg`](constant_initializer/target_body/invalid/main.omg) |

The nine places are pinned as
`tests/omega/{pass,fail}/facts/abstract_signature_call_constant_initializer_through_<place>_{exit,rejected}`;
the unchanged root is pinned by the corresponding
`abstract_signature_call_constant_initializer_{exit,rejected}` fixtures. The
positive fixtures author exit 0, and the rejects require `structurally false`
and `premise(kept)`.

The fixed baseline `aa202339cb2f513fbf478efa46f212a6d94680fd` refuses the
unchanged root before execution with
`constant initializer call has no exact ordinary machine declaration: selected MachineParameter`.
The optimized baseline CLI digest is
`c17e6b7a283b3e6898b975a7f8b596d17422dc1132f0fe8d4907bb8fca1e7c45`;
qualification receipt `cec7a06e6adb4037aa71cf85c2ba67d7` retains exit 1,
the source/oracle hashes, and no invalidations. No baseline runtime is claimed.

### Callable composition controls

[`forwarded`](constant_initializer/forwarded/main.omg) passes the selected
callback through another static binder before evaluating the same application.
[`selected_entries`](constant_initializer/selected_entries/main.omg) evaluates
both `apply<witness>(true)` and `apply<alternate>(true)`: their signatures agree,
but their bodies return opposite values, and `hold`/`hold_false` consume those
exact results. Both have premise-false twins and corpus pins. The
[`wrong_result`](constant_initializer/wrong_result/main.omg) control instead
keeps the premise true and changes only the selected `witness` result to false;
it must refuse `hold`'s `direct == true` contract. This checks the oracle and
selected body independently of the premise refusal.

### Statement and terminal call productions

Independent review of candidate `97d993169881f1071faf16ac700e00ebface2d01`
found two additional valid productions of this same constant call:
[`terminal_callback`](constant_initializer/terminal_callback/main.omg) forwards
`Observe(kept)` through a terminal transition, and
[`statement_explicit_discard`](constant_initializer/statement_explicit_discard/main.omg)
explicitly discards the call before returning `true`. Both preserve the frozen
premise, selected callback, constant and exit-0 oracle. Their pre-repair
constant forms refuse respectively with `constant call differs from its exact
authored selection` and `authored statement call reclassification found duplicate
selections for the same source token`, while their runtime controls execute in
both engines. Frozen source hashes are respectively
`2c740bc17e25f12d6c0137408a17bf1fa676c2ff309ff422c8a08b4d11ad10a0`
and `5c1d78452b23dae080d466a3fc98ec019f8ff03deb101c18acb1aac20c14203a`.

Both productions have premise-false twins and corpus pins. Their required
repair carries the original binder on the shared call-occurrence view in every
call position and retains one authored selection across recipe, materialized,
expression and statement copies. Duplicate or inconsistent selections remain
errors. The rejected C1 review and its raw independent observations are retained.
At repaired source `74b572b3e1769346ef76cc50f24ce138b78a14b3`, both frozen
productions execute with native exit 0 and Terminal exit 0, and both premise-false
twins refuse the intended obligation. This is the same fact and transport row;
the additional call positions do not create another nine-place matrix.

### Calls inside the supplied body

Independent review of `0cb9d9fe3c9272d60ca402aab5c2353bd5e4718f` found that
ordinary terminal calls inside the selected callback still lose their authored
callable-entry selection. Frozen
[`supplied_body_terminal`](constant_initializer/supplied_body_terminal/main.omg)
and [`unexecuted_body_dependency`](constant_initializer/unexecuted_body_dependency/main.omg)
retain the same callback, premise, constant and `hold` consumer. The second
also retains a call in an unexecuted branch, so source custody must cover the
whole selected body. Both constant forms refuse `constant call differs from its
exact authored selection`; separate runtime-oracle controls observe native 0 and
Terminal 0. An initial runtime variant that retained `hold` could not prove its
result contract and supplies no runtime evidence.

Both frozen positives have premise-false twins and matching corpus pins. The
rejected C3 review remains immutable. This closure repair rejoins the exact
machine's callable entry declaration while terminal invocation keeps its machine
identity; a matching name or signature grants no selection. At source
`a8b77070b93c27ae30c01988bf046f2b0ee90c8f`, both frozen positives execute with
native exit 0 and Terminal exit 0. Their twins now reject the intended premise,
rather than being masked by the custody error. The source dependency producer
emits that exact callable entry too, so recipe and body copies retain one
consistent authored selection. Deferred body demand still grants no call
admission.

### Requirement accounting for this production

- **Establishment:** the closed invocation evaluates only after ordinary checked
  specialization and concrete premise/failure admission. `premise(kept)` is an
  obligation before the result exists; it never establishes itself.
- **Identity and lifetime:** dependency discovery records the source binder and
  explicit callback body separately. Replay rejoins the original binder to the
  exact entry in its owning specialization. The constants are immutable value
  snapshots; the composition controls distinguish equal ABI signatures.
- **Composition:** the same computed result reaches `hold` through all nine
  places. Parameters and relay contracts prove arrival under the actual
  constant substitution; joins keep the same result in both arms. No caller
  guard replaces the carried fact.
- **Soundness boundaries:** every premise-false twin refuses its own obligation,
  and the wrong-result control refuses the consumer's required result. These
  closed Boolean computations acquire no authority, borrow no storage, and
  introduce no runtime service reach; other custody modes remain outside this
  row's evidence.
- **Psi 05–07:** constant evaluation retains and validates authored call custody
  before materializing the Boolean. The resulting program lowers through
  Terminal Psi, whose interpreter must observe exit 0 beside native execution.
- **Omega 00–02:** all positives retain the four hosted `ProgramEntry` bindings
  and add `macos_x86_64` for this host. Static callback binders are erased; the
  result uses the ordinary Boolean carrier and call placement.
- **Omega 03–06:** the Intel host exercises scalar/record homes, calls, joins,
  nested-machine arrivals, and the consumer's success/crash control flow.
- **Omega 07–09 and emission:** actual host Mach-O execution must agree with
  Terminal Psi. The other four authored targets have no runtime observation
  on this Intel host and remain outside this run's platform qualification.

## Applied generic callback production

The next production under **OPERATOR-MACHINE-SUPPLY** keeps the same premise,
selected body and `hold` operation, and supplies the callback's explicit static
tuple: `const DIRECT: bool = apply<witness<1>>(true)`, with
`witness<const Marker: u64>`. The unused parameter still owes its declared kind,
carrier and bounds. The frozen [root](constant_initializer/applied_callback/main.omg)
has source SHA-256
`e5383eb6e400c986fd0c5a1b5b93b5da04b0881b8a993c214266abca86415b7b`.
The optimized producer from fixed source
`37f85b4aa3810cfb579257afb37cc8b99508c84f` refuses it before execution with
`a nested machine application without a completely admitted callable tuple`.
That is the formation gap after the constant-custody repair in PR 253.
The first repair candidate, `f2dcd8f0c3adc0ed943d2987aa49af7097cf1f87`,
executes the unchanged root with native exit 0 and Terminal exit 0. The full
new production is now pinned at optimized producer
`3c2ec6aaf4ddff261c9d25f85b2a664a440a3da0`: its root, nine transports and six
compositions observe native exit 0 and Terminal exit 0 on Intel macOS, and its
22 invalid controls reach their intended obligations. Full-fact public acceptance,
affected-consumer checks and exact independent review remain in the current
qualification ledger; these observations do not close the parent capability.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local](constant_initializer/applied_callback/local/main.omg) | [invalid](constant_initializer/applied_callback/local/invalid/main.omg) |
| field | [field](constant_initializer/applied_callback/field/main.omg) | [invalid](constant_initializer/applied_callback/field/invalid/main.omg) |
| parameter | [parameter](constant_initializer/applied_callback/parameter/main.omg) | [invalid](constant_initializer/applied_callback/parameter/invalid/main.omg) |
| call result | [call result](constant_initializer/applied_callback/call_result/main.omg) | [invalid](constant_initializer/applied_callback/call_result/invalid/main.omg) |
| match join | [match join](constant_initializer/applied_callback/match_join/main.omg) | [invalid](constant_initializer/applied_callback/match_join/invalid/main.omg) |
| const name | [const name](constant_initializer/applied_callback/const_name/main.omg) | [invalid](constant_initializer/applied_callback/const_name/invalid/main.omg) |
| nested machine | [other state](constant_initializer/applied_callback/other_state/main.omg) | [invalid](constant_initializer/applied_callback/other_state/invalid/main.omg) |
| another machine | [other machine](constant_initializer/applied_callback/other_machine/main.omg) | [invalid](constant_initializer/applied_callback/other_machine/invalid/main.omg) |
| target body | [target body](constant_initializer/applied_callback/target_body/main.omg) | [invalid](constant_initializer/applied_callback/target_body/invalid/main.omg) |

Each twin changes only the premise's own result to false. The nine transports
and root are pinned under
`tests/omega/{pass,fail}/facts/abstract_signature_call_applied_callback_*`.
The same selected-tuple mechanism also has positive compositions for an unused
[type argument](constant_initializer/applied_callback/type_argument/main.omg),
an unused [copy bound](constant_initializer/applied_callback/type_bound/main.omg),
[opposite results from two applications of one provider](constant_initializer/applied_callback/opposite_instances/main.omg),
[a repeated recipe](constant_initializer/applied_callback/repeated_recipe/main.omg)
and [forwarding through another binder](constant_initializer/applied_callback/forwarded_applied/main.omg),
plus an unused [structural array type](constant_initializer/applied_callback/structural_type_argument/main.omg).
Each has a premise-false twin. Additional controls pin an unused type-bound
violation, an out-of-range const argument, a stronger callback premise, a wider
crash contract, and an incorrect result selected by a different application.
Those controls must reach their own obligations; a missing tuple is insufficient.

The structural array production exposed a second declaration-context gap after
the numeric and named-type cases passed. The unchanged source
`apply<witness<[u8; 4]>>(true)` still refused at fixed source 37 and the historical
CCA producer with no retained caller type/lifetime context. A private checking
projection omitted its compiler-created probe root, while arena span relocation
left four identical declaration headers. Lexical type formation now reads the
exact supplying declaration independently of executable root membership. All
retained copies must agree on the complete declaration; conflicting revisions
refuse, and reading a header grants no executable body. The structural positive
now observes both exits at 0, its [premise-false twin](constant_initializer/applied_callback/structural_type_argument/invalid/main.omg)
rejects the premise, and an [undeclared lifetime](constant_initializer/applied_callback/undeclared_structural_lifetime/main.omg)
still rejects ordinary type formation. This region-free array and the negative
lifetime control do not establish general projected reference-lifetime support.

The repair prepares the ordinary occurrence-owned provider view for retained
constant recipes. It preserves the authored arguments for replay and consumes
the selected roster only in executable occurrences. Full tuple formation and
callable refinement remain separate checking obligations. The complete row,
independent review and publication remain to be established.

Further valid-positive refusals are retained against fixed baseline 37
and the historical first candidate `f2dcd8f`: a provider tuple containing a Machine argument refuses
`an unclosed machine or runtime operand`, a cross-module const alias loses its
exact declaration selection, and a generic abstract-premise call confuses its
actual and required `T`. Their current producer behavior has not been re-observed;
they remain the same parent's next formation work.
This Type/Const application step does not claim those repairs. The broader
task's law, declaration migration and other-host acceptance remain open.

The bounded existing contextual-call check also preserves two inherited
open-Const refusals:
[`contextual_static_machine_const_scopes_exit`](../../../../tests/omega/pass/generics/contextual_static_machine_const_scopes_exit/main.omg)
and [`contextual_static_machine_open_const_exit`](../../../../tests/omega/pass/generics/contextual_static_machine_open_const_exit/main.omg).
Both fixed producer 37 and current producer `3c2ec6aaf4dd` refuse those positives
before execution. All 38 selected existing fixtures retain exactly the same
status, diagnostic, facts and oracle fields, including three inherited diagnostic
movements. The raw gate exit remains 3, with no oracle or infrastructure errors;
it is an attributed inherited failure, not a clean corpus or an accepted language
restriction. Open-Const families remain part of the parent formation acceptance.

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
  name), `main` state `check` (a nested machine), and `observe` state `entry`
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
`machine done() {}` (exit 0) only through it; any other result takes
`machine bad() { crash Trap; }`. The twin keeps the same caller, the same
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
| a nested machine | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | same fragment from `main` state `check` |
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
   for the entry machine and its helpers, with the `true` arm routed to `done()` and the other arm to the
   `bad()` trap.
8. **Form encoding, branch relaxation, relocations, and native image emission
   (`Omega 07–09` + emission):** the emitted host image executes and agrees with
   Terminal Psi (`omega run --both`), and the twins reject before emission.

## Fixed-base constant-production observations

At source checkpoint `289c0e2e27400b3f9c2640b0fea7504807b254fd`, the optimized
CLI SHA-256 is
`98362f018b493b0774979dc4ccaa65ef90e04932457d5b279de9b2b836ba45aa`,
and the corpus runner is
`27c25a0d0901cab630aeec55f3dea9d107d121215cc620ea9745b956b292f975`.
Their generation record binds the compiler, library, Cargo, and toolchain
inputs; both report opt-level 1. The focused corpus `--both` observation covers
21 positive fixtures (the prior nine, this production's root and nine places,
and two composition controls): each native and Terminal run exits 0 and the
engines agree. Its initial verdict is additions to the Intel host golden,
with no lost successes or runtime oracle failures; the Jev advisory is
`record_safe`.

The preliminary public sample gate reports 21/21 native positives and 22/22
intended-invalid controls, using the earlier implementation producer
`be69a36903ae7c7d63d36f9ae186ce4896d867fd618eaa6a07d7cca7f27d04c9`.
That observation retains its actual producer identity. The pinned qualification ledger, raw engine streams, prepared locks, source
snapshots, and required independent review use `build/swarm/operator/`; the PR
identifies their exact submitted candidate. Package preparation is completed for nested
projects from leaves to root before frozen execution captures.

### Statement and terminal repair observations

The repaired optimized source generation is
`74b572b3e1769346ef76cc50f24ce138b78a14b3`: CLI SHA-256
`d96499467e1ac027b9a5b3f81c3dba187d08940aa48702cdd9ea15c5fc65a094`,
corpus runner SHA-256
`aa0fc7bdecd39ce7acf635f90840f1dfec90ae5d0d918182b512daa04ec3a70d`.
The focused corpus checks 23 positives and rejects 24 intended-invalid controls
with every expected refusal fragment. All 23 positive fixtures execute with
native exit 0, Terminal exit 0 and engine agreement on `macos_x86_64`.
The count includes the prior nine places, the constant root and nine places,
forwarded and opposite-body controls, and the statement and terminal productions.
The four affected crates pass scoped `mbx check` and `mbx clippy`.

A bounded 42-fixture constants check also exercised the affected normalization
consumer. Its three existing red or stale-control cases and its three semantic
moves against the older golden were reproduced on the immutable fixed-AA CLI,
with unchanged fixture and library inputs. Those baseline observations retain
their own source identities; this change records no unrelated constants golden.
These observations do not establish that the whole constants domain is green.
The preserved ledger, exact producer receipts, raw baseline and current streams,
accepted locks and independent review remain under `build/swarm/operator/`.

### Supplied-body closure repair observations

At optimized source `a8b77070b93c27ae30c01988bf046f2b0ee90c8f`, CLI SHA-256
`fc69138f217ea07c4c79d62097f8ff56811de4350194eb1b6167df606c8967ac`
and corpus runner SHA-256
`08b93723e7daf5083085164a7ba651199d5e7ce2645d80b468b66713de2d3ccc`,
the focused source corpus checks 25 positives and rejects 26 controls for their
expected reasons. All 25 positive corpus fixtures execute with native exit 0,
Terminal exit 0 and agreement on `macos_x86_64`. The prior source rows do not
move; only the two supplied-body positive/twin pairs and two runtime observations
are new. Four affected crates pass `mbx check` and `mbx clippy`.

All 42 bounded constants observations have the same status, facts, expected
fragment result and diagnostics as the earlier C2 measurement. The six AA
baseline attributions above therefore still apply; no unrelated constants rows
are recorded. The rejected C1/C3 reviews, both source generations and all
nonaccepted intermediate observations retain their original identities.

## What remains open

- The rest of **OPERATOR-MACHINE-SUPPLY** is untouched: the static
  callable/evidence argument and generic-dispatch formation checks, domain and
  default predicates, citation/induction guarantees, the
  `OpenIndexOperationSelection` frontier, and the `operator` introducer
  retirement.
- The current constant-production runtime evidence is from the Intel
  `macos_x86_64` host, in addition to the historical Apple Silicon observations
  above. All four required hosted bindings remain authored; their execution is
  unobserved by the current Intel run. Other static-argument applications,
  effects, authority and loans need their own demonstrated parent acceptance.
