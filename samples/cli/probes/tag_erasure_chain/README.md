# tag_erasure_chain

Domain-tagged call results through match arms into a
qualified parameter: `relay(match ... -> mark(...))
as i64` lands the match result as `i64 in
Coordinate<7>`, relays it, and `as i64` erases the tag
at the caller's result type.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
