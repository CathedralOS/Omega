An owned local record lends primitive fields and indexed bytes to a scalar-returning helper.
Neighboring fields stay distinct, and a later loan observes an earlier store in the original backing.

Run `omega run --both tests/omega/pass/calls/local_record_leaf_loan_scalar_calls_exit/main.omg`.
Both engines must exit 70.
