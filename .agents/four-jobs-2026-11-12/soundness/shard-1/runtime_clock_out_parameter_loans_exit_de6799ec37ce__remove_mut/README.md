# Borrowed clock out-parameters

On macOS arm64, from the repository root:

```sh
omega run --both tests/omega/pass/time/runtime_clock_out_parameter_loans_exit/main.omg
```

The exact libSystem `_clock_gettime` import writes two signed 64-bit timespec
coordinates into the caller's original storage. Signed clock IDs serve the
record and nanosecond reads; unsigned clock IDs serve the array read. Each
declaration keeps its logical carrier while supplying the same 32-bit native
clock ID. Record names do not select
semantics: the same physical write also fills a fixed array. A helper and a
captured reference retain a nested record's backing; its unwritten suffix,
siblings and separately copied readings remain unchanged. Scalar nanosecond
reads and timespec reads observe the same invocation-private model before and
after finite poll and nanosleep waits. A captured read-only timespec request
serves both returned-status and discarded-result nanosleep calls. Independent
readings require each wait to advance the shared model; neither call changes
the request's fields or suffix. A non-null numerical remainder carrier grants
no storage access on this signal-free success path.

Darwin's invalid-request path needs an errno model and is explicitly unavailable;
it cannot reuse Linux's raw kernel error convention. Interruption, cancellation
and native latency remain outside the private time model. The native contract
is described by [Apple's nanosleep documentation](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/nanosleep.2.html)
and [its libc implementation](https://github.com/apple-oss-distributions/Libc/blob/main/gen/nanosleep.c).
Native execution checks progress and normalized coordinates, not exact latency.
Both engines return normally with status zero; a stale initializer or independent
clock timeline traps. This program needs no ProcessExit delegation.
