An owned byte argument retains its live `ascii_only` predicate while a later
argument replaces the source with non-ASCII UTF-8. Both engines must exit 0.

```text
omega update --project tests/omega/pass/borrows/runtime_copied_byte_predicate_snapshot_exit --target macos_arm64
omega run --both tests/omega/pass/borrows/runtime_copied_byte_predicate_snapshot_exit/main.omg
```

The predicate follows the exact bytes copied at argument evaluation. It does not
certify the caller's rewritten storage. The paired negative control is
`fail/calls/copied_argument_ensures_cannot_restore_mutated_source`.
