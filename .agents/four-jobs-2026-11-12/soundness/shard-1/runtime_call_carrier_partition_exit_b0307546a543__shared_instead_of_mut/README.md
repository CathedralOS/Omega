Ordinary calls preserve the prepared argument-carrier partition. Interleaved
copy records, packed cases and scalar arrays retain their value payloads;
an exclusive primitive argument retains its original backing. The Unit callee
changes the caller's primitive while observing both record arguments.
Scalar-returning and record-returning callees carry the same mixed roster.
Both engines must exit 0.

```sh
omega update --project tests/omega/pass/calls/runtime_call_carrier_partition_exit --target macos_arm64
omega run --both tests/omega/pass/calls/runtime_call_carrier_partition_exit/main.omg
```
