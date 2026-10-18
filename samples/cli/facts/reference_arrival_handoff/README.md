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

| Place | Project | Receiving binding |
| --- | --- | --- |
| local | `local/` | a second local |
| field | `field/` | a record field carrying the loan |
| parameter | `parameter/` | a callee parameter |
| call result | `call_result/` | a result binding returned by `hand` |
| match join | `match_join/` | a `match` join binding |
| const name | `const_name/` | a binding written with a named constant |
| another state | `other_state/` | a state parameter |
| another machine | `other_machine/` | a parameter forwarded to `write` |
| another target body | `target_body/` | a parameter of each target's `write` |

Each place has a `controls/held_source` twin that keeps the source binding or
backing writable while the receiving loan is live; each must reject with the
pinned borrow-active diagnostic.

At `match_join` the joined binding receives a loan produced on each arm, the
same convention `returned_write_destination` uses for reference cargo. A match
arm that moves an *existing* exclusive binding (`true -> original`) is a
separate rejected capability today: owned-selection receipts cover owned data
leaves and exclude exclusive-reference roots, so checking records no receipt
and lowering stops at `owned selection place is absent from its receipt
sources`.

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
