A non-interval scalar domain keeps its complete predicate through array stores, a shared slice read, and a scalar result. The copied value remains 38 after its backing element is replaced with 0.

Run `omega run --both tests/omega/pass/slices/runtime_predicate_qualified_scalar_copy_exit/main.omg`; both engines must exit 0.
