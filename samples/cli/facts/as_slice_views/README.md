# as_slice_views

The ordinary `&main.entries[..]` range lends a shared view of noncopy entries
through the imported core `Slice::range` requirement. The sample reads its
extent and records whether it still covers all four entries.

Check this probe with `omega --check samples/cli/facts/as_slice_views/main.omg`.
The corresponding executable regression checks the extent with a trap oracle:

```text
omega run --both tests/omega/pass/slices/runtime_imported_range_views_exit/main.omg
```

The range uses selected checked-provider supply and retains the bounds contract
and backing loan. The runtime fixture expects exit 0 in both engines.
