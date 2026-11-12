Run `omega run --both tests/omega/pass/borrow/runtime_write_only_call_result_observation_exit/main.omg`.

Exit 0 requires both caller-owned write-only destinations to be replaced, a
readable field and runtime-selected element of independent call results to be
observed, and the array producer's counter to advance exactly once. A write-only
argument in a producer does not grant or remove read access to its result.
