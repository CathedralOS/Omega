Run `omega run tests/omega/pass/host/runtime_console_byte_payload_bounds_exit/main.omg --both`
after ordinary local package review. Both engines exit 70.

The active Byte payload admits both 0 and 255. Its extracted i32 establishes
the store state's requirements before exact conversion to u8. A zero payload
is still Byte, while zero-initialized result storage is Eof. The separate
negative and above-byte constructors must reject.
