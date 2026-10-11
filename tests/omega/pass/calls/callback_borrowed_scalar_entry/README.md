# Borrowed scalar callback entry

After accepting the generated package review, run:

```sh
omega update --project tests/omega/pass/calls/callback_borrowed_scalar_entry --offline
omega update --project tests/omega/pass/calls/callback_borrowed_scalar_entry --resume
omega run tests/omega/pass/calls/callback_borrowed_scalar_entry/main.omg
```

Real macOS Arm64 qsort sorts two u64 elements. The comparator reads both
borrowed scalars through an ordinary helper and returns their ordering; the
caller observes 17 then 91 and exits 0, or 71 for a mismatch. The foreign
pointer word must become an internal scalar-loan descriptor, not be relabeled
as one. These loans are synchronous and call-scoped, not future registrations.
