# trait_parent_binding

`trait IntSink<S>: Sink<S, i32>` — a child trait pins
its parent's arguments; `satisfies IntSink<Counter>`
names the parent's member (`Sink::put =
Counter::put`), and the implementer carries
`satisfies Sink<Counter, i32>::put`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
