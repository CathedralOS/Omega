`omega run --both tests/omega/pass/host/runtime_descriptor_nested_array_window_exit/main.omg`

Both engines return zero and independently write `ir\n` to stdout. An ordinary
helper forwards a nonzero-offset window of the caller's nested fixed array to
the foreign byte-view boundary, then forwards an empty window at its end.
The caller observes its original bytes unchanged after the helper returns.
