# as_slice_views

Remaining receiver-spelling probe with the `.as_slice()` view constructor:
`main.entries.as_slice()` yields `&[Entry]`; `view.len`
reads the extent. Migrate this source to the ordinary `&main.entries[..]`
route once the selected core range requirement has executable supply.

## Gaps found while authoring (filed)

The canonical core range probe is:

```text
omega run --both tests/omega/pending/slices/runtime_imported_range_views_exit/main.omg
```

It passes package visibility but stops at `outer calls: ordered statement call`:
the bodyless returned-view signature is not planned. Boundary calls still lack
returned-view transport and admitted descriptor supply. A selected declaration
must not be bypassed with builtin slicing.
The owning frontier is `STATIC-MACHINE-MIGRATION` in `TASKS.md`.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
