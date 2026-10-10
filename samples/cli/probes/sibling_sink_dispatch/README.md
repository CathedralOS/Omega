# sibling_sink_dispatch

Same-spelled `boundary trait Sink { emit() -> i32 }` in sibling modules (`module left;`/`module right;`) are distinct dispatch slots — `left::Provider::emit()`=11 vs `right::Provider::emit()`=37. Learning: `reaches`/`invokes` reject module-qualified names (`left::Sink` parses as `::` error) — qualified services can only be reached through direct provider calls here.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
