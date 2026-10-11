# NEW070 initial validation checkpoint

This immutable source note records preparation observations, not the final
runtime verdict. Subsequent exact-command receipts live in `build/validation/`
and are included in the independent review bundle.

Base: `b4008bb3c7894aebad47eaccf8af12bb4a8006b1`. Isolated local branch
`new070-spatial`; no remote worker branch and no worker dependencies.

Build: `source /workspace/cloud-setup/activate.sh` then
`CARGO_TARGET_DIR=/workspace/new070-target mbx build -p omega`. Exit 0,
2m15s, dev unoptimized, four Rust jobs, cgroup four CPU quota and 16 GiB memory.
Binary SHA256 `be6ae351fe985373a5943ec9638b46a2fc3478ad9c3998f3eac5e74cefee598f`.
Matching library: `$PWD/source/library` via `OMEGA_LIBRARY_ROOT`.

Oracle and tolerance froze before first check: zero numeric tolerance and
byte-exact expected stdout. `sha256sum -c evidence/oracle-freeze.sha256` from the
repository root verifies the frozen source inputs and independent expected data.

The default sandbox compiler invocation rejected source resolver storage:
`git cache entry / is invalid: cache custody ancestry is owned by an unrelated user`.
The same check in the approved normal per-command context ran to a package
preparation rejection: std's `LinuxX86_64Application` entry schema needs an
accepted program-entry binding, and the diagnostic prescribes `omega update`.
Custody and trust guards were preserved.

Exact first check: `/workspace/new070-target/debug/omega --check --target
linux_x86_64 samples/cli/independent/spatial_sensor_deployment/main.omg`.
Approved-context exit 1, approximately 3m33s. The first review attempt ran approximately 18 minutes, then rejected because
the local source tree changed during immutable capture. Validation notes were
edited while it ran; the author corrected this workflow by freezing all source
edits before retrying review. No acceptance decision was attempted or denied.
The second review checked both source packages successfully and produced 285
pending policy decisions. No application execution occurred at this checkpoint. The
separate intended-invalid verdict is recorded below.

Claim registration timed out. Pending ticket:
`3e29114c9c3547419f86054a0adcd7d9`. Later read-only status succeeded and showed
NEW070 absent; a registration retry timed out and another status confirmed
absence. Registration was not claimed successful. Development continued in isolation.

Separate complete intended-invalid control: `/workspace/new070-target/debug/omega
--check samples/cli/independent/spatial_sensor_deployment/controls/invalid/invalid.omg`.
Approved-context exit 1, exact sole diagnostic:
`` `loans` holds `main.sensor` as mutable more than once ``. This is the authored
exclusive-loan rejection, not a package, parser or unrelated proof rejection.
The control declares no foreign dependency or entry schema and is excluded from
the ordinary valid-sample discovery by using `invalid.omg` as its root filename.
