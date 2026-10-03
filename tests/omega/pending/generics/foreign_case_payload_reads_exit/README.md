# Selected foreign payload reads

This preserves the stronger version of the generic case-selection witness:
both helpers return a field from their selected record payload instead of a
constant. `Second` puts a `u64` before its `i32` field, so both payload layouts
must remain exact. Do not replace these reads with case-specific constants.

```sh
omega run --both tests/omega/pending/generics/foreign_case_payload_reads_exit/main.omg
```

Expected native and checked-interpreter exit: 70. With the generic membership
referent fix over `3038c39a718`, the checked interpreter returns 70, but native
artifact production rejects with `case payload observation requires an
established case`. Package review is a separate prerequisite of the public
command.

Stage 05's `structural_fields::resolve_case_payload` refuses a nested record
payload read while its binding is deferred. Successor-edge emission prepares
these leaves after case selection, but direct scalar-return lowering does not
carry that preparation. Extend the shared selected-payload observation and
evaluation ordering, retaining independent active-case validation; do not add
a generic-instance or return-spelling exception. Then graduate this unchanged
program into the native corpus. SQUALR-HEADLESS retains the application acceptance.
