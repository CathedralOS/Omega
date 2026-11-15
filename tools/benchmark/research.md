# Performance tooling reference log

2026-10-29: define native resource measurements and safe profiler use.
Queries: `site.learn.microsoft.com GetProcessMemoryInfo PeakWorkingSetSize
QueryProcessCycleTime process`, `site.learn.microsoft.com wpr command line
instancename start CPU filemode`, `site.man7.org wait4 ru_maxrss children rusage
wait4`, `site.man7.org perf stat json output perf record`.

- [Microsoft process memory information](https://learn.microsoft.com/en-us/windows/win32/psapi/process-memory-usage-information):
  read how working set and pagefile differ. Use `PeakWorkingSetSize` for peak
  resident bytes, not Job Object commit-charge counters. PageFaultCount counts
  both fault kinds; do not call it major page faults.
- [GetProcessTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes):
  process CPU time is in 100 ns FILETIME units. Keep user and
  kernel CPU separate from wall time. Include only this process on Windows.
- [WPR commands](https://learn.microsoft.com/en-us/windows-hardware/test/wpt/wpr-command-line-options#instancename):
  read `-instancename` and checked local `wpr -help start`, `-help stop`,
  and `-help advanced`. Put the unique instance last on every control command.
  Stop or cancel only the instance this run started. A failure never
  permits cancelling the default session or changing privileges.
- [wait4](https://man7.org/linux/man-pages/man2/wait4.2.html):
  it waits for one exact child; collect its status and resource
  usage together. Do not subtract process-wide RUSAGE_CHILDREN snapshots under
  concurrency. macOS returns RSS bytes; Linux returns KiB. State which
  descendants the kernel counts.
- [perf stat](https://man7.org/linux/man-pages/man1/perf-stat.1.html#JSON_FORMAT)
  and [perf record](https://man7.org/linux/man-pages/man1/perf-record.1.html):
  read JSON counter values, event-running metadata, and call-graph capture.
  Unsupported or uncounted values are unavailable, not zero. Capture profiler
  output apart from uninstrumented comparisons and keep the raw files.

QueryProcessCycleTime's old processthreadsapi documentation URL was
unavailable; the pinned local windows-sys source places the API in
`Win32::System::WindowsProgramming`. It counts processor cycles, not elapsed
nanoseconds. The platform calibration fixture checks positive cycles for known
CPU work; it infers no frequency conversion.

Rejected: periodic RSS polling as a peak measurement (misses short peaks),
Job Object commit charge labelled RSS, hardware-counter permission workarounds,
cancelling another's WPR session, and treating separately captured historical runs as
interleaved A/B observations. The runner's
resource, integrity, profiling, and statistical controls enforce these.
