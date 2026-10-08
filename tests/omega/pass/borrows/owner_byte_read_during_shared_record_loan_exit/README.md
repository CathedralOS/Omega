# Shared owner observations

After ordinary package review, run:

```sh
omega run --both tests/omega/pass/borrows/owner_byte_read_during_shared_record_loan_exit/main.omg
```

Both engines must exit 70. The owner observes its byte field while a shared
record loan remains live for a subsequent call. A nonescaping read may share
that storage; exclusive loans and mutations retain their separate restrictions.
