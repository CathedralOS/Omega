# Performance tooling reference log

2026-10-29: clarify native resource definitions and safe profiler ownership.
Queries: `site.learn.microsoft.com GetProcessMemoryInfo PeakWorkingSetSize
QueryProcessCycleTime process`, `site.learn.microsoft.com wpr command line
instancename start CPU filemode`, `site.man7.org wait4 ru_maxrss children rusage
wait4`, `site.man7.org perf stat json output perf record`.

- [Microsoft process memory information](https://learn.microsoft.com/en-us/windows/win32/psapi/process-memory-usage-information):
  read the working-set/pagefile distinctions. Use `PeakWorkingSetSize` for peak
  resident bytes, not Job Object commit-charge counters. PageFaultCount includes
  both fault types; do not rename it major page faults.
- [GetProcessTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes):
  process-owned CPU accounting uses 100 ns FILETIME units. Preserve user and
  kernel CPU separately from wall time. Include only this process on Windows.
- [WPR commands](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/wpr-command-line-options#instancename):
  read `-instancename` ownership and checked local `wpr -help start`, `-help stop`,
  and `-help advanced`. Put the unique instance last on every control command.
  Stop/cancel only the instance this invocation successfully started. Failure
  does not authorize cancellation of the default session or privilege changes.
- [wait4](https://man7.org/linux/man-pages/man2/wait4.2.html):
  read the exact-child wait and rusage contract; collect its status and resource
  usage together. Do not subtract process-wide RUSAGE_CHILDREN snapshots under
  concurrency. macOS returns RSS bytes; Linux returns KiB. Keep the kernel's
  descendant-accounting scope explicit rather than pretending providers match.
- [perf stat](https://man7.org/linux/man-pages/man1/perf-stat.1.html#JSON_FORMAT)
  and [perf record](https://man7.org/linux/man-pages/man1/perf-record.1.html):
  read JSON counter values, event-running metadata, and call-graph capture.
  Unsupported/not-counted values are unavailable, not zero. Capture profiler
  outputs separately from uninstrumented comparisons and preserve raw artifacts.

QueryProcessCycleTime's former processthreadsapi documentation URL was
unavailable; local pinned windows-sys source places the API in
`Win32::System::WindowsProgramming`. Its count is processor cycles, not elapsed
nanoseconds. The platform calibration fixture checks positive cycles for known
CPU work; no frequency conversion is inferred.

Rejected: periodic RSS polling as a peak measurement (misses short peaks),
Job Object commit charge labelled RSS, hardware-counter permission workarounds,
unowned WPR cancellation, and treating separately captured historical runs as
interleaved A/B observations. Corresponding acceptance lives in the runner's
resource, integrity, profiling, and statistical controls.
