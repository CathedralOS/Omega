# Captured range endpoint

`omega run --both tests/omega/pass/slices/runtime_captured_range_end_exit/main.omg`
must exit 70 in both engines. A copied length measurement forms the whole range
of an unknown-length parameter, and a returned view retains the caller's narrowed
two-byte extent. The original descriptor and scalar copies remain unchanged.
