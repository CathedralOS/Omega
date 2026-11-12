The earlier constructor field captures an issued owned ticket. A later field
replaces the caller's source with an unissued ticket. The observer receives
the original 41/9 payload and membership, while the caller sees 0/0.

After package preparation, run:

```text
omega update --project tests/omega/pass/calls/runtime_constructor_child_membership_snapshot_exit --target macos_arm64
omega run --both tests/omega/pass/calls/runtime_constructor_child_membership_snapshot_exit/main.omg
```

Both engines must exit 0. Supply finite stdin when running with `--both`.

Constructor children retain their own completion facts and declared destination
paths. The fail fixture `constructor_child_membership_precedes_repair` checks
the opposite order: later issuance cannot validate an earlier unissued child.
`constructor_child_distinct_epochs_not_equal` checks that separate reads across
a mutation cannot become the same scalar observation.
