# Scalar record backing lifetime

Run `omega update --project tests/omega/pass/structural/runtime_scalar_record_reclamation_exit --target linux_x86_64`, then `omega run --both tests/omega/pass/structural/runtime_scalar_record_reclamation_exit/main.omg`. Expected native and Terminal exit: 0.

4096 iterations copy and return an unrestricted record while a suspended caller retains its original record and a copied holder retains the shared referent. Every iteration reads the current record and that referent; after the call the caller checks the returned value against its original and the final stamp against 4096. The unknown entry seed prevents replacing the checked reads with a fixture-specific value.
