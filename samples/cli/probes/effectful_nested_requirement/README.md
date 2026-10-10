# effectful_nested_requirement

An effectful call as the argument of a requires-bearing call:
`accept(produce(7, &mut main.counter))` — the writes to `counter.hits`
complete before `accept` judges `requires value <= 20`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
