# nested_binding_call

Checking retains the same logical Binding through a nested field and a
borrowed parameter — `read(&mut main.session.reading)` and the borrowed
`binding.read()` receiver. Merely checking a helper's `Binding`
parameter (`unused`) does not demand an unrelated provider selection.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
