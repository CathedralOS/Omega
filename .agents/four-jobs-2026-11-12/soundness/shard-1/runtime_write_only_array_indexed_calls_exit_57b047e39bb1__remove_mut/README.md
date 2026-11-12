Nested write-only array loans forward runtime-selected records to ordinary
helper calls. Two writes select the right element; changing the selector then
writes the left element. Both padding fields remain untouched in caller storage.

Run `omega run --both tests/omega/pass/borrow/runtime_write_only_array_indexed_calls_exit/main.omg`
with stdin closed. The authored native and Terminal exit is 0.
