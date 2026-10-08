# Linux clock and wait loans

On Linux x86-64 or arm64, from the repository root:

```sh
omega run --both tests/omega/pass/time/runtime_kernel_clock_wait_loans_exit/main.omg
```

The exact selected kernel clock and nanosleep syscalls share one explicitly
supplied invocation-private time model in Terminal execution. Native execution
uses the actual kernel; neither simulation nor cross-target artifact generation
qualifies native timing or interruption behavior.

A helper and captured loans preserve the caller's original nested clock output
and shared wait request. Clock writes leave the output suffix and siblings
unchanged; waits do not modify the request. Both retained and discarded status
declarations use the same wait operation. Negative seconds and unnormalized
nanoseconds return raw `-EINVAL` without mutation. Successful signal-free waits
do not access a nonzero remainder carrier. Both engines return normally with
status zero; an incorrect status, stale output or changed neighboring storage
traps. Native waits may overshoot; the program checks progress, not exact latency.
