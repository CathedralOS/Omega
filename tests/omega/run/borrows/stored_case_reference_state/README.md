The stored `Optional<&Cell>` materializes a reference descriptor when its
`Some` payload is copied. Forwarding that value to a borrowed state parameter
must cross the descriptor with `Referent`, not lend the descriptor's own slot.
The call and state argument paths use the same typed endpoint completion.

`omega run --both main.omg` must return zero in both engines after observing 41
through the receiving state and retaining the original owner's value. The
access-widening control is
`tests/omega/fail/borrows/stored_case_reference_state_widening`.

This pins the general state transport repair exposed by Cathedral's borrowed
placed-view snapshot. It does not establish Cathedral mapping activation or
close `ADDRESS-TRANSLATION-CANARY`.
