# Borrowed clock out-parameters

On macOS arm64, from the repository root:

```sh
omega run --both tests/omega/pass/time/runtime_clock_out_parameter_loans_exit/main.omg
```

The exact libSystem `_clock_gettime` import writes two signed 64-bit timespec
coordinates into the caller's original storage. Record names do not select
semantics: the same physical write also fills a fixed array. A helper and a
captured reference retain a nested record's backing; its unwritten suffix,
siblings and separately copied readings remain unchanged. Scalar nanosecond
reads and timespec reads observe the same invocation-private model before and
after a finite poll wait.
Native execution checks progress and normalized coordinates, not exact latency.
Both engines return normally with status zero; a stale initializer or independent
clock timeline traps. This program needs no ProcessExit delegation.
