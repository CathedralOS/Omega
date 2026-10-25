# Descriptor payload exclusive loan

A match on one evaluation of a copyable flag returns the exclusive loan of the
selected case payload's descriptor, writes 41 through that loan, and leaves the
unselected descriptor unchanged.

The root `main.omg` and `build.omg` carry the fact with no transport place: the
match on `choose(&mut visits)` selects the writing arm, `selected` is the
exclusive view of the selected payload's descriptor, `selected[0] = 41` writes
through it, and the successor read observes 41 with `visits == 1`. Root and all
nine places bind `ProgramEntry` for `windows_x86_64`, `linux_x86_64`,
`macos_arm64` and `linux_arm64`, declare `Expected exit: 0`, and pass
`omega run --both` on macOS ARM64 — native exit 0 with the Terminal Psi leg
exiting 0 in agreement.

`f99397375a26b565fb9b493d8cc6f523901e6591` (`psi: record selected descriptors on
match joins`) is the starting behavior below this row: a match that loans the
selected case payload's descriptor exited 0 for a local, a call result, and
a nested machine, but returning the indexed write from a parameter, another
machine, or a target body left that descriptor on the Terminal Psi affine
frontier. Exclusive element views now transfer the selected descriptor owned
onto the join, and the successor reads that carrier, so the exclusive loan
survives all nine transport places on both engines.

Ordinary helper boundaries declare the two active `Slot::Present` cases and
`visits == 0` before they select a loan or call `choose`. Each caller proves
those entry requirements from its own construction; caller facts do not become
undeclared premises of a machine body. The matching reject twins carry the
same requirements and still replace a source while its selected loan is live.

## Places

The nine places carry the same match, the same one-evaluation subject
(`visits` proves it ran once) and the same exclusive descriptor loan through
each language construct:

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | direct local form with the match inline |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | owned local |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | owned record member |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | owned machine parameter |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | checked result guarantee |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | common completed owned-member fact |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | named payload constants |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | state-local owned backing |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | forwarded machine parameters |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | four hosted source bodies |

Twins keep the match, the selected arm and the write through the exclusive
view, then replace the selected source (`chosen`, or `pair.chosen` for
`field`) while the view is still live and before the successor read. Checking
rejects each twin with one diagnostic —
``statement N mutates `chosen` while local borrow `selected` is still active``
(with `pair.chosen` for `field`; `target_body` reports it once per hosted body,
4 diagnostics) — exit 1, never a caller guard. `git diff` of each positive
against its twin shows only that one mutation statement.

The matching corpus pairs are
`tests/omega/pass/facts/descriptor_payload_exclusive_loan_through_<place>_exit`
and
`tests/omega/fail/facts/descriptor_payload_exclusive_loan_through_<place>_rejected`,
with `expected_native_exit.txt` `0` on each positive and the loan-exclusivity
fragment in each twin's `expected.txt`.

## Requirements and Evidence

Owning item: **MATCH-SELECTIVE-LOWERING** in
[TASKS.md](../../../../TASKS.md), whose borrowed-joins
frontier names case/reference/descriptor-bearing referents as still missing
and whose acceptance asks for corpus run fixtures for borrowed joins running
natively with interpreter agreement. This row discharges the descriptor-bearing
referent of an exclusive element-view join through all nine transport places:
the selected case payload's descriptor is loaned exclusively by the match
join, the successor writes and reads 41 through it, and every place runs
natively with Terminal Psi agreement. Canonical authority:
[value dispatch](../../../../wiki/spec/language/patterns.md) and the
[ownership contract](../../../../wiki/spec/terminal-psi/ownership.md).

- **Establishment:** `choose(&mut visits)` runs exactly once (`visits == 1`
  gates the exit), the selected arm yields the exclusive element view of the
  selected payload's descriptor (`&mut chosen.value[..]` / `pair.chosen` /
  the parameter, result, const, state and target-body carriers), and the
  successor read of `selected[0] == 41` is the establishing evidence itself;
  no post-hoc caller equality guard (`transition x == 100`) re-establishes
  the fact after the construct completes.
- **Identity and lifetime:** `selected` names the exclusive loan of the
  selected descriptor for its whole live range; the write is observed at the
  same identity by the successor read and by the transition's `observed`
  value. The twins show the boundary from the other side: replacing the
  source storage (`chosen = Slot::Present { value: [7, 0] }`, `pair.chosen =
  ...`) while the view is live rejects with the diagnostic above, so
  overlapping writes to the borrowed storage are refused while provably
  unrelated access is not.
- **Composition:** the same match, subject count and exclusive descriptor loan
  compose across all nine standard places; each `<place>/main.omg` differs
  from `<place>/invalid/main.omg` only in that one mutation statement of the
  construct's own source, never in a caller guard, and calls substitute the
  exact actuals (`apply(&mut chosen, &mut other, &mut visits)`,
  `present([17, 0])`). The consuming operation reads `selected[0]` directly
  from the fact carried through the place.
- **Soundness boundaries:** the exclusive view is affine; using the source
  while it is live is rejected (9 twins, each with its own diagnostic), the
  join transfers the selected descriptor owned rather than publishing a
  loan of the arm-local view, and `visits == 1` pins once-only effectful
  subject evaluation at both engines. No erased mathematics is substituted
  for the executable element write.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** stage 05 records the
  selected descriptor on the match join: an exclusive element view transfers
  it owned onto the join parameter and the successor reads that carrier,
  while shared views and mutable byte views keep joining under their loan;
  the checked program lowers without loss into canonical target-neutral
  Terminal Psi and executes in the Terminal Psi interpreter with
  `interp exit: 0` against native exit 0 on all nine places (`omega run
  --both`, stdout empty, exit 0).
- **Provider settlement, layout geometry, and ABI calling plans (`Omega
  00–02`):** `ProgramEntry` settles for all four hosted targets in every
  place's `build.omg`; the payload is `[i64; 2]` inside a case payload whose
  element view is addressed at its element width; `CallPlan`/`StatePlan`
  lower the parameter, other-machine, other-state and target-body carriers
  across register and stack lanes.
- **Instruction selection, register homes, and activation frame realization
  (`Omega 03–06`):** the exclusive element store and the successor element
  load select target ISA instructions on `arm64` (host) and have `x86_64`
  bodies checked for `linux_x86_64` and `windows_x86_64`; the chosen/other
  locals and the view holder get disjoint stack-frame homes, and the entry
  prologue/epilogue specialize each hosted body.
- **Form encoding, branch relaxation, relocations, and native image emission
  (`Omega 07–09` + emission):** the native leg emits and executes a direct
  Mach-O image on macOS ARM64 (`native exit 0`, agreeing with Terminal Psi),
  with the frame displacements of the local/field forms resolved and the
  match's short branches relaxed inside the function.

Run evidence on this checkout (base `114906953e11`, macOS ARM64). The places
and twins were measured first with the shared producer at `9ff26a87021` and
re-measured after integrating incoming main with a compiler built from the
base (`49caaa8c2a21`, `--build-profile` `opt_level 1`): all ten positives
(root + nine places) exit 0 with `native exit 0` and `interp exit: 0`, and all
nine twins exit 1 with their pinned fragment.

```sh
omega run --both samples/cli/facts/descriptor_payload_exclusive_loan/local/main.omg   # exit 0, native exit 0, interp exit: 0
omega --check samples/cli/facts/descriptor_payload_exclusive_loan/local/invalid/main.omg   # exit 1, the loan diagnostic
python3 tools/sample_gate.py --filter cli/facts/descriptor_payload_exclusive_loan --cli <omega>          # 19/19 checked, 9 intended rejections matched
python3 tools/sample_native_gate.py --filter cli/facts/descriptor_payload_exclusive_loan --cli <omega>   # 10/10 meet oracle, 9/9 controls match
python3 tools/corpus_gate.py --filter descriptor_payload_exclusive_loan
python3 tools/corpus_gate.py --native --filter descriptor_payload_exclusive_loan
python3 tools/corpus_gate.py --both --filter descriptor_payload_exclusive_loan
```

Corpus rows recorded at `114906953e11+wt` (row additions only, 0 golden
deletions): the check leg records nine `checked` pass rows with
`check-target:macos_arm64` and nine `rejected` twins carrying their pinned
loan fragment; the native leg records eight `built exit:0` rows; the both leg
records eight `built exit:0` rows with `interpreter-exit:0`,
`interpreter-stdout:match` and `interpreter:agrees`.

Known red: `pass/facts/descriptor_payload_exclusive_loan_through_other_state_exit`
is observed `rejected` in the corpus checked-compile route while the same
source builds and runs through `omega run --both` (and through plain `omega`
for all four declared targets). Reproduce with
`OMEGA_CORPUS_NATIVE=1 OMEGA_CORPUS_FIXTURE_FILTER=descriptor_payload_exclusive_loan_through_other_state_exit OMEGA_COMPILER_CHECK_SITES=1 <corpus_runner>`;
the record is

```
target legalization failed: SourceCustodyMismatch { machine: Some(MachineId(1)), operation: Some(OperationId(12)), site: omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/legalization/scalar_graph_input/reference_custody.rs:1283 }
```

The refusal is at the transition call into the successor machine whose body
selects an element view of a borrowed parameter's case payload; the same
call with a plain write, and the same match-view inside a called machine
(`other_machine`, which returns), both accept. The successor machine's owned
descriptor leaf therefore still reads as a suspended child of the caller's
carrier argument at target legalization. The fixture and rows stay pinned so
the repair flips them visibly.

## What remains open

Nearest: the checked-compile route's refusal for the `other_state` successor
state (above) — the one fixture this row cannot yet record as a native
success; the sample loop's next required repair for this row is the
custody-identity disagreement between a successor machine's owned descriptor
leaf and the caller's carrier argument at the transition call.

Under MATCH-SELECTIVE-LOWERING, open after this row: write-only joins
(**WRITE-ONLY-BORROW**), runtime-sized element-view selections, ranges,
computed roots, and join-place reads/projections; borrowing joins that end at
last use rather than pinning origins through the whole dominated region
(`optimization-unit/semantics/src/current_ownership/address_joins.rs`); and
combined Psi rewrite provenance for `branch_edges`. The acceptance's other
fixture classes — owned call-product arms, interleaved and projected sources,
linear child carriers, effect/skipped-arm/overlap coverage and the float match
customers — stay with the owning item, as do Windows/Linux native rows, which
record on their own hosts' goldens. This row claims no foreign native
execution; `linux_arm64`/`linux_x86_64`/`windows_x86_64` bindings are source
checked and realized, not executed here.