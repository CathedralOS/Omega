# Forwarded descriptor snapshot

`omega run --both tests/omega/pass/slices/runtime_forwarded_range_extent_snapshot_exit/main.omg`
must exit 0 in native and Terminal execution. A local view retains the endpoint
values captured at construction, despite a later endpoint assignment. A second
helper replaces that descriptor with its forwarded result, using inequalities
rather than an equality-shaped length-preservation guarantee. The caller uses
the composed published guarantees for an unguarded nonempty-view call.
