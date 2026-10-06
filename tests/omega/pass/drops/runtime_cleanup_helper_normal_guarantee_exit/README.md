# Normal guarantees at cleanup

Run `omega run --both tests/omega/pass/drops/runtime_cleanup_helper_normal_guarantee_exit/main.omg`.
Both executions must exit 0.

The helper establishes the nominal hook's compound prerequisite. The caller
captures the changed values before an unconditional outgoing cleanup edge;
the next state observes those captures after cleanup. A guard on that edge
must not hide a missing guarantee by supplying the prerequisite itself.
