# Complete Darwin stat ABI

Fact: the shared Darwin stat representation preserves all SDK fields, signed carriers and its complete 144-byte extent through C exchange and transport.

Parent: LIBRARY-REPRESENTATION-FAMILIES in [TASKS.md](../../../../TASKS.md).
Nearest row: [explicit nested C representations](../c_layout_nested_representation/README.md).
This row adds a real platform ABI boundary, not a larger mixed-width count: the
actual Darwin SDK `struct stat` contains four complete timespecs, signed 32-bit
device identifiers and the reserved tail through byte 143. Both std and these
consumers import [the same ABI definitions](../../../../source/library/std/targets/macos_arm64/filesystem_abi.omg).
No sample duplicates the Omega struct and no test-only public accessor is added.

The C fixture fills and verifies all 23 scalar field component. Each guard
also reads those components in Omega, including all nanoseconds, high unsigned
bits, negative signed values and both reserved array elements. C sizeof and
alignment must agree with 144/8. Every C checker field has a self-test mutation.
On Mac the fixture includes the installed SDK's `sys/stat.h`; other hosts use
an explicit Darwin-format fixture replica and make no native SDK claim.

Local, owned-field and by-value parameter places are the first transport chunk.
Each twin changes only the receiving policy to an equal-geometry distinct
implementation. All four hosted entry bindings remain explicit. Field values
are established by a mutable C exchange; the compiler cannot substitute stale
zero initializer facts or assume the foreign checker succeeds.

Psi 00–04 retains the exact public std type, selected child plans and policy
identity; borrowed inputs retain source backing and copy obligations. Psi 05–07
and Omega 00–02 must preserve the complete selected C field geometry and ABI
pointer lane. Omega 03–09 must emit the stores, copies, projections and foreign
calls so any wrong offset, signed carrier, omitted timestamp half, wrong branch
or lost array element traps in native/Terminal execution. The source metadata
does not grant storage or field access rights.

Source inputs and the fixture are being qualified. No Omega runtime success is
claimed yet. The other six transports, broader SDK integration and other-host
observations remain open; the complete parent also retains conversions and
Product parity.

`file_times` checks the same shared two-element C timestamp array
through the C ABI with UTIME_OMIT/NOW values -2/-1. Its twin
changes only the receiving element representation to bare DarwinTimespec.
