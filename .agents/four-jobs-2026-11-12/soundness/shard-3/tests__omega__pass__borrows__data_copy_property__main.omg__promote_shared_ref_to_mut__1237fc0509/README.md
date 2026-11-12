# data_copy_property

`data Point [copy]` — the declared type property verifies because every
field is structurally copyable, at zero runtime cost; `Holder [copy]`
qualifies only because `Point` declares the property itself (nested
propagation).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
