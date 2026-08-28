# Omega Resolver Execution

This crate owns the platform-specific native launch boundary for package-source
resolution. Callers select one closed resolver phase and provide already
verified executable and quarantine paths; callers cannot author sandbox policy
text or containment claims.

## Structure

- `src/lib.rs` defines the closed phase vocabulary, host-backend identity,
  command construction, native policy, inherited resource limits, and canaries.

## Current enforcement

- macOS uses a compiler-fixed Seatbelt profile. Network and writes derive from
  the phase, and descendant execution is restricted to the supplied verified
  tool set.
- Unix children intersect compiler CPU, core-file, single-file, and descriptor
  ceilings with stricter inherited limits. Linux and Android additionally
  apply an address-space ceiling.
- Other Unix hosts currently receive limits without strict filesystem/network
  confinement. Windows retains the package layer's existing process-container
  floor but has no strict backend here yet.

This is engineering enforcement, not an accepted package-source receipt. The
macOS profile still permits broad file reads and unbrokered outbound endpoints;
aggregate resource quotas, Linux/Windows strict backends, and opaque receipt
issuance remain package-manager tasks. See
[`SOURCE_RESOLVER_SECURITY.md`](../omega-packages/SOURCE_RESOLVER_SECURITY.md).
