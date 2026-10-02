# Local record call arguments

This program lends distinct local records, fixed-index record-array elements,
and a scalar field inside an element to Unit-, scalar-, and record-returning
calls. The record-returning callee forwards its borrowed parameters to a
successor state. The caller observes the writes, the returned reports, and the
untouched neighboring element. Native and checked interpretation must both exit
70.

Prepare this checkout's package review for the running host, then run the
unchanged program. On Linux x86-64:

```sh
omega update --project tests/omega/pass/calls/local_records_mixed_record_result_arguments_exit --target linux_x86_64
# Review build/package-manager/review-linux_x86_64.txt in the fixture directory.
omega update --project tests/omega/pass/calls/local_records_mixed_record_result_arguments_exit --resume
omega run --both tests/omega/pass/calls/local_records_mixed_record_result_arguments_exit/main.omg
```

Package acceptance records the review decisions; it does not claim a dependency
audit. Keep the resulting `omega.lock` and `omega.admissions` local: the lock
records absolute source paths and acceptance for this checkout, and cannot be
reused as another checkout's accepted baseline.
