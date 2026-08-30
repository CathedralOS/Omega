# Omega Resolver Execution

This crate owns the platform-specific native launch boundary for package-source
resolution. Callers select one closed resolver phase and provide already
verified executable and quarantine paths; callers cannot author sandbox policy
text or containment claims.

## Structure

- `src/lib.rs` is the documented public entrance and reexports only the closed
  request, observation, backend, child-lifecycle, and endpoint types.
- `src/model/` owns closed phases and transports (`phase.rs`), native guarantee
  vocabulary (`guarantees.rs`), and canonical policy observations (`mod.rs`).
- `src/request.rs` validates compiler-selected executable and custody paths;
  it does not interpret package-authored locator text.
- `src/backend/` selects and verifies the host backend (`host.rs`), validates
  launch requests (`request.rs`), constructs policy observations
  (`observation.rs`), and realizes opaque prepared executions
  (`preparation.rs`). Its `mod.rs` is the small type boundary.
- `src/prepared.rs` owns the only caller-configurable command surface and the
  bounded identity of its exact program, arguments, environment, working
  directory, and null-or-pipe standard-stream dispositions. It exposes no raw
  command extraction, executable replacement, inherited streams, or arbitrary
  caller-opened handles.
- `src/process/` consumes prepared executions, owns child-container lifecycle
  and compiler-owned process limits, and issues completion only after explicit
  whole-container termination or confirmed absence plus child reaping.
- `src/network/` owns typed endpoint policy and observations (`model.rs`), the
  shared transfer ceiling (`budget.rs`), the bounded loopback route
  (`broker.rs`), CONNECT framing (`connect_protocol.rs`), bidirectional copying
  (`relay.rs`), and the fixed helper process boundary (`helper.rs`). Its
  `mod.rs` is the explicit network facade.
- `src/confinement/` owns native confinement facts and implementations.
  `macos/` separates Seatbelt policy encoding, metadata paths, executable
  custody, and native command realization;
  `linux.rs` owns Landlock ABI-v5 filesystem mutation and execution policy;
  `windows/` separates suspended launch, Job Object setup, limits, lifecycle,
  whole-job termination, native adapters, and behavior-named tests.

Tests follow the responsibility they exercise: backend request/observation
tests live under `src/backend/tests/`, endpoint tests under `src/network/`, and
native platform tests beside their confinement owner.

## Dependency direction

```text
lib.rs (public reexports only)
  -> backend.rs
       -> model.rs + request.rs + network/ + process/limits.rs
       -> confinement/ (host policy and guarantee classification)
       -> prepared.rs (opaque configured launch)
  -> process/ (owned child lifecycle)
       -> prepared.rs + completion observation
       -> confinement/linux.rs or confinement/windows.rs where applicable
  -> network/ (sealed endpoint policy, broker, and connector helper)

confinement/ -> model.rs
model.rs -> network endpoint policy encoding
```

The model and request vocabulary do not depend on command construction.
Platform modules never broaden caller input or mint package acceptance; they
only realize and classify the compiler-owned launch policy. Unsupported native
guarantees remain `Unavailable` rather than acquiring placeholder modules or
best-effort claims. These rows report optional platform hardening; they are not
universal prerequisites for successful source resolution.

## Current enforcement

- macOS uses compiler-fixed, self-contained Seatbelt profiles with no host-
  profile imports. Every resolver phase deliberately retains broad filesystem
  reads. Ambient reads are required for ordinary user and system Git/SSH
  configuration, recursive config includes, credential and transport helpers,
  identity files, known-host data, and equivalent host-selected inputs. Broad
  reads do not grant fetched source execution: every phase still admits only
  the exact compiler-selected executable set and write-data to the fixed
  `/dev/null` sink. Initialization
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
- Unix children mark the non-standard ambient descriptors observed at launch
  close-on-exec, then intersect compiler CPU, core-file, single-file, and
  descriptor ceilings with stricter inherited limits. Linux uses atomic
  `close_range`; other Unix hosts snapshot `/dev/fd`. Linux and Android
  additionally apply an address-space ceiling.
- Linux kernels with fully available Landlock ABI v5 handle every ABI-v5
  filesystem right. A dedicated restricted thread launches the child so Omega's
  other threads remain unrestricted. Reads remain broad and therefore
  unclaimed; handled content/namespace writes are admitted only beneath the
  exact mutable root, device writes only to `/dev/null`, and path-based
  execution only for verified regular files in the primary and bounded helper
  set. Enforcement requires both `FullyEnforced` and `no_new_privs`. These are
  useful controls, not the complete `FilesystemWritesConfined` or
  `ExecutablePathsConfined` guarantees: Landlock does not mediate several
  metadata mutations and cannot prevent executable memfds or anonymous
  executable code. Both rows therefore remain unavailable. The package manager
  falls back to the ordinary Unix resource-limit backend when Landlock ABI v5
  is unavailable. Landlock also does not establish endpoint confinement,
  direct-egress denial, or aggregate descendant custody.
- Other Unix hosts currently receive limits without strict filesystem/network
  confinement. Windows commands are created suspended, assigned to a resolver-
  owned kill-on-close Job Object, and resumed only after assignment. The job
  prohibits breakaway by omission and enforces at most 16 active processes,
  2 GiB committed memory per process, 4 GiB across the job, and 120 aggregate
  user-CPU seconds. Completion requires both the primary child status and the
  Job Object's active-process-zero event. Setup failure terminates and reaps the
  suspended child rather than falling back. Windows therefore enforces
  descendant containment, process-count, CPU-time, and aggregate CPU/memory
  rows, but still lacks filesystem, executable-path, and endpoint confinement.
- Every prepared execution owns a bounded canonical policy observation binding
  the backend, phase, closed network transport when
  applicable, sealed endpoint route when applicable, generated policy hash,
  numeric resource ceilings, primary executable path, normalized bounded
  descendant-executable path set, discovery/inspection phase roots when
  applicable, mutable root, and every closed native guarantee as `Enforced`,
  `Unavailable`, or `NotRequired`. There is no public constructor
  or decoder. Spawning consumes the prepared value. A completion observation is
  issued only after whole-container termination or confirmed absence and reap;
  it binds that policy to the exact command identity, normalized exit status,
  and cleanup disposition. Each standard stream must explicitly be null or a
  compiler-owned pipe before spawn; arbitrary pre-opened handles and ambient
  inheritance are not representable. Piped standard-input content remains
  separately bound by the protocol owner. These rows are lifecycle provenance
  retained by the package-source receipt, not claims that the host operating
  system excluded all ambient authority.

The broker bounds CONNECT request bytes and headers, the complete DNS result
set collected before any upstream connection, accepted connections, buffers,
connection/relay duration, and bytes accepted for relay. Every route in one
source resolution shares one compiler-owned bidirectional transfer budget;
CONNECT framing and DNS traffic are excluded. Endpoint observations retain
closed outcomes, effective socket peers, and exact uploaded/downloaded counts.
An over-ceiling read is not forwarded or charged, closes the tunnel, and emits
`TransferCeilingReached`. This bounds the brokered route and records its actual
peer; it does not replace the transport's normal TLS or SSH authentication and
cannot prevent a helper from bypassing the broker on a backend without endpoint
confinement.

The installed `omega` package includes `omega-resolver-connect` beside the main
binary. HTTPS uses Git's command-scoped proxy configuration. SSH invokes the
companion through a fixed ProxyCommand name and prepends its directory to the
host `PATH`;
the exact system command shell and current login shell needed by Git and
OpenSSH are independently verified and included in the native executable set;
compiler-authored environment fields carry the broker and target authorities,
so package locator text never becomes shell syntax. User and system Git/SSH
configuration and the inherited host environment remain available for ordinary
credential helpers, agents, identity files, known-host policy, and proxies.

This engineering enforcement is recorded in each successful package-source
receipt. Every macOS phase marks `FilesystemReadsConfined` unavailable because
ambient host reads are intentional. Other unavailable rows remain visible as
platform-hardening facts and do not prevent resolution. See
[`SOURCE_RESOLVER_SECURITY.md`](../acquisition/SOURCE_RESOLVER_SECURITY.md).
