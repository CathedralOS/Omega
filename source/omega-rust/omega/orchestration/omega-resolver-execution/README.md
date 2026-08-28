# Omega Resolver Execution

This crate owns the platform-specific native launch boundary for package-source
resolution. Callers select one closed resolver phase and provide already
verified executable and quarantine paths; callers cannot author sandbox policy
text or containment claims.

## Structure

- `src/lib.rs` defines the closed phase vocabulary, host-backend identity,
  command construction, native policy, inherited resource limits, opaque
  per-command policy observations, and canaries.

## Current enforcement

- macOS uses a compiler-fixed Seatbelt profile. Canaries prove ordinary writes
  and remote TCP are denied in closed phases and unlisted descendant execution
  is denied on the tested host. The profile imports Apple's mutable
  `system.sb`, which grants special-file writes and local socket access, so
  these canaries are not reported as universal strict guarantees.
- Unix children intersect compiler CPU, core-file, single-file, and descriptor
  ceilings with stricter inherited limits. Linux and Android additionally
  apply an address-space ceiling.
- Other Unix hosts currently receive limits without strict filesystem/network
  confinement. Windows retains the package layer's existing process-container
  floor but has no strict backend here yet.
- Every command is constructed together with a bounded canonical policy
  observation binding the backend, phase, generated policy hash, numeric
  resource ceilings, primary executable path, normalized bounded descendant-
  executable path set, mutable root, and every closed native guarantee as
  `Enforced`, `Unavailable`, or `NotRequired`. There is no public constructor
  or decoder. This describes configuration, not execution or executable
  content identity; `require_strict` rejects the current backends.

This is engineering enforcement and one input to a future package-source
receipt, not that accepted receipt. The macOS profile still permits broad file
reads, imported system exceptions, and unbrokered outbound endpoints; aggregate resource quotas and
Linux/Windows strict backends remain package-manager tasks. See
[`SOURCE_RESOLVER_SECURITY.md`](../omega-packages/SOURCE_RESOLVER_SECURITY.md).
