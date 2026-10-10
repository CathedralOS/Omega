# operand_order_linear

First sample proving statement-level operand order with a
linear move: `holder.token = transform(holder.token.value,
holder.token)` reads the scalar BEFORE the member is
extracted, even inside one assignment.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
