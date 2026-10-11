# Borrowed record callback entry

Prepare this disposable fixture through the normal package review. Accept the
reviewed package decisions in the rendered review before resuming:

```sh
omega update --project tests/omega/pass/calls/callback_borrowed_record_entry --offline
omega update --project tests/omega/pass/calls/callback_borrowed_record_entry --resume
omega run tests/omega/pass/calls/callback_borrowed_record_entry/main.omg
```

On macOS Arm64 the real C qsort sorts two 16-byte records. Its comparator
receives two borrowed records, calls an ordinary helper that reads both values,
and returns a signed comparison. The caller checks sorted values and their
associated stamps, then exits 0 (71 on a mismatch). This pins real inbound
referent transport and foreign write-back, not compilation or inert addresses.

The borrow is call-scoped and qsort invokes the comparator synchronously.
This establishes neither durable registration nor a future callback root.
