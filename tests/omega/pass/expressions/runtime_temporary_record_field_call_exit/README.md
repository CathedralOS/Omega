A scalar tail reads a field directly from a call-produced record. The call
runs exactly once before field observation; its mutation and returned value
must both remain visible. Both engines must exit zero.
