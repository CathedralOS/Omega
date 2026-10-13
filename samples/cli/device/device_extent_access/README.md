# device_extent_access

Checked `DeviceLoanProvider` contract over provider-issued opaque data
(`DeviceLoan`, `DeviceOrderingEvent`) with a loopback provider whose ten
crossings are `via ForeignBinding::CompilerIntrinsic` leaves. The bound entry
is inert (`Expected exit: 0`); the two DMA drivers are checked but unreachable.

## Frontier

- Native build and run of the inert entry are green on macos_arm64. The
  product retains a checked boundary plan for every `DeviceLoanProvider`
  requirement and every loopback leaf, including the requirements that lend
  a linear opaque (`publish`, `maintain_cache` take `&DeviceLoan`) and the
  leaves whose parameters carry a domain qualification (`release`,
  `lend_for_read`, `lend_for_write`, `notify`, `acquire`).
- Making a driver reachable from `Main::main` needs native realization of the
  ten crossings. Omega's compiler-intrinsic inventory
  (`omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/provider_admission/terminal_authority_policy/inventory.rs`)
  closes over hosted exit/write/read byte, builtin functions, float binary
  operations, integer comparisons and numeric conversions; it has no
  device-loan mechanism, so a reachable `reserve` call has no native boundary
  realization to admit. That is the next edge, not a checker gap.
