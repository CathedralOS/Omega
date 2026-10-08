# Crash abandons an open borrowed-storage window

Run `omega update --project tests/omega/pass/ownership/runtime_borrowed_window_crash_exit --dangerously-accept-all`, then `omega run --both tests/omega/pass/ownership/runtime_borrowed_window_crash_exit/main.omg`.

The helper extracts a linear child through an exclusive loan and crashes without
repair. The authored result is Trap, not a normal return, rollback, or cleanup
success. A returning path with the same hole must still reject.
