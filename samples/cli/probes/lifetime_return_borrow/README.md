# lifetime_return_borrow

`combine<'destination>(... &'destination mut Cell) -> &'destination mut Cell` — borrowed-param lifetime forwarded to the result; caller keeps writing through the returned loan.

Compile-checked only: run a check target, not native.
