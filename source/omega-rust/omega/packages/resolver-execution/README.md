# Omega Resolver Execution

This crate owns the platform-specific native launch boundary for package-source
resolution. Callers select one closed resolver phase and provide already
verified executable and quarantine paths; callers cannot author sandbox policy
text or containment claims.

## Structure

- `src/lib.rs` is the documented public entrance and reexports only the closed
  request, observation, backend, child-lifecycle, and endpoint types.
- `src/model.rs` owns closed phases, transports, guarantees, backend identity,
  resource ceilings, and opaque canonical policy observations.
- `src/request.rs` validates compiler-selected executable and custody paths;
  it does not interpret package-authored locator text.
- `src/backend.rs` validates one launch request, selects the verified host
  backend, and constructs an opaque prepared execution with its policy.
- `src/prepared.rs` owns the only caller-configurable command surface and the
  bounded identity of its exact program, arguments, environment, and working
  directory. It exposes no raw command extraction or executable replacement.
- `src/process/` consumes prepared executions, owns child-container lifecycle
  and compiler-owned process limits, and issues completion only after explicit
  whole-container termination or confirmed absence plus child reaping.
- `src/network/` owns typed endpoint policy and observations (`model.rs`), the
  shared transfer ceiling (`budget.rs`), the bounded loopback route
  (`broker.rs`), CONNECT framing (`connect_protocol.rs`), bidirectional copying
  (`relay.rs`), and the fixed helper process boundary (`helper.rs`). Its
  `mod.rs` is the explicit network facade.
- `src/confinement/` owns native confinement facts and implementations.
  `macos.rs` owns Seatbelt policy/custody and its native canaries;
  `windows.rs` owns suspended launch, Job Object assignment and limits,
  whole-job termination, and active-process-zero completion. Linux has no
  falsely named confinement module: its current enforcement is only the Unix
  process limits in `src/process/limits.rs`.

Tests follow the responsibility they exercise: backend request/observation
tests live under `src/backend/tests/`, endpoint tests under `src/network/`, and
native macOS/Windows tests beside their confinement owner.

## Dependency direction

```text
lib.rs (public reexports only)
  -> backend.rs
       -> model.rs + request.rs + network/ + process/limits.rs
       -> confinement/ (host policy and guarantee classification)
       -> prepared.rs (opaque configured launch)
  -> process/ (owned child lifecycle)
       -> prepared.rs + completion observation
       -> confinement/windows.rs only on Windows
  -> network/ (sealed endpoint policy, broker, and connector helper)

confinement/ -> model.rs
model.rs -> network endpoint policy encoding
```

The model and request vocabulary do not depend on command construction.
Platform modules never broaden caller input or mint package acceptance; they
only realize and classify the compiler-owned launch policy. Unsupported native
guarantees remain `Unavailable` rather than acquiring placeholder modules or
best-effort claims.

## Current enforcement

- macOS uses compiler-fixed, self-contained Seatbelt profiles with no host-
  profile imports. SSH discovery and fetch admit broad reads. Repository
  initialization, inspection, and HTTPS discovery/fetch confine metadata and
  content reads to their phase root plus exact executable/runtime paths and
  literal ancestors. HTTPS discovery/fetch additionally admit metadata-only
  lookup within the compiler-selected Git helper directory and the fixed
  `/etc/ssl` alias needed to reach the canonical TLS root. HTTPS discovery/fetch
  additionally admit the fixed system TLS configuration root
  `/private/etc/ssl`. Every phase admits the exact compiler-selected executable
  set and write-data to the fixed `/dev/null` sink. Initialization
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
  descendant-executable path set, discovery/inspection content-read roots when
  applicable, mutable root, and every closed native guarantee as `Enforced`,
  `Unavailable`, or `NotRequired`. There is no public constructor
  or decoder. Spawning consumes the prepared value. A completion observation is
  issued only after whole-container termination or confirmed absence and reap;
  it binds that policy to the exact command identity, normalized exit status,
  and cleanup disposition. Standard-input content remains separately bound by
  the protocol owner because native `Stdio` handles have no portable inspectable
  identity. This is lifecycle provenance, not executable-content identity or a
  source receipt; `require_strict` rejects the current backends.

The broker bounds CONNECT request bytes and headers, the complete DNS result
set collected before any upstream connection, accepted connections, buffers,
connection/relay duration, and bytes accepted for relay. Every route in one
source resolution shares one compiler-owned bidirectional transfer budget;
CONNECT framing and DNS traffic are excluded. Endpoint observations retain
closed outcomes, effective socket peers, and exact uploaded/downloaded counts.
An over-ceiling read is not forwarded or charged, closes the tunnel, and emits
`TransferCeilingReached`. This does not establish TLS or SSH host trust,
credential custody, package acceptance, or a receipt, and it cannot prevent a
helper from bypassing the broker on a backend without endpoint confinement.

The installed `omega` package includes `omega-resolver-connect` beside the main
binary. HTTPS uses Git's command-scoped proxy configuration. SSH invokes the
companion through a fixed ProxyCommand name and a sealed helper-only `PATH`;
compiler-authored environment fields carry the broker and target authorities,
so package locator text never becomes shell syntax.

This is engineering enforcement and one input to a future package-source
receipt, not that accepted receipt. macOS initialization, inspection, and HTTPS
discovery/fetch now mark `FilesystemReadsConfined` enforced, but SSH discovery/
fetch still permit broad reads, so complete resolver-wide filesystem-read
confinement remains unavailable. The fixed TLS root
is not a TLS trust receipt or credential-custody claim. Aggregate CPU, memory,
and process-count confinement remains unavailable on Unix; during-write object-
store quotas, Linux/Windows endpoint confinement, and complete strict backends
remain package-manager tasks. See
[`SOURCE_RESOLVER_SECURITY.md`](../manager/SOURCE_RESOLVER_SECURITY.md).
