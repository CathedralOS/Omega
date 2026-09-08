# Service permission and terminal authority

[Service reach](../language/effects.md) reports which abstract boundaries may be
reached. [Authority values](../resources/authority.md) carry permission for an
invocation. [Provider trust](provider_selection.md#executable-trust-and-containment)
records evidence for the realization. Receiving policy separately checks the
dangerous authority exercised by selected physical mechanisms. None of these
axes substitutes for another.

## Two-axis containment

Package review can examine nominal reach before provider selection. Installed
authority review walks the exact selected-provider closure to imports, syscalls,
intrinsics, firmware/table operations, and checked physical instructions.
The accepted versioned receiving policy relates two independently keyed tables:

| Table | Key and result |
| --- | --- |
| Service permission | Exact service identity, normalized schema commitment, and requirement identity map to permitted terminal classes. |
| Mechanism classification | Exact role-tagged normalized mechanism and checked/admitted contract map to exercised terminal classes. |

For every demanded terminal leaf, `exercised classes subset-of permitted classes`.
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
directory enumeration, namespace mutation, process output, process termination,
machine control, port I/O, interrupt control, interrupt entry, and root-memory
access. Historical broad `Filesystem`/`Process` risk summaries grant no terminal
permission. Separate cross-platform realizations satisfy one portable
requirement; `Linux + Windows` would mean reach union, not provider choice.

## Consumer acceptance and replay

Permissions are explicit consumer input, not findings that approve themselves.
A supplied service row joins its exact schema and requirement to one method.
A partial table may be retained during binding construction; final selected
closure admission owns demand-completeness. A candidate carries no permission
merely because its service is familiar.

[Package review](../packages/review.md) retains each permission as its own
blocking obligation with exact service/requirement source custody. Broad risk
acceptance cannot authorize a specific terminal class. Direct compilation and
retained-Terminal re-entry compare checked package rows with the independently
accepted permission set and then the distinct receiving policy. Missing, changed,
or duplicate package rows reject; unrelated receiving-policy rows are allowed.

Accepted-permission transport derives solely from obligations accepted by fresh
root-policy replay. Re-entry binds the retained production report to the accepted
root's dependency closure, consumed sources, selected build machine,
invocation-local evaluation usage, build observations, and target. Aggregate
sponsor/session peaks remain orchestration custody, not invocation identity.
Retained profile/native target and production subject must agree. A constructed
policy, reconstructed proposal, or Terminal module cannot grant package admission.
Exact receiving policy version and strong commitment survive native-artifact
replay; classification alone grants no execution, installation, or invocation
authority.

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
exercise them; source placement inside `FilesystemHost` cannot force a broader
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

## Privileged services

Checked instructions retain separate service identities, not a catch-all
`Privileged` or lowercase effect category:

| Service | Scope |
| --- | --- |
| `MachineControl` | Halting, interrupt enable/disable, control-register and MSR operations. |
| `PortIo` | Port input/output, potentially mediated by hardware permission maps. |
| `Mmio` | Volatile device access under admitted mapping authority. |

Machine-control authority normally belongs to the trusted boot/kernel domain;
listing its service does not establish ownership of the machine. Each exact
instruction contract independently requires its machine or consumer-defined
publication authority, or no authority. Direct emission and checked helpers
retain the same recursive reach rules. See
[checked instructions](hardware_materialization.md#checked-instructions) and
[device access](../resources/device_access.md).

An untrusted binary's manifest is only a claim: permitted CPU instructions can
bypass it. Source-derived contracts rely on the trusted compiler or independently
checked evidence; verified portable IR removes trust in its producer only for
the checked IR claims. Locally interpreting/lowering it does not admit supplied
native bytes. [Executable installation](executable_installation.md) requires
exact admitted artifact and placed-byte provenance. Native refinement PCC,
lower-rooted bootstrap assurance, and hardware control-flow hardening are
separate assurance layers. Admitted call/state exits do not make opaque
in-process code memory-safe or its executable inventory complete.
