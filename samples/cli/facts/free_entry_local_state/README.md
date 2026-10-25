# Free entry local tally

A free entry owns and cleans an ordinary local tally of 7, and the hosted contract supplies no provisioned ZII context.

Every place copies that sentence. The bound is code 7. The operation is the same: the free entry `start` owns an ordinary local `Tally`, a mark whose code is 7 cleans into that tally, and the entry exits 0 only when the tally reads 7. No place adds an entry parameter, a provisioned context, or a different bound. The entry name is `start`, not a reserved `main`.

Parent outcome: `EXPLICIT-ENTRY-STORAGE` in `TASKS.md`. This row establishes the free-entry local clause of that acceptance: a hosted entry with no parameters owns and cleans ordinary local state, and an unprovided value parameter still rejects. The companion [installed-entry row](../installed_entry_storage/README.md) establishes explicitly lent static backing and declared handoff arguments; the [applied-zero row](../applied_zero_establishment/README.md) and [dynamic-selection controls](../dynamic_trait_dispatch/README.md) cover ordinary initialization validity. Those distinct obligations are not inferred from this tally row.

| Place | Positive program | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | [`invalid/main.omg`](invalid/main.omg) | the code 7 is the entry's own local |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | the code 7 is copied into a local |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | the code 7 is read from a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | the code 7 arrives as `finish`'s parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | a call result returns the code 7 |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield the code 7 |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant is the code 7 |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | a successor machine receives the code 7 |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | another machine supplies the code 7 |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each hosted target body cleans the entry's tally |

## Execution

Every positive place, and the root `main.omg`, binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, and declares `Expected exit: 0`.

```sh
omega run --both samples/cli/facts/free_entry_local_state/main.omg
omega run --both samples/cli/facts/free_entry_local_state/local/main.omg
omega run --both samples/cli/facts/free_entry_local_state/field/main.omg
omega run --both samples/cli/facts/free_entry_local_state/parameter/main.omg
omega run --both samples/cli/facts/free_entry_local_state/call_result/main.omg
omega run --both samples/cli/facts/free_entry_local_state/match_join/main.omg
omega run --both samples/cli/facts/free_entry_local_state/const_name/main.omg
omega run --both samples/cli/facts/free_entry_local_state/other_state/main.omg
omega run --both samples/cli/facts/free_entry_local_state/other_machine/main.omg
omega run --both samples/cli/facts/free_entry_local_state/target_body/main.omg
```

The exit code 0 discriminates the cleanup. The tally starts at 0. `Mark::drop` requires code 7 and writes that code into the tally. The following transition accepts only `tally.total == 7`. A skipped hook leaves 0, and the `bad` state crashes.

### Historical Linux observations

Host: Linux x86_64. Producer: `target/debug/omega`, dev profile, `opt-level` 1, compiler sources at `c2e06b875ba2f6a205dd49b408d937536833b190` (this change adds no Rust). Windows, macOS, and QEMU were not run.

`omega run --both` reported `native exit 0` and `interp exit: 0` for the root and all nine places. Each reject twin exited 1 from `omega --check` with `hosted ProgramEntry exposes no arrival parameters, but start declares unprovided`. The first `other_state` spelling nested `state done` inside `state finish`; the parser rejected it with `expected ;, found identifier done`. The place was rewritten to the sibling-state shape already used by other facts, with the same code 7, the same tally, and the same cleanup. That form then agreed on both engines.

## Reject twins

The nine place positives and their `invalid/main.omg` twins differ only in the entry contract: `machine start()` versus `machine start(unprovided: &mut Unprovided)`. Both contain the same ordinary record declaration. The hosted contract supplies no arrival parameter or backing, so the twin must reject even though an all-zero `Unprovided` would be an ordinary valid local. The root twin retains the separate unprovided `u64` control. The pinned fragment is `hosted ProgramEntry exposes no arrival parameters`.

## Pinned fixtures

The nine pass fixtures under `tests/omega/pass/facts/free_entry_local_state_through_<place>_exit` are byte-identical to the place programs and record native exit 0. The nine reject fixtures under `tests/omega/fail/facts/free_entry_local_state_through_<place>_unprovided` are byte-identical to the place twins.

## Requirements and evidence

- **Owning requirement:** `EXPLICIT-ENTRY-STORAGE` — a free entry owns and cleans ordinary local state, with no provisioned ZII context. Unprovided parameters still reject.
- **Establishment:** `start` has no parameters, so the hosted signature is a free entry. The tally is an ordinary local the entry constructs. The mark's cleanup requirement `mark.code == 7` is established by the transported code, not by a post-hoc guard after cleanup. The transition reads the value the hook wrote.
- **Identity and lifetime:** The tally is the entry's own local storage. The mark borrows that exact local, writes it, and ends before the read. No provisioned context is substituted for the local. A later write of a different tally would be a different fact.
- **Composition:** Calls, joins, fields, constants, states, machines, and target bodies carry the same code 7 into the same cleanup. Each twin changes only the entry signature.
- **Soundness boundaries:** Unprovided mutable-record entry parameters are never given backing by selection. All nine twins reject; the root additionally retains the unprovided `u64` control.
- **Checked-to-Terminal Psi lowering (`Psi 05–07`):** The local, the mark, the cleanup hook, and the transition lower into Terminal Psi. `omega run --both` compares the interpreter with native execution.
- **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** `ProgramEntry` binds `start` on all four hosted targets. The tally and the mark use ordinary value and reference layouts. The free entry has no provisioned receiver lane.
- **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** The tally is an ordinary stack local of the entry. Its cleanup is the entry's normal return cleanup, not a separate entry allocation.
- **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** The hosted image runs the free entry and agrees with Terminal Psi on this machine.

This row establishes ordinary local ownership and cleanup, not every parent obligation. The companion rows above retain their own backing, qualification, partition, and rejection evidence. Current macOS ARM64 observations are recorded below; the earlier Linux observations remain bound to their historical producer. Windows and Linux ARM64 native execution remains unmeasured here.

## Automatic-context retirement baseline

On macOS ARM64 at `b50066b75303e6e899f27764c28e57deb5dbb1ab`,
the public checker wrongly accepted an otherwise ordinary entry with an unprovided
`&mut Context`. The preceding `3ddc7d1455f182d54645df101870ea9b441b9019`
producer also executed that entry with native exit 0 and Terminal exit 0 by
manufacturing backing. Those are refusal-control failures, not successful
acceptance of an installed argument. The frozen probe and qualification receipt
are retained in the outcome claim.

The repair removes the signature scan and source-generated context caller,
interpreter construction, implicit receiver entry shape, and native receiver
construction. Former contextual applications now select authored free wrappers
that own ordinary local state and pass an explicit borrow into the unchanged
application machine. Their normal construction, cleanup and complete WCSU apply.
Explicit installed placed-RAM recipes remain separate: their exact backing,
initial bytes, occurrence identity and loan retirement are still required.

Fresh macOS ARM64 execution at source checkpoint
`9a080b560fe3803f9244108c75a2d74f90c6e552`, using its optimized candidate
producer, reports native exit 0 and Terminal exit 0 for the root and all nine
places. The root scalar twin and all nine mutable-record twins reject with
`hosted ProgramEntry exposes no arrival parameters`: 20 discriminating public
CLI observations. Their raw commands, exits, streams and binary digest are
retained in the outcome's qualification/claim record. The earlier Linux runs
above retain their original producer and source identities; they do not qualify
this repair. The companion installed-entry row records actual macOS ARM64 execution and exact
backing observations at checkpoint `2214d8fa1b3b11b32675f105e61c2d2ce3c55a04`;
this tally row does not stand in for those measurements.
