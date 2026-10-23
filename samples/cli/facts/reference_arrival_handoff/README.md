# Reference arrival handoff

An exclusive reference moves into another binding. Writing through the
receiving binding changes the original backing; the following read must see
41. Expected exit: 0.

The compiler represents the handoff as an owned block arrival, then lends that
arrival's referent to the store. Its home belongs to the destination block:
lowering must not require the predecessor's operation identity to borrow it.

Run `omega run --both samples/cli/facts/reference_arrival_handoff/main.omg`.

The original `main.omg` remains the starting witness. Every place copies the
sentence above and keeps the same operation; the binding that receives the
moved reference is the place:

| Place | Positive | Reject Twin (`invalid`) | Control (`held_source`) | Receiving binding |
| --- | --- | --- | --- | --- |
| local | [program](local/main.omg) | [invalid](local/invalid/main.omg) | [control](local/controls/held_source/main.omg) | a second local |
| field | [program](field/main.omg) | [invalid](field/invalid/main.omg) | [control](field/controls/held_source/main.omg) | a record field carrying the loan |
| parameter | [program](parameter/main.omg) | [invalid](parameter/invalid/main.omg) | [control](parameter/controls/held_source/main.omg) | a callee parameter |
| call result | [program](call_result/main.omg) | [invalid](call_result/invalid/main.omg) | [control](call_result/controls/held_source/main.omg) | a result binding returned by `hand` |
| match join | [program](match_join/main.omg) | [invalid](match_join/invalid/main.omg) | [control](match_join/controls/held_source/main.omg) | a `match` join binding |
| const name | [program](const_name/main.omg) | [invalid](const_name/invalid/main.omg) | [control](const_name/controls/held_source/main.omg) | a binding written with a named constant |
| another state | [program](other_state/main.omg) | [invalid](other_state/invalid/main.omg) | [control](other_state/controls/held_source/main.omg) | a state parameter |
| another machine | [program](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) | [control](other_machine/controls/held_source/main.omg) | a parameter forwarded to `write` |
| another target body | [program](target_body/main.omg) | [invalid](target_body/invalid/main.omg) | [control](target_body/controls/held_source/main.omg) | a parameter of each target’s `write` |

Each twin keeps the source binding or backing writable while the receiving
loan is live; it must reject at the pinned borrow-active obligation.

## Pinned Fixtures

The row is pinned by `tests/omega/{pass,fail}/facts/reference_arrival_handoff_through_*`.
Each of the nine positive places has a
`tests/omega/pass/facts/reference_arrival_handoff_through_<place>_exit` pin — the
place program with its `build.omg` and authored native exit 0 — and each reject
twin is pinned at
`tests/omega/fail/facts/reference_arrival_handoff_through_<place>_rejected` with
that twin's borrow-active fragment in `expected.txt`. The `cleanup_receiver`
variations keep their own pinned rows beside them.

This row covers **MATCH-SELECTIVE-LOWERING** in [TASKS.md](../../../../TASKS.md):
transport the exact exclusive loan through an arrival, then write through that
arrival to its original backing. The contracts are
[reference carriers and custody](../../../../wiki/spec/language/ownership.md),
[selected compatible arm values](../../../../wiki/spec/language/patterns.md), and
[edge-local transfer and cleanup](../../../../wiki/spec/terminal-psi/ownership.md).
The match place selects existing `original`/`alternate` descriptor occurrences.
Source availability, each descriptor's own lifetime, and access to the borrowed
referent remain separate obligations. Shared source observations remain copyable.

The [cleanup receiver rung](match_join/cleanup_receiver/main.omg) composes that
same 41 writeback with **CML4**: the selected exclusive carrier enters an intact
nominal `Keeper` receiver, its hook proves `keeper.code == 41`, and normal cleanup
writes 41 into the selected original backing. Both choices execute, and the
unselected backing must remain zero. Its
[prerequisite twin](match_join/cleanup_receiver/controls/prerequisite/main.omg)
changes only the captured code to 40 and must reject the unchanged hook clause.
Run `omega run --both samples/cli/facts/reference_arrival_handoff/match_join/cleanup_receiver/main.omg`.

The [ordered receiver variation](match_join/cleanup_receiver/order/main.omg)
interleaves an ordinary marker owner between the two candidate carriers. Cleanup
must print `ABM` when the first carrier is selected and `BMA` when the second is
selected, while the caller still observes the same 41/other-zero writeback after
the helper returns. Its [prerequisite twin](match_join/cleanup_receiver/order/controls/prerequisite/main.omg)
keeps the hook's 41 demand and captures 40. The
[held-residual control](match_join/cleanup_receiver/order/controls/held_residual/main.omg)
tries to read the backing before the remaining nominal receiver dies; its
captured loan must still reject that read. The Console provider in this variation
explicitly assumes finite completion for its one-byte writes; it does not
strengthen the standard Console contract.

The [rebound source variation](match_join/cleanup_receiver/order/rebound_source/main.omg)
re-establishes `original` after selecting its old value. Cleanup retains the old
unselected occurrence while the replacement keeps the named local's declaration
position: `ABMC` then `BMCA`, with the same 41/other-zero writeback. Its
[prerequisite twin](match_join/cleanup_receiver/order/rebound_source/controls/prerequisite/main.omg)
changes only the captured Keeper code to 40.

The [projected sibling variation](match_join/cleanup_receiver/order/projected_sibling/main.omg)
selects `Pair.first` into the same Keeper while each Pair retains its Marker
sibling. The selected Carrier dies after the observation; remaining Pair
children die at their original declaration positions when the helper returns.
The two choices print `AYBMX` and `BYMXA`, then the caller observes the same
41/other-zero writeback. Its [code40 twin](match_join/cleanup_receiver/order/projected_sibling/controls/prerequisite/main.omg)
still rejects Keeper's unchanged prerequisite; its
[held-residual twin](match_join/cleanup_receiver/order/projected_sibling/controls/held_residual/main.omg)
keeps the original and alternate loan custody live until their normal deaths.
The compiler uses checked contextual subtree actions for those deaths, rather
than disposing sibling geometry at the selection join or treating a partial
Pair as an intact nominal receiver.

The [projected state-jump variation](match_join/cleanup_receiver/order/projected_sibling/state_jump/main.omg)
captures that selected Carrier as the explicit argument of `finish`. The entry
state closes its Pair residuals and Marker before the successor receives the
Carrier: `YBMX` then `A` for the first choice, and `YMXA` then `B` for the second.
The successor's Keeper still writes 41 through the exact selected loan; the
caller observes 41/other-zero after the helper returns. Its
[code40 twin](match_join/cleanup_receiver/order/projected_sibling/state_jump/controls/prerequisite/main.omg)
rejects Keeper's unchanged requirement in `finish`. The
[early-read twin](match_join/cleanup_receiver/order/projected_sibling/state_jump/controls/held_residual/main.omg)
reads the original Counter backing before the jump while the local candidate
and selected loans remain active. This pins the projected-argument normal-edge
witness required by CML4, without claiming its broader cleanup acceptance.

The [owned-parameter replacement control](../../../../tests/omega/pass/facts/reference_arrival_handoff_cleanup_receiver_parameter_rebound_exit/main.omg)
keeps the same 41 writeback and `ABMCBMCA` order when another parameter supplies
the replacement. Its [code40 twin](../../../../tests/omega/fail/facts/reference_arrival_handoff_cleanup_receiver_parameter_rebound_prerequisite/main.omg)
still rejects the captured cleanup prerequisite. The
[nested construction control](../../../../tests/omega/pass/drops/runtime_selected_transfer_nested_construction_exit/main.omg)
keeps completed nested values in declaration order while the existing incomplete
construction control keeps abandoned prefixes in establishment order.

These programs establish the exercised loan identity, selected writeback and
normal receiver cleanup. They do not establish arbitrary projected or runtime-sized
selection, write-only joins, every hook-order interaction, foreign-host execution,
or closure of either parent task. Those requirements remain on the board.

## Another state passes a reference to a state

`other_state/` hands a reference to a named target whose parameter is a loan.
The two reference accesses share one route, and both must work: `&mut i64` and
`&i64`. An owned value, a machine parameter, and a second local holding the
reference follow different routes and were never affected.

A plain local borrowed at the transition exposed a lowering refusal, not a
checking one. `retained_backing` found the borrowed root symbol, but
`local_storage::source` has no structural occurrence to report for a scalar
symbol, and the `?` on that lookup turned "caller-owned storage" into "missing
provenance", so the state had no executable body:

    `main` has no executable body: this compiler does not lower a transition
    of it yet (state loan: captured backing); in state `main::entry`, statement 2

The descriptor case the guard protects is already refused above the symbol
route: `stored_loan_parts` is `Some` exactly when the root type is a stored
reference carrier, and that branch returns first. A root that reaches the
symbol route is neither a descriptor nor a state parameter, so an absent
occurrence can only mean the local was declared before the operation prefix —
the same caller-owned conclusion the producer search below draws when it finds
no producer for its root. `tests/omega/pass/facts/`<br>
`reference_arrival_handoff_through_other_state_exit` pins this: it records
`incomplete` against a compiler without the repair and `built exit:0` with it.

## Requirements and Evidence

- **Parent outcome:** `MATCH-SELECTIVE-LOWERING` (`TASKS.md`).
- **Requirement established:** End-to-end nine-place fact preservation, loan exclusivity, arrival handoff lowering, and native realization for moved exclusive references (`&mut i64`). Specifically:
  1. **Establishment:** Forming an exclusive reference `&mut value` (or `&mut backing`, initialized to `17`) and moving that exclusive reference carrier into the receiving binding establishes exclusive write authority to the original backing. Writing `child = 41` (or writing through the receiving binding across each place) mutates the original backing to `41`, without requiring any post-hoc caller equality guard (`transition x == 100`) after the construct completes. Reading through the active loan (or inspecting the mutated backing after loan retirement) proves `observed == 41` directly from the static fact established by the store.
  2. **Identity and lifetime:** The moved exclusive reference names the exact backing storage version of `backing`. Across arrival handoff (and across `cleanup_receiver` variations: ordered markers, rebound source, projected siblings, and state jumps), the loan retains exact backing identity. The move invalidates read and write access through the moved-from source binding until the receiving loan expires; overlapping writes to unrelated storage do not invalidate the carried reference, but concurrent access to `backing` or `original` while the receiving loan is active is statically prohibited.
  3. **Composition:** The exclusive reference handoff composes uniformly across all nine standard places:
     - `local`: a second local binding receives the moved reference.
     - `field`: an aggregate record field (`Holder.child`) carries the loan across construction and projection.
     - `parameter`: an explicit machine parameter receives the moved loan.
     - `call_result`: helper function `hand` returns the exclusive reference carrier to the caller's receiving binding.
     - `match_join`: a match expression unifies the loan across distinct control flow arms.
     - `const_name`: a binding written with a named constant guides the handoff.
     - `other_state`: a successor state parameter captures the moved reference across transition edges.
     - `other_machine`: an intermediate machine forwards the reference to an external writing machine.
     - `target_body`: target-scoped machine bodies execute the reference handoff across each hosted platform.
     In all nine places and cleanup-receiver variations, `<place>/main.omg` vs. `<place>/invalid/main.omg` differ only in the conflicting use of the source binding or backing while the receiving loan is active—never in any caller guard.
  4. **Soundness boundaries:** Loan exclusivity, carrier unavailability after move, and cleanup preconditions are strictly enforced at compile time. The nine `invalid/` twins (and their identical mirrors under `controls/held_source/`) attempt to mutate the moved-from source binding (`original`) or backing while the receiving loan (`child` or `held`) remains active, rejecting with `mutates `original` while local borrow `child` is still active` (or `state ... receives `original` while local borrow `held` is still active`). Furthermore, the cleanup-receiver `prerequisite` controls reject with `cannot prove automatic cleanup requires Keeper::drop clause `keeper.code == 41`` when an invalid code is supplied, and `held_residual` controls reject unauthorized reads of the backing while carrier custody remains live.
  5. **Checked-to-Terminal Psi lowering (`Psi 05–07`):** Owned block arrivals and state-parameter loans lower into canonical target-neutral Terminal Psi (`PSITERM\0`) without loss. The compiler represents reference handoff as an owned block arrival whose storage home belongs to the destination block, ensuring lowering does not require predecessor operation identity at the destination store. Lowering materializes explicit affine cleanup actions (`DiscardRoot`, `DiscardResidual`, `InvokeNominal`, `RetireHome`) and borrow restoration windows, handles state transitions and ranking edges, and executes identically in the Terminal Psi interpreter (`omega run --both`).
  6. **Provider settlement, layout geometry, and ABI calling plans (`Omega 00–02`):** Build companion rows bind `ProgramEntry` across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`). Target layout plans evaluate scalar width/alignment geometry, pointer sizing (64-bit address carriers), and structural records (`Holder`, `Pair`, `Keeper`). ABI calling plans (`CallPlan`, `StatePlan`) correctly partition reference pointer carriers across register argument slots (RCX on `windows_x86_64`, RDI on `linux_x86_64`, X0 on `macos_arm64`/`linux_arm64`) and spill slots.
  7. **Instruction selection, register homes, and activation frame realization (`Omega 03–06`):** Backend instruction selection emits indirect pointer stores (`*target = 41`) for reference assignments, followed by scalar loads for verification. Activation storage assigns disjoint stack-frame homes for the backing local variable (`backing` / `value`) and live-across-call reference carriers, ensuring reference displacement and caller-owned stack slots do not clobber one another across calls and state transitions. Standard function prologues and multi-return restoration epilogues strictly honor the computed frame bounds.
  8. **Form encoding, branch relaxation, relocations, and native image emission (`Omega 07–09` + emission):** Symbolic stack-frame displacements for the borrowed local backing resolve into concrete machine-code offsets. Function-relative basic blocks, state transitions, and match edges lay out with branch relaxation and offset reflow. Transitive machine-state footprint evidence is validated, and direct PE, ELF, and Mach-O executable images are emitted. Native binaries execute and agree completely with Terminal Psi (`omega run --both` exit 0) across host targets.

- **What remains open:** Arbitrary multi-level projected or runtime-sized loan selection, write-only join unifications, unconstrained hook-order reordering across divergent cleanup paths, and foreign non-hosted targets.

