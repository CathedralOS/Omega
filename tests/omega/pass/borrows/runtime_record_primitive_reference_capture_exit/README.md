Run `omega run --both tests/omega/pass/borrows/runtime_record_primitive_reference_capture_exit/main.omg`.

A direct reference capture in a record retains the original mutable primitive home. The helper writes through the stored reference, and the caller observes 9 in its original storage; both engines exit 0.
