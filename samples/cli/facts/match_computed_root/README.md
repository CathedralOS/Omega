# Match computed root

A match subject is one evaluation of a computed root, the selected exclusive
loan records that root, and the consumer writes 41 through that recorded loan.

`compute(&mut visits)` runs exactly once (`visits == 1` after), yielding the
root index `0`. The match on `chosen[0] == 17` selects `&mut chosen[0]`, the
loan stays live through `selected = 41`, and the source's unselected element
and the sibling storage stay unchanged.

The reject twin mutates the source `chosen` while `selected` is live and
rejects with `while local borrow `selected` is still active`.

## Places

The same sentence is copied through `local`, `field`, `parameter`,
`call_result`, `const_name`, `match_join`, `other_state`, `other_machine`,
and `target_body`.
