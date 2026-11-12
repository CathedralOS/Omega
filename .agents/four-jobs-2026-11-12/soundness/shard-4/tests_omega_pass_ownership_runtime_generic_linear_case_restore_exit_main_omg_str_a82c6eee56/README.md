# Generic linear outcomes restore borrowed storage

Run `omega run --both tests/omega/pass/ownership/runtime_generic_linear_case_restore_exit/main.omg`.

Failure returns the original linear token. Success consumes it and constructs
its replacement inside the generic outcome carrier. Both paths restore the
borrowed holder exactly once, retaining backing 73 and guard 19 while producing
values 41 and 42. The final token uses its authored no-op cleanup hook.

Constructor typing retains complete static arguments before claim production;
the ordinary completed-value events, not constructor syntax, conserve custody.
