# Return safely after a handler calls an Omega helper

The interrupt handler calls an ordinary helper that emits `T`. Its emitted body retains two functions and a real internal call. The [CPU harness](../../../../../tools/native_entry_cpu/README.md) requires eight `T`/return pairs and intact interrupted state after every return. It uses the original emitted image addresses, not a handwritten handler.

This has the same entry and runtime oracle as the direct-body sibling. It checks that the body’s actual callees are included in its state-use obligations. The hosted application is compiled only; the CPU test creates no acknowledgement or installed-root authority.
