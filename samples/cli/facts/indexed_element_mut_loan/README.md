# Indexed element mutable loan

A runtime `u64` index below 2 exclusively loans one `Room` to `bump`; the
selected count increases once and its neighbor is unchanged. Every place uses
`bump(&mut rooms[index])` on the same two-element array initialized with counts
0 and 10. `bump` adds 1 in `u32 in Wrapping`. Both indices run: selecting 0
must produce counts (1, 10), and selecting 1 must produce (0, 11).

The original root `main.omg` and `build.omg` remain unchanged. That probe only
checks its `Main::hits` state; it has no authored process-exit oracle. The nine
new applications have an observable oracle: exit 0 after both count checks,
or `Trap` on a wrong result. Their `build.omg` files bind all four hosted
targets. Native execution evidence must name its matching host.

| Place | Project | Transport to the indexed loan |
| --- | --- | --- |
| local | `local/` | a local copy of the runtime selector |
| field | `field/` | the selector is read directly from a record field |
| parameter | `parameter/` | the array and selector are explicit callee parameters |
| call result | `call_result/` | `select_index` returns the selector with an equality postcondition |
| match join | `match_join/` | a join chooses between equal runtime selectors; both arms run |
| const name | `const_name/` | `ROOM_COUNT = 2` supplies the bound on the runtime selector |
| another state | `other_state/` | a successor state receives array and selector and performs the loan |
| another machine | `other_machine/` | a forwarding machine transports array and selector to the loaning machine |
| another target body | `target_body/` | each of four target bodies performs the same indexed loan |

Every `exercise` requires the same strict upper bound, `index < 2` (spelled
`index < ROOM_COUNT` in the const place). `u64` supplies nonnegativity. Neither
this bound nor the result checks manufacture mutable authority. The selected
loan must be exclusive, refer to the original array storage, and end before
the caller observes the counts.

Each project has one `controls/held_loan/` twin. It adds a competing
`&mut Room` loan of the same selected element, uses it after the `bump` call,
and must reject at that call for the still-active `held` borrow. It retains
the same bound, initialization and increment operation. A provider failure,
unproved bound, or another rejection does not satisfy `expected.txt`.

The canonical contracts are
[borrows and aliases](../../../../wiki/spec/language/ownership.md#borrows-and-aliases),
[collection bounds](../../../../wiki/spec/language/numeric_values.md#collection-bounds),
[captured places](../../../../wiki/spec/terminal-psi/loans.md#captured-places-and-available-premises),
and [referent identity](../../../../wiki/spec/terminal-psi/structural_access.md#referent-identity).
They require original storage and independent bounds and loan authority; they
do not permit copying a small referent and pretending that callee-local writes
establish caller-visible mutation.

Run the assigned row with:

```sh
python3 tools/sample_native_gate.py --cli <fixed-compiler> --facts --filter indexed_element_mut_loan
python3 tools/corpus_gate.py --filter indexed_element_mut_loan
python3 tools/corpus_gate.py --native --filter indexed_element_mut_loan
```

The corresponding regression fixtures are
`tests/omega/pass/facts/indexed_element_mut_loan_through_<place>_exit` and
`tests/omega/fail/facts/indexed_element_mut_loan_through_<place>_held`.

On the initial fixed compiler, three places ran and all nine twins rejected;
local, field, call result, match join and const name reached a missing
result-operand storage presentation, and another state reached a missing loan
transfer. The compiler repair retains the call's already evaluated selector
in the result-backed argument path and carries checked loans of affine local
backing across state edges. The repaired row runs all nine applications with
exit 0 on Linux x86-64, and all nine twins retain their intended rejection.
The root remains an unscored probe, separate from that execution evidence.

The additional regression
`tests/omega/pass/borrows/runtime_result_element_call_selector_capture_exit`
changes the selector in a later call argument and checks that the callee
mutates the element selected by the earlier argument. This tests formation
order rather than merely storing an index in an immutable local.
