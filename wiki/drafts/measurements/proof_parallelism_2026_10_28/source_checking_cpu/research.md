# Source-checking CPU attribution research

Question: Which passes own the remaining source-checking process CPU on the merged source?
Fixed source: cec7b6657299525e62aa6fc2168d3768b4d33b6a plus archived temporary probes.
Fixed workload: authored Dungeon program and EOF, windows_x86_64.
Bound: six interleaved cold trials (off,on,on,off,on,on), two warm profiles, one BOTH control.
No optimization or before/after speedup claim. Keep all valid observations.

Query: site.learn.microsoft.com GetThreadTimes GetCurrentThread thread user kernel time FILETIME

Primary sources read:
- https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getthreadtimes
- https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentthread
- GetProcessTimes/GetSystemTimes research retained in ../graph_phase_cpu/research.md and ../cold_variation/research.md.

Thread times are kernel/user execution durations in 100ns units. Pseudohandles refer to the calling thread and need no close. Process times include other threads. Capture both; actual wall coordinates determine nested ownership. Subtract disjoint children only, report residuals, reject ambiguous overlap. CPU quantization is not zero work. API failure aborts observations. PSI retains forbid(unsafe_code); host artifacts owns FFI. No new dependencies.

Instrumentation boundary: selective coarse pass names only; ordinary fine wall profiling disabled. Same executable probe-off controls check gross observer disturbance, not a precise correction. Endpoint system CPU records external contention without pausing other work.
