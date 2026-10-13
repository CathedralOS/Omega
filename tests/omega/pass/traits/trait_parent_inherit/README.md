# trait_parent_inherit

Child trait binds its parent: `trait IntSink<S>: Sink<S, i32>` + `CounterIntSink satisfies IntSink<Counter> { Sink::put = Counter::put }` parent-qualified row fill.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
