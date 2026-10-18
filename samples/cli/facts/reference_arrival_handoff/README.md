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

| Place | Positive | Borrow-active reject twin | Receiving binding |
| --- | --- | --- | --- |
| local | [program](local/main.omg) | [control](local/controls/held_source/main.omg) | a second local |
| field | [program](field/main.omg) | [control](field/controls/held_source/main.omg) | a record field carrying the loan |
| parameter | [program](parameter/main.omg) | [control](parameter/controls/held_source/main.omg) | a callee parameter |
| call result | [program](call_result/main.omg) | [control](call_result/controls/held_source/main.omg) | a result binding returned by `hand` |
| match join | [program](match_join/main.omg) | [control](match_join/controls/held_source/main.omg) | a `match` join binding |
| const name | [program](const_name/main.omg) | [control](const_name/controls/held_source/main.omg) | a binding written with a named constant |
| another state | [program](other_state/main.omg) | [control](other_state/controls/held_source/main.omg) | a state parameter |
| another machine | [program](other_machine/main.omg) | [control](other_machine/controls/held_source/main.omg) | a parameter forwarded to `write` |
| another target body | [program](target_body/main.omg) | [control](target_body/controls/held_source/main.omg) | a parameter of each target’s `write` |

Each twin keeps the source binding or backing writable while the receiving
loan is live; it must reject at the pinned borrow-active obligation.

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
