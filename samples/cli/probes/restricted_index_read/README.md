# restricted_index_read

A nonconstant checked selector reads a constrained scalar field through a
call: `data Byte requires value >= 0 && value <= 1` + `&main.bytes[position]
.value` element member borrow into `take(&u8)`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
