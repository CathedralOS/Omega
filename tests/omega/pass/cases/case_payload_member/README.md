# case_payload_member

Case arms READ a bound record payload's member field:
`Shape::Boxed { inner } -> emit(main, inner.x)` — the nested read
resolves through a copy of the record member staged under the
selecting case edge.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
