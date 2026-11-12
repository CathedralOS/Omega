# nested_binding_call

Checking applies an exact boundary-trait requirement through a nested helper
(`read()` applying `Reading::read()`) and through a state. Merely checking a
helper's unselected requirement (`unused` applying `Unselected::read()`) does
not demand an unrelated provider selection.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
