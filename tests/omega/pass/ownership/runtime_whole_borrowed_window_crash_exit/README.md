Run `omega update --project tests/omega/pass/ownership/runtime_whole_borrowed_window_crash_exit --dangerously-accept-all`, then `omega run --both tests/omega/pass/ownership/runtime_whole_borrowed_window_crash_exit/main.omg`.

Whole-referent extraction through a retained exclusive alias abandons its hole
and detached linear value at an authored crash. The helper's scalar result
signature does not establish a value or run normal-return cleanup on that path.
