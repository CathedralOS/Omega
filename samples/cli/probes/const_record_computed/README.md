# const_record_computed

This probe constructs a nominal constant record with a computed integer and
Boolean field, then reads `CONFIG.size` and `CONFIG.enabled` from machine bodies.
Its anonymous `SIZE` initializer requires `(7 / 2 * 2) as u64` under
[explicit integer landing](../../../../wiki/spec/language/numeric_values.md#fractional-intermediate-diagnostics).
That source migration belongs to [the landing task](../../../../wiki/drafts/designs/explicit_integer_landing.md).

Compile-checked only: no standard-library dependency and
no `Main::exit`. Run a check target, not native.
