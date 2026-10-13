# nested_const_projection

Nested constant projections resolve through module scope and through
the imported spelling alike — `settings::VALUE.middle.inner.value` and
`VALUE.middle.inner.value` both compute 7; same-spelled local carriers
cannot redirect the imported constant's fields, and `make()` may be a
machine call inside a const initializer.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
