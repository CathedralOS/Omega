# Reserved generic construction

Run `omega run --both tests/omega/pass/drops/runtime_generic_reserved_construction_exit/main.omg`.
The program establishes the contained token before observing or cleaning it and
expects exit 0 from native and Terminal execution. The field store establishes
the reserved generic container; the later scalar read and automatic child
cleanup retain the value 42.
