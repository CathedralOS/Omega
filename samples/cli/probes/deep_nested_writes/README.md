# deep_nested_writes

Deep write paths: a struct-in-struct field write `main.nested.second.b`, a
sum constructed into a struct field `main.holder.msg = Msg::Pong { .. }`
destructured back through the nested path, and an array-of-struct element
field write `main.pair[1].a` (indexed + member combined).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
