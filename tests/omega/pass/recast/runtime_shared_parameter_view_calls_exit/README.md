Shared integer and representation views borrow the same scalar parameter home.
Both views are passed to ordinary helpers, with one observed before conditional
completion uses the other. The entry exercises both the helper-call and literal
completion arms. Returning the scalar result must dispose the remaining
reference carriers without disposing their borrowed storage.

Run `omega run --both tests/omega/pass/recast/runtime_shared_parameter_view_calls_exit/main.omg`.
Both engines must exit 0; a wrong bit image reaches the authored trap.
