# Owner Questions

Only the two owners, Zac and Jarod, answer an entry. An owner's word in a
session is the ruling. To close an answered question at the owner's direction,
update the governing documentation or specification, point existing references
there, and remove the question from this queue in the same PR. Track any
remaining implementation work in the owning task.

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Psi representation and encoding design is delegated; human review is deferred
under [the current policy](source-rust/subsystems/compiler/psi/README.md#changes-to-psi).
Missing IR operations or a schema change needed for accepted behavior are
implementation work, not owner questions.

Before a proposed surface becomes an owner question, audit whether it is
implemented, whether any authored source uses it, and whether ordinary Omega
already expresses the customer. An unimplemented, unused spelling that adds no
capability beyond existing checked machines is retired rather than redesigned.
Hypothetical future utility does not by itself preserve syntax; a concrete
customer requiring a distinct capability may propose a new surface later.

Every `OWNER-BLOCKED` escalation must name an independently motivated product
requirement or credible external use case. Existing corpus use is not required.
A test, experiment, benchmark, or implementation task cannot be the sole
motivation, and machinery introduced only to support such work is removed or
kept non-authoritative rather than promoted into an owner decision.

Application ports, including Squalr, are implementation customers, not a separate
class of owner questions. Port shape, API adaptation, missing library operations,
and compiler rejections stay with the implementer under the existing contracts.
Escalate only a specific unresolved language, architecture, or trust rule, citing
the contracts that leave it open. The application may motivate that question;
its name or a workaround does not establish one. Do not ask the owner to approve
ordinary port choices or treat a workaround as permission to reduce acceptance.

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

Bootstrap construction follows the owner's [variant mandate](bootstrap/MINIMIZATION.md).
Architecture exploration and implementation within that mandate are delegated.
Unresolved changes outside it belong in [OWNER_QUESTIONS_BOOTSTRAP.md](OWNER_QUESTIONS_BOOTSTRAP.md).
This file continues to own product-language questions.

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

## Open questions

<a id="compiler-owned-os-classification-tables"></a>
### 1. Should the compiler hardcode OS-specific API classification tables, or only report the exact OS calls?

The permission contract says the receiving OS, loader, or deployment system
selects its own versioned policy independently of the offered program, while
`Build::exclude_physical_authority` reuses mechanism classification without
requiring a receiving policy.
[Production versus receiver admission](wiki/spec/build/permissions.md#artifact-production-versus-receiver-admission) ·
[Two-axis containment](wiki/spec/build/permissions.md#two-axis-containment) ·
[Physical exclusions](wiki/spec/build/behavior_exclusions.md#physical-exclusion-authoring)

Today the compiler itself ships hardcoded OS tables and per-OS flow proofs
inside `provider-admission`: Win32 DLL exports (`windows_imports.rs`), Linux
syscall numbers (`posix_operations.rs`), and macOS `_poll` / clock imports
(`darwin_poll.rs`, `host_cohorts.rs`). The 12,501-line Windows handle release
replay (`windows_handle_*.rs`) was archived to `_recycling/` on 2026-11-27
because nothing reached it, leaving `CloseHandle`, `DuplicateHandle`, and
non-sandbox Linux `openat`/`close`/`pread64`/`pwrite64` unclassified.
[Windows imports](source-rust/subsystems/compiler/psi-to-native/verification/provider-admission/src/terminal_authority_policy/windows_imports.rs) ·
[POSIX syscalls](source-rust/subsystems/compiler/psi-to-native/verification/provider-admission/src/terminal_authority_policy/posix_operations.rs) ·
[Darwin poll](source-rust/subsystems/compiler/psi-to-native/verification/provider-admission/src/terminal_authority_policy/darwin_poll.rs) ·
[Policy root](source-rust/subsystems/compiler/psi-to-native/verification/provider-admission/src/terminal_authority_policy.rs)

**Needs both owners (Jarod and Zac).** Jarod's lean is that the compiler's
abstraction should stay clean and OS-neutral rather than baking OS-specific
syscall/DLL knowledge into the compiler:
- **Option A (OS-neutral compiler):** The compiler only records and reports the
  exact foreign imports, syscalls, and checked instructions a binary reaches.
  OS-specific mapping to physical authority classes (and any per-API lifecycle
  proofs) lives outside the compiler in an external versioned policy or
  deployment checker—meaning `Build::exclude_physical_authority` either takes
  an explicit policy input or operates only on compiler-owned primitives/effects.
- **Option B (Compiler-owned OS tables):** Keep hardcoded Windows/Linux/macOS
  classification tables in `provider-admission` so `Build::exclude_physical_authority`
  works out of the box with no policy input, and accept maintaining per-OS API
  knowledge (and cross-OS handle-release proofs) inside the compiler.

Which boundary should Omega adopt?
