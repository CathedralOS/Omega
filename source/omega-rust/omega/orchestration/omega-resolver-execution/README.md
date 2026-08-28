# Omega Resolver Execution

This crate owns the platform-specific native launch boundary for package-source
resolution. Callers select one closed resolver phase and provide already
verified executable and quarantine paths; callers cannot author sandbox policy
text or containment claims.

## Structure

- `src/lib.rs` defines the closed phase vocabulary, host-backend identity,
  command construction, native policy, inherited resource limits, opaque
  per-command policy observations, and native canaries.
- `src/network.rs` defines typed requested endpoints, the fixed-bound loopback
  HTTP CONNECT broker, sealed route policy, and bounded endpoint observations.

## Current enforcement

- macOS uses compiler-fixed, self-contained Seatbelt profiles with no host-
  profile imports. Discovery and fetch admit broad reads. Initialization and
  inspection instead admit broad metadata reads but confine file-content reads
  to their exact mutable quarantine or retained bare repository, the selected
  executable set, `/dev/null`, and the literal filesystem-root directory entry
  required by the native process runtime. Every phase admits the exact compiler-selected
  executable set and write-data to the fixed `/dev/null` sink. Initialization
  and fetch additionally admit writes only beneath the exact mutable quarantine
  root. Discovery and fetch require one explicit closed HTTPS or SSH authority
  and admit outbound network. Only SSH receives the OpenDirectory libinfo
  lookup and `kern.hostname` read required by the pinned client, plus the exact
  `hw.pagesize_compat` read required to initialize the compiler-owned Rust
  CONNECT helper; HTTPS receives none.
  The child may connect only to the exact compiler-owned loopback broker port;
  the broker accepts only the typed requested host and port and records the
  effective connected peer. Initialization and inspection deny network and
  reject any supplied transport authority or route. They also omit
  `process-fork`, so even an allowlisted executable cannot become a descendant;
  discovery and fetch retain descendant creation for their verified transport
  chains. The applicable
  filesystem-write, network-denial, executable-path, and phase-applicable
  descendant-containment rows are enforced.
- Unix children intersect compiler CPU, core-file, single-file, and descriptor
  ceilings with stricter inherited limits. Linux and Android additionally
  apply an address-space ceiling.
- Other Unix hosts currently receive limits without strict filesystem/network
  confinement. Windows retains the package layer's existing process-container
  floor but has no strict backend here yet.
- Every command is constructed together with a bounded canonical policy
  observation binding the backend, phase, closed network transport when
  applicable, sealed endpoint route when applicable, generated policy hash,
  numeric resource ceilings, primary executable path, normalized bounded
  descendant-executable path set, inspection content-read root when applicable,
  mutable root, and every closed native guarantee as `Enforced`, `Unavailable`,
  or `NotRequired`. There is no public constructor
  or decoder. This describes configuration, not execution or executable
  content identity; `require_strict` rejects the current backends.

The broker bounds CONNECT request bytes and headers, the complete DNS result
set collected before any upstream connection, accepted connections, buffers,
and connection/relay duration. Its endpoint observation records closed
connection outcomes and effective socket peers. It does not establish TLS or
SSH host trust, credential custody, network-transfer quotas, package
acceptance, or a receipt.

The installed `omega` package includes `omega-resolver-connect` beside the main
binary. HTTPS uses Git's command-scoped proxy configuration. SSH invokes the
companion through a fixed ProxyCommand name and a sealed helper-only `PATH`;
compiler-authored environment fields carry the broker and target authorities,
so package locator text never becomes shell syntax.

This is engineering enforcement and one input to a future package-source
receipt, not that accepted receipt. macOS still permits broad reads during
network phases and broad metadata reads during nonnetwork phases, so complete
filesystem-read confinement remains unavailable. Aggregate resource quotas and
Linux/Windows endpoint confinement and strict backends remain package-manager
tasks. See
[`SOURCE_RESOLVER_SECURITY.md`](../omega-packages/SOURCE_RESOLVER_SECURITY.md).
