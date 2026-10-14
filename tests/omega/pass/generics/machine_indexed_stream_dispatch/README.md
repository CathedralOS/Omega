# machine_indexed_stream_dispatch

A machine body constructs, forwards and dispatches over a recursive family
indexed by a static machine:

- `Index` is a recursive enum (`Next` carries an `Index` payload).
- `Stream<machine S>` binds a static machine index under
  `where machine S(index: Index) -> Index`; `Stream::More` stores
  `tail: Stream<S>`.
- `forward_schema<identity_schema, succ>` passes one machine through another
  machine's nested machine parameter; specialization makes the calls direct.
- The body builds a two-element `Stream<succ>`, forwards it, and routes the
  result through two states that each `transition` on the forwarded value.

`pass/generics/higher_order_schema` pins the nested-contract signatures; this
fixture pins construction and state-routed case dispatch on the same family.

Compile-checked only. Both `Index` and `Stream<machine S>` are recursive by
value, so they are proof-only ("Recursive mathematical data without a finite
runtime representation remain proof-only", `wiki/spec/proofs/contracts.md`;
"Here `Stream` denotes an appropriate proof-only family",
`wiki/spec/language/generics.md`). `build.omg` binds no program entry.

This was `samples/cli/probes/higher_order_streams`, which bound a
`ProgramEntry` that native realization could never lower.
