The earlier owned byte argument retains its `ascii_only` predicate while a
later argument replaces the source with non-ASCII UTF-8. Both engines must exit 0.

```text
omega update --project tests/omega/pending/borrows/runtime_copied_byte_predicate_snapshot_exit --target macos_arm64
omega run --both tests/omega/pending/borrows/runtime_copied_byte_predicate_snapshot_exit/main.omg
```

Source checking succeeds. Native compilation rejects with
`Lowering(OperationProofUnavailable(ObligationId(9223372036854775809)))`, also
reproduced through the public CLI at published baseline 6c0fd10e915.
Preserve the captured argument's byte-predicate evidence through the Terminal
call-requirement join; the caller's later non-ASCII source is a different value.
The negative `fail/calls/copied_argument_ensures_cannot_restore_mutated_source`
must reject only its final `take_ascii` call, never the earlier saved value.

The missing owner is Terminal operation-fact reconstruction for
`StructuralLeafCopy`: record field transport returns early for a bounded-byte
leaf and contributes no `ByteSequencePredicate` fact for its fresh destination.
Copy content predicates from the exact source/path at that occurrence. The later
source invalidation must remain effective; a saved predicate belongs to the
copied bytes, not the rewritten caller storage.
