# Complete Darwin stat ABI

Fact: the shared Darwin stat representation preserves all SDK fields, signed carriers and its complete 144-byte extent through C exchange and transport.

Parent: LIBRARY-REPRESENTATION-FAMILIES in [TASKS.md](../../../../TASKS.md).
Nearest row: [explicit nested C representations](../c_layout_nested_representation/README.md).
This row adds a real platform ABI boundary, not a larger mixed-width count: the
actual Darwin SDK `struct stat` contains four complete timespecs, signed 32-bit
device identifiers and the reserved tail through byte 143. Both std and these
consumers import [the same ABI definitions](../../../../source/library/std/targets/macos_arm64/filesystem_abi.omg).
No sample duplicates the Omega struct and no test-only public accessor is added.

The C fixture fills and verifies all 23 scalar field components. Each guard
also reads those components in Omega, including all nanoseconds, high unsigned
bits, negative signed values and both reserved array elements. C sizeof and
alignment must agree with 144/8. Every C checker field has a self-test mutation.
On Mac the fixture includes the installed SDK's `sys/stat.h`; other hosts use
an explicit Darwin-format fixture replica and make no native SDK claim.

Local, owned-field, by-value parameter, call-result, match-join and const-name
transport are qualified on the matching Mac host.
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

The exact `1a31aa120cdd` optimized Mac producer passed all eight checking
controls and all four corpus native/Terminal programs at exit 0 against SDK 15.5.
The corpus stages of the [focused run](https://github.com/CathedralOS/Omega/actions/runs/41335219379)
passed; its ordinary public reviews exceeded the former 180-second allowance.
The same retained compiler then completed all four ordinary public package
reviews and `omega run --both` executions at exit 0, including canonical Terminal
inspection, with a measured 600-second allowance for each review phase.


The [six-control follow-on run](https://github.com/CathedralOS/Omega/actions/runs/41349339798)
at exact `79f174854e30` passed the call-result, match-join and const-name checks,
their exact policy-mismatch twins, and all three native/Terminal executions at
exit 0 against SDK 15.5. Private specializations retain unique labels when a
constant's demanded checking closure projects the declaration roster; their
complete semantic applications and source commitments remain unchanged.

The existing portable metadata/set-times control also checks and exits 0
natively. Its Terminal `_futimens` adapter gap predates this migration and remains
under FILESYSTEM-PLATFORM-SERVICES in [TASKS.md](../../../../TASKS.md); the
private native sponsor does not grant extra filesystem capability.

The final nested-machine, other-machine and selected-target transports and
other-host observations remain open; the
complete parent also retains conversions and Product parity.

`file_times` checks the same shared two-element C timestamp array
through the C ABI with UTIME_OMIT/NOW values -2/-1. Its twin
changes only the receiving element representation to bare DarwinTimespec.
