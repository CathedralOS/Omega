# Service permission and terminal authority

[Service reach](../language/effects.md) reports which abstract boundaries may be
reached. [Authority values](../resources/authority.md) carry permission for an
invocation. [Provider trust](provider_selection.md#executable-trust-and-containment)
records evidence for the realization. At an ecosystem admission boundary,
receiving policy separately checks the dangerous authority exercised by selected
physical mechanisms. None of these
axes substitutes for another.

A boundary requirement contributes only its explicitly declared service row,
not the name of its containing provider interface. Core-critical and
root-registered services are always surfaced by
[package audit policy](../packages/review.md#critical-service-audit-policy).
That policy neither replaces physical-mechanism classification nor grants
receiver permission. An empty abstract row cannot hide a selected mechanism's
filesystem or hardware authority; classification and admitted trust remain
independently visible.

<a id="compiler-owned-os-classification-tables"></a>

The compiler propagates the [authored boundary reach](../language/effects.md#declarations)
and records the exact selected import or syscall. Checking that source contract
requires no compiler-maintained table of OS API meanings. An explicit physical
exclusion checks the selected mechanism against independent classification
evidence. If a reachable mechanism has no such classification, the exclusion
is unproved, even when the boundary author declares empty reach.

The receiver selects the policy's versioned package, exact dependency closure
and concrete configuration independently of the offered program.
Its accepted commitments enter receiver-admission evidence. This authority-class policy
does not replace memory-safety guarantees or confine which objects may be touched.

## Artifact production versus receiver admission

Ordinary compilation and retained-Terminal lowering produce artifacts, not
permission to install or execute them in another environment. They require no
receiving policy. Absence of that input means no receiver-admission claim, not
an empty deny-all policy and not implicit acceptance. Do not add a mandatory
policy file, build declaration, or CLI switch merely to emit a binary.

The compiler still checks source contracts, project acceptance, selected-provider
custody, lowering, and all requested guarantees. It retains the mechanism and
authority evidence needed for those claims. An explicitly requested deployment
compatibility check applies the supplied policy and rejects incompatibility;
ordinary production must not silently invent such a request.

An OS, loader, registry, or deployment system selects and enforces its own
admission policy. Neither a binary nor its build script can authorize itself,
select the receiver's policy, or enforce rules on its host.
`omega run` combines production with a host execution request; production grants
no host permission. [Build-time authority](execution.md#restricted-build-requests)
is a separate concern because the build actually executes on the build host.

[Build-level exclusions](behavior_exclusions.md) may independently forbid exact
abstract services or physical terminal classes. Physical exclusions consume the
classified selected-mechanism closure below; missing classification is failure,
not evidence of absence. A class exclusion is a producer-requested guarantee,
not receiver permission, and grants no authority. It uses the same classification
semantics without requiring a receiving-policy acceptance table. An empty physical
class set cannot satisfy an abstract service exclusion if that service is invoked.

## Two-axis containment

Package review can examine nominal reach before provider selection. Installed
authority review walks the exact selected-provider closure to imports, syscalls,
intrinsics, firmware/table operations, and checked physical instructions.
The accepted versioned receiving policy relates two independently keyed tables:

| Table | Key and result |
| --- | --- |
| Service permission | Exact service identity, normalized schema commitment, and requirement identity map to permitted terminal classes. |
| Mechanism classification | Exact role-tagged normalized mechanism and checked/admitted contract map to exercised terminal classes. |

For receiver admission, every demanded terminal leaf must satisfy
`exercised classes subset-of permitted classes`.
Every admitted leaf needs exactly one explicit classification, including an
explicit empty set. The policy may be partial over the operating system's
coordinate universe, but it must be complete for the admitted demand. Unknown,
missing, duplicate, cyclic, string-only, or substituted leaves reject. Names,
filenames, aliases, package roles, reviewer judgments, and risk labels are not
authority identities. Provider context/schema cannot narrow physical behavior.

The mechanism identity is a closed role-tagged sum:

- Structural compiler intrinsic.
- Exact target/ABI, syscall number, and compiler-checked argument contract.
- Complete normalized foreign locator and admitted contract.
- Exact firmware/table identity and receiver contract.
- Exact checked physical-operation catalog entry.

Only meaningful coordinates belong to each role; its discriminant participates
in identity. A foreign contract uses its strong canonical boundary-entry-plan
commitment, not a provider report fingerprint. A syscall's checked argument
contract is distinct from that calling-plan commitment: it accounts for every
runtime value permitted by the carriers, or the exact retained constants,
ranges, handle provenance, and other constraints justifying a narrower union.
Syscall number zero is an ordinary valid coordinate when the target permits it.
Module-local IDs and readable service names cannot stand in for that evidence.

Each row classifies the union of authority reachable through all permitted
argument values. Narrowing requires exact checked constraint evidence in the
mechanism key. An empty row means no class in this vocabulary is exercised;
it does not mean pure, nonblocking, interference-free, trusted, custody-safe,
or object-confined. Exact service reach and demanded operation remain visible.

The closed classes are filesystem content read/write, metadata query/mutation,
directory enumeration, namespace mutation, process input, process output,
process termination, machine control, port I/O, interrupt control, interrupt
entry, root-memory access, host network stack access, and off-machine network
transmission. Historical broad `Filesystem`/`Process` risk summaries grant no terminal
permission. Separate cross-platform realizations satisfy one portable
requirement; `Linux + Windows` would mean reach union, not provider choice.

### Physical network authority

Report the [network service](platform_services.md#network-reach) a machine may
call, whether its selected mechanisms can use the host network stack, and
whether they can transmit off-machine. Two independent physical authority
classes describe the provider.

| Physical class | When it applies |
| --- | --- |
| `HostNetworkStack` | A selected mechanism can use the host operating system's networking facilities, including a loopback-only socket. |
| `OffMachineNetworkTransmit` | A selected mechanism can cause traffic to leave the machine, including name queries, handshakes, acknowledgments, or application data. |

A provider that only reads a recording in memory exercises neither network
class. A socket proven confined to loopback exercises `HostNetworkStack`
alone if its entire selected path stays local. A remote-capable host socket
can exercise both. An application protocol stack with a device-backed outbound
flow can exercise `OffMachineNetworkTransmit` without using the host socket
stack. Device access and custody still require their own evidence.

Classify the full behavior permitted by the checked mechanism and its backing.
For example, a host resolver that can fall back from its cache to a live query
may transmit off-machine even when a particular lookup hits the cache. A grant
that excludes transmission needs evidence covering every selected path.
Excluding abstract `NetworkSend` alone allows other operations that can send
protocol traffic.

## Consumer acceptance and replay

Permissions are explicit consumer input, not findings that approve themselves.
A supplied service row joins its exact schema and requirement to one method.
A partial table may be retained during binding construction; final selected
closure admission owns demand-completeness. A candidate carries no permission
merely because its service is familiar.

[Package review](../packages/review.md) retains each permission as its own
blocking obligation with exact service/requirement source custody. Broad risk
acceptance cannot authorize a specific terminal class. Direct compilation and
retained-Terminal re-entry compare checked package rows with project acceptance;
they do not additionally require an ecosystem's receiving permission set.
Missing, changed, or duplicate required package rows reject. Explicit receiver
admission separately checks the offered artifact against its independently
selected policy; unrelated receiving-policy rows are allowed. Project acceptance
must never be mirrored into that policy to make admission pass.

Accepted-permission transport derives solely from obligations accepted by fresh
root-policy replay. Re-entry binds the retained production report to the accepted
root's dependency closure, consumed sources, selected build machine,
invocation-local evaluation usage, build observations, and target. Aggregate
sponsor/session peaks remain orchestration custody, not invocation identity.
A constructed
policy, reconstructed proposal, or Terminal module cannot grant package admission.
When receiver admission is requested, its exact policy version and strong
commitment survive admission replay against the native artifact. Ordinary
artifact evidence does not fabricate that verdict. Classification alone grants
no execution, installation, or invocation authority.

## Portable filesystem control and lifecycle authority

The six filesystem facets distinguish operations, not confined objects. Raw
integer descriptors establish no object-confinement guarantee. The following
dispositions apply to exact mechanisms and accepted contracts; requirement names
identify examples, not classification keys.

| Cohort | Disposition under the stated contract |
| --- | --- |
| `read_link` | Metadata query of the stored link target, not content read or directory enumeration merely because it returns a path. |
| `sync`, `sync_data` | Explicit empty for durability-only completion of existing changes. |
| `lock_file`, `lock_file_ex`, `unlock_file` | Explicit empty for ordinary locking/unlocking; there is no coordination class. |
| `seek`, `get_osfhandle`, `duplicate` | Explicit empty for accepted position, handle-representation, and alias operations. |
| `get_last_error`, `errno` | Explicit empty for recognized error-state observation, independent of declaring service. |
| `close`, `find_close`, `close_handle` | Explicit empty only with an established ordinary-release contract, not automatically for generic raw-handle close. |

Locks can deny others access and aliases can share state without becoming content
or metadata mutation. Adding a coordination class requires a concrete policy
customer, a new accepted version/commitment, and revalidation. The six facets
do not require six core traits. Services permit classes and selected mechanisms
exercise them; source placement inside a raw filesystem service cannot force a broader
mechanism into filesystem-only classes.

### Requested mutation versus incidental completion

Requesting or arming namespace mutation exercises namespace-mutation authority.
Ordinary release does not additionally exercise it merely because release
completes another actor's already-requested deletion. Likewise, durability-only
completion is not a new logical write request. This is attribution policy, not
an assumption excluding concurrent deletion.

Preserve Windows shared-delete behavior; removing `FILE_SHARE_DELETE` to obtain
an empty row changes the protocol. Do not invent a portable POSIX delete-on-close
open flag as its counterpart. An ordinary-empty release must exclude deletion
attached to the released handle itself, whether at acquisition or by a later
disposition-changing operation, and all other behavior outside ordinary release.
A generic Win32 closer may release a last kill-on-close job handle and terminate
processes; filesystem placement does not narrow that mechanism.

A completely classified broad mechanism can fail filesystem-only containment.
An unknown/incompletely classified mechanism instead fails classification.
Both refuse realization, not source requirement formation. Demand-completeness
never permits fabricating a broad union for an unsupported leaf.

### Bounded occurrence-specific release proof

Use one retained occurrence, one exact mechanism-plus-checked-contract key, and
one classification. Constrained and unconstrained uses have distinct keys, not
competing rows. An unsupported unconstrained operation does not block an
independently proved constrained use.

The derivation must establish:

1. Successful acquisition under the exact object/argument contract, excluding
   attached deferred deletion.
2. Handle identity through aliases and control flow, excluding unproved escape,
   storage, duplication, substitution, or foreign intervention.
3. Preservation by every intervening handle-taking call. An empty or query-only
   authority set is not a preservation contract.
4. One release on applicable returning paths, no subsequent use, and allowed
   release behavior under explicit external-environment assumptions.

Constants, a digest of an asserted proof, and method-name exceptions do not
satisfy these obligations. Bind the constraint identity to the derivation and
exact occurrence. Changed arguments, flow, selected calls, or contracts require
re-establishment. Failure falls back only to a justified unconstrained
classification; otherwise reject, never retain an unsupported narrowed row.

For an open/query/close sequence, access zero, `OPEN_EXISTING`, and
`BACKUP_SEMANTICS` without `DELETE_ON_CLOSE` are useful inputs, not proof of
object kind or complete lifecycle. Check successful acquisition before query or
close and preserve read/write/delete sharing. The path need not be constant.
An `i64 in FileHandle` qualification alone cannot prove lifecycle: copies may
survive release and refer to recycled handles. Bounded flow proof may reason
about the integer without making it linear; general owned opaque handles are
not a prerequisite.

Failed acquisition, escape, alias reuse, second close, handle-changing calls,
and stale/substituted proofs must prevent narrowing. External pending deletion
completed by ordinary close remains compatible with empty classification;
deferred deletion attached to that handle does not.

<a id="freestanding-machine-owner-authority"></a>

## Privileged services

A checked instruction contributes the services required by its catalog contract
to the containing machine's reach. Calls propagate that reach through helpers.
The consuming project must accept new or widened critical reach for the package
and target through [ordinary package acceptance](../packages/review.md#critical-service-audit-policy).

| Hardware service | What the package can reach |
| --- | --- |
| `InterruptControl` | Changing the current processor's interrupt mask, including disabling, enabling and restoring it. |
| `InterruptEntry` | Entry into an installed interrupt handler. Acknowledging a device through a port also contributes `PortIo`. |
| `PortIo` | Reading or writing hardware I/O ports. The provider scopes access to particular ports. |
| `RootMemoryAccess` | Accessing physical memory through authority supplied by the system root. Ordinary reads and writes within a borrowed buffer add no such reach. |
| `MachineControl` | Broad processor control, including halting a processor, switching the active page table and unrestricted system-register writes. |

For example, `cli` and a helper that only saves and masks interrupts contribute
`InterruptControl`. If a later revision also switches the active page table,
package review must surface the added `MachineControl` reach. A compound
operation accounts for every effect its contract permits. An ordinary arithmetic
instruction or memory fence adds no hardware service reach. At receiver
admission, the [physical classification](#two-axis-containment) of a leaf that
only changes the interrupt mask is likewise `InterruptControl`. A provider
that uses additional mechanisms must account for their actual authority.

### Where the operation can run

For each output, the compiler checks that the selected target supports the
instruction and that the selected provider contracts cover its required
authority. Freestanding describes a target environment. Selecting it alone
does not establish machine ownership. This rule retires `Build.freestanding`.
If a build binds both firmware and a hosted process, the compiler must reject
a direct interrupt-mask change in the hosted output when its execution contract
lacks that privilege, even if the firmware contract supplies it.

A Linux host can check and emit firmware for a target that supplies machine
control. Emission needs no permission to change the build host's interrupt
mask. At execution the receiver supplies the actual scoped authority. A Psi
test can mask interrupts on an explicitly supplied modeled processor. Live
port I/O requires authority for the ports used. Publishing an interrupt table
requires the consumer's authority for that CPU and table. Hosted tests use the
same target or model contracts without a separate check-only grant. See
[interpreted assembly](../language/assembly.md#interpreted-execution),
[checked instructions](hardware_materialization.md#checked-instructions) and
[device access](../resources/device_access.md).

An untrusted binary's manifest is only a claim: permitted CPU instructions can
bypass it. Source-derived contracts rely on the trusted compiler. Locally
interpreting or lowering portable Psi does not admit supplied native
bytes. [Executable installation](executable_installation.md) requires exact
admitted artifact and placed-byte provenance. Lower-rooted bootstrap assurance
and hardware control-flow hardening are separate assurance layers. Admitted
call/state exits do not make opaque in-process code memory-safe or its
executable inventory complete.
