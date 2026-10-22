# Body-bearing match equality

Equal `f64 in Marked` carriers. The selected `==` body returns false, so the
direct comparison is false and the match takes the wildcard. Builtin equality
would do the opposite.

Owning task: **OPERATOR-MACHINE-SUPPLY** (`TASKS.md`). The exact requirement this
row establishes is that item's `call_binding.rs` bullet: a body-bearing `==`
spelling — binary and match equality — binds to the declared operator's own body
as an ordinary call, the selected body's boolean result is recorded on the
comparison, and a direct `== false` obligation reads that result. This folder is
the std-free witness for a body-bearing spelled comparison. A bodyless floating
comparison of `f64` stays on the comparison route in `float_comparison_route`.
This row does not close the rest of OPERATOR-MACHINE-SUPPLY.

Every place copies that sentence and the same operation: the two carriers are
equal `f64 in Marked` values (`1.0`, `1.0`), they are compared with the
body-bearing `==`, and the same two carriers are matched
(`match value { pattern -> 7, _ -> 11 }`). The program reaches `done()` (exit 0)
only when the comparison is false and the match arm misses with 11; a true
comparison or a hit reaches `state bad() { crash Trap; }`. `hold` carries
`requires direct == false`, so each place proves that call from the comparison's
recorded boolean result — not from a post-hoc caller equality guard. Only the
transport of the two carriers changes.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical root witness; the comparison is the `compare` helper and the branch reads its result |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the entry machine copies the marked carrier into a second local and compares the two locals |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | both carriers arrive as record fields |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | both carriers arrive as explicit machine parameters |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a call result returns each carrier |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield a carrier |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | named constants are the carriers |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor state receives both carriers |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a forwarding machine supplies both carriers |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | one body per hosted target uses the carriers |

Every positive place (and root `main.omg`) declares `Expected exit: 0` and binds
`ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and
`linux_arm64` with no `Build::depend` line. The eight pre-existing twins change
exactly one thing: the selected body returns `true` instead of `false`, so the
same comparison is true and `direct == false` is unprovable. The new
[`local`](local/invalid/main.omg) twin keeps the body and does the opposite
instead: it weakens the copied operands to their `f64` encoding
(`(copy as f64) == (copy as f64)`), so the same equal carriers take builtin
equality — true — and the same obligation is unprovable. That is the fact's own
negative: the row exists because the selected body routes the comparison rather
than the builtin.

## The local place carries its own mechanism

The root keeps both carriers in entry-machine locals but compares them inside
`compare`, so the route is selected by a callee's parameter list. No other place
compares at a local site. `local/main.omg` is the ninth place and is not a count
variant of `field` or `parameter`:

- The right carrier is a **copy of the left in a second local**
  (`let copy: f64 in Marked = value;`), and the comparison site is those locals
  (`let direct: bool = value == copy;`). The domain qualification that selects
  the body comes from the local declarations, and the copy has to preserve it.
- Measured on the retained Terminal Psi of `local/main.omg`: **0**
  `IeeeFloatCompare` operations. The entry machine's comparison site is
  `operation kind=CallStructuralScalar { callee: MachineId(4) }` whose two
  argument declarations carry `qualifications: ScalarQualificationSetId(1)`
  (the `Marked` set), and machine 4 is the selected `== f64::Marked::same` body
  returning `BooleanConstant { value: false }`. The match machine lowers its arm
  test to a `CallStructuralScalar` on the same body and yields 7 or 11.
- Control (measured): the same copied local with the selected body flipped to
  `true` rejects `cannot prove requires contract for call hold from main:
  direct == false`. The recorded result reads the body.
- Control (measured): weakening the copy implicitly (`let copy: f64 = value;`)
  is refused outright — `implicit domain weakening in machine main state entry
  local copy drops Marked` — so the local transport cannot silently decay to
  `f64` and builtin equality.
- Control (measured): the twin's explicit weakening produces **1**
  `IeeeFloatCompare` and no call on the selected body, and runs to `native
  signal 4` / `interp crash: Trap`. Builtin equality is true for these equal
  carriers; the body-bearing route is what makes the positive exit 0.

Copying the marked carrier therefore preserves the qualification, and the
compiler already fails closed where a local copy would lose it. No place in this
row refused, so this row needed **no compiler repair** and changed nothing in
`omega-rust/`.

## Execution

Measured at base `2ece6cb7889b8ca67e43f2317d9d14c062f928e5` on an **Intel macOS
x86_64** host, with an opt-level 1 producer built from that tree
(`python3 tools/build_slot.py -- mbx build -p omega`;
`omega --build-profile` reports `opt_level: 1`, `x86_64-apple-darwin`). The four
authored hosted targets cannot execute on this host, so each scratch copy added
`macos_x86_64::ProgramEntry` and `target_body` also got an identical
`macos_x86_64 machine observe()`; no fact source carries a fifth binding, because
that is host pinning, not a language gap.

```sh
omega run --both samples/cli/facts/body_bearing_comparison/<place>/main.omg
omega --check samples/cli/facts/body_bearing_comparison/<place>/invalid/main.omg
python3 tools/sample_native_gate.py --facts --filter cli/facts/body_bearing_comparison
python3 tools/corpus_gate.py --filter facts/body_bearing_comparison
```

- All nine positives in the scratch copy: `native exit 0` and `interp exit: 0`.
- All nine twins: `omega --check` exits 1 with `cannot prove requires contract
  for call hold from main: direct == false` (the parameter, other_machine,
  other_state and target_body twins report it from `observe`). The corpus runner
  records all nine as `rejected` with **0** rejected without their expected
  fragment.
- All nine retained artifacts carry **0** `IeeeFloatCompare` operations, so the
  comparison never lowers to the float builtin route at any place.
- The bare public command on this host stops at host pinning: `selected target
  macos_x86_64 has no bound required root slot macos_x86_64::ProgramEntry`.
  `omega run --target macos_arm64` is compile-publication only
  (`ExecutionOutcome::TargetOnly`); it executes nothing and is not runtime
  evidence. Native execution on `windows_x86_64`, `linux_x86_64`,
  `macos_arm64`, and `linux_arm64` is **not measured here** and remains owed to
  a host that can execute one of them.

Main advanced twice while this row was being finished, including Psi changes to
recorded Boolean results, match joins, and constructor memberships. The row was
rebased onto `3b7e1df0ff1` and then `1654d389500`, with a producer rebuilt from
each rebased tree (opt-level 1). At the final base: all nine positives report
`native exit 0` with `interp exit: 0`, all nine twins still reject with both
authored fragments, every retained artifact still carries 0 `IeeeFloatCompare`,
the corpus gate reports 0 structural moves over the 18 fixtures (18 perf-only
moves at a measured load of 64.57 on 16 CPUs, which the gate flags as noise), and
the `field` and `other_state` artifact-ingress refusals above persist unchanged.
At `3b7e1df0ff1` the sample gate reported 10 of 10 intended-invalid controls
matching with 11 projects not-host-runnable.

## Reject twins

| Place | Twin | Change | Pinned diagnostic fragment |
| --- | --- | --- | --- |
| local | [`local/invalid/main.omg`](local/invalid/main.omg) | comparison operands explicitly weakened with `as f64`, so builtin equality is used | `cannot prove requires contract for call hold`, `direct == false` |
| field | [`field/invalid/main.omg`](field/invalid/main.omg) | selected body returns `true` | `cannot prove requires contract for call hold`, `direct == false` |
| parameter | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | selected body returns `true` | `cannot prove requires contract for call hold`, `direct == false` |
| call result | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | selected body returns `true` | `cannot prove requires contract for call hold`, `direct == false` |
| match join | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | selected body returns `true` | `cannot prove requires contract for call hold`, `direct == false` |
| const name | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | selected body returns `true` | `cannot prove requires contract for call hold`, `direct == false` |
| another state | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | selected body returns `true` | `cannot prove requires contract for call hold`, `direct == false` |
| another machine | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | selected body returns `true` | `cannot prove requires contract for call hold`, `direct == false` |
| another target body | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | selected body returns `true` in every target body | `cannot prove requires contract for call hold`, `direct == false` |

Each twin breaks only its own fact or obligation: the eight body flips change the
declared body's value and nothing else, and the local twin changes only the
comparison's route/domain while keeping the body, the carriers, the match, and
the obligation. Every twin still rejects with the row's own obligation, so a
missing or unrelated refusal is not standing in for the fact.

## Pinned fixtures

`tests/omega/pass/facts/body_bearing_comparison_through_<place>_exit` (nine
positives, byte-identical copies of their place `main.omg`, each with a
`build.omg` that binds the four hosted targets and an
`expected_native_exit.txt` of 0) and
`tests/omega/fail/facts/body_bearing_comparison_through_<place>_rejected` (nine
twins, byte-identical copies of their twin `main.omg` with an `expected.txt`
carrying both fragments). Their eighteen golden rows were recorded by filtered
splice at `2ece6cb7889b`: nine `checked` with `check-target:macos_arm64` and nine
`rejected`, +30/-0 lines in `corpus_outcomes.txt` with no other row moved and no
fixture key or diagnostic continuation line lost. The row was previously at 8/9
places with **no pinned fixtures at all**; all nine places and all nine twins are
now pinned.

Eight of the nine positive places keep their original program bytes. The ninth,
`local`, is authored here.

## The returned-boolean rung

The comparison records the selected body's boolean result; the match and the
direct use read that result, and a domain on an `f64` constant keeps the `f64`
encoding. The [`returned_boolean`](returned_boolean/main.omg) rung keeps a
result constant through agreeing literal returns, a named state and a terminal
call; a separate conditional result still executes both true and false. Its
[`twin`](returned_boolean/invalid/main.omg) changes only the named state's
returned value. Body-result equations join every reachable normal return and
selected target body. An unknown or disagreeing return supplies no constant;
an earlier expression statement is not the invocation's final result. The rung's
positive is pinned byte-identically as
[`constant_boolean_return_join_exit`](../../../../tests/omega/pass/proofs/constant_boolean_return_join_exit/main.omg).

On `55898d082fc8`, an unrelated conditional body returning true or false wrongly
checks as always true, and a mixed named-state body wrongly checks as always
false. The agreeing named-state/call rung instead refuses its valid `demand`
contract. Those frozen baseline observations and the actual host execution
records are retained in the owning qualification/claim evidence. The focused
fixtures under `tests/omega/{pass,fail}/proofs/` pin the shared return join;
they do not close the rest of **OPERATOR-MACHINE-SUPPLY**. The rung and the 18
place fixtures above are separate evidence: the rung isolates the return join,
the nine places carry the fact through its transports.

## Requirements and evidence

1. **Establishment:** the comparison itself establishes the observed truth at
   every place: the selected `== f64::Marked::same` body returns `false`, and its
   result is recorded on the comparison. Nothing re-establishes the result in the
   caller: `hold(direct)` is admitted from that recorded result. In the local
   place the marked qualification itself is established by the local declaration
   and preserved by the copy; the compiler refuses an implicit weakening instead
   of admitting an unqualified carrier.
2. **Identity and lifetime:** the carriers are exact values (`1.0`, `1.0`) with
   an exact domain qualification. The local copy preserves both, and the
   comparison, the match, and the obligation all read the same carrier version.
   No place writes a compared carrier between the comparison and its use, so
   overlapping-write invalidation is **not exercised** by this fact; that
   obligation belongs to the owning task's other witnesses.
3. **Composition:** the comparison result composes into the consuming obligation
   `hold(direct)` (`requires direct == false`) at all nine places, and the match
   join composes into `matched == 11`. `<place>/main.omg` differs from
   `<place>/invalid/main.omg` only in the construct's own value or domain: eight
   twins move the declared body's literal from `false` to `true`; the local twin
   moves the comparison operands' domain with an explicit `as f64`. No twin adds
   a caller guard, and no place re-proves the result outside the construct.
4. **Soundness boundaries:** all nine twins reject for the row's own obligation.
   The builtin-route control shows the contrast at runtime: with builtin equality
   the same equal carriers compare true and the program takes its `crash Trap`
   route (`native signal 4`, `interp crash: Trap`, host presentation agrees). The
   implicit-weakening control fails closed at checking. The selected route cannot
   silently become the builtin one without an explicit, authored domain change.
5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** the body-bearing `==`
   lowers without loss to canonical target-neutral Terminal Psi (`PSITERM\0`) as
   a `CallStructuralScalar` on the selected body, and no place's artifact
   contains an `IeeeFloatCompare` — the comparison is never folded onto the float
   builtin route. Proof-only obligations are erased and the state transitions
   lower. The Terminal Psi interpreter executes all nine positives to
   `interp exit: 0` and agrees with native execution. **Open:** the retained
   artifacts of two places, `field` and `other_state`, are refused by the codec's
   own structural-foundation ingress validation with
   `scalar field write lacks its required domain identity`; see
   [what remains open](#what-remains-open).
6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):**
   every positive and twin binds `ProgramEntry` for the four hosted targets; the
   corpus check leg realizes `macos_arm64` from those bindings and records
   `checked`, and the executed scratch images realize `macos_x86_64`. `f64`
   layout is 8 bytes wide and 8-byte aligned, and the selected body is called
   with two scalar arguments in every place. Per-operand ABI placement records
   were not separately captured in this row.
7. **Instruction selection, register homes, and activation frame realization
   (`Omega 03–06`):** the executed images select the call/return path for the
   selected body, assign register homes and disjoint frame slots for the entry
   machine and its helpers, and route the true arm to the trap. This host
   executed only `macos_x86_64`; the four authored hosted targets are checked
   through their entry bindings but not executed here.
8. **Form encoding, branch relaxation, relocations, and native image emission
   (`Omega 07–09` + emission):** the emitted `macos_x86_64` images execute and
   agree with Terminal Psi (nine of nine `native exit 0` / `interp exit: 0`), and
   the twins reject before emission. Native emission for the four authored
   hosted targets remains owed to their hosts.

## What remains open

- **Native execution on the four authored hosted targets.** This host pins
  `macos_x86_64`, and `omega run --target` is compile-publication only. The row's
  runtime evidence here is the scratch `macos_x86_64` execution plus Terminal Psi
  agreement.
- **Two retained artifacts are not re-ingestible.** `field/main.omg` and
  `other_state/main.omg` execute correctly on both engines, but their retained
  `program.oterm` is refused by the codec's foundation validation at ingress:
  `cannot inspect Terminal artifact:
  Semantic(MalformedStructuralFoundation("scalar field write lacks its required
  domain identity"))`. Minimal reproduction at `2ece6cb7889b`: a record with a
  domain-qualified scalar field constructed from a literal
  (`data Carriers [copy] { left: f64 in Marked; }` with
  `Carriers { left: 1.0 }`) produces the same refusal, so the loss is in the
  emitted `EstablishRecord` scalar member (and in the state-transition argument
  store), not in this row's transport. The in-memory compile-to-run path does not
  re-ingest the artifact, which is why both engines still agree; a byte-level
  consumer (`lower_artifact`, `TerminalProgram::load`) refuses it. This is a
  separate `psi` lowering repair — carry the receiving field's declared
  qualification into the constructed scalar declaration — and it needs its own
  witness and independent semantic review; it is not repaired here.
- **The rest of OPERATOR-MACHINE-SUPPLY is untouched:** the call-containing
  scalar requirement (`requires observe(left) == observe(right)`), the formation
  checks for static callable/evidence arguments and generic dispatch, and the
  `OpenIndexOperationSelection` frontier remain as the board states them.
