# Value consumers preserve stored backing relations

Each helper replaces a record-carried mutable view and returns its backing.
Direct expressions, local captures, transition values and assignment values
produce references without invoking their authority. Nested calls still have
their own write effects; a named transition is an invocation, not a value return.

This fixture checks source relations. The companion
`slices/runtime_helper_stored_view_return_exit` fixture requires actual native
and Terminal execution with the descriptor's new count and element contents.
