`omega --check tests/omega/fail/borrow/stored_write_only_observation/main.omg`
must reject all three observing machines. A shared array descriptor or enclosing
shared reference does not make the selected write-only referent readable.
