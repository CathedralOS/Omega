# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the specification and language guide; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

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

Apply the same test to security machinery. Omega owns only claims it can
enforce at its actual compiler, package, and artifact boundaries. A proposal
that merely restates host operating-system, credential, transport, or operator
trust must be deleted or delegated to that owner rather than dressed as an
Omega guarantee. If the boundary or enforceable claim is genuinely ambiguous,
promote that narrow ambiguity here before adding machinery.

A compelling proposal for a new bootstrap language, replacement rung, or
alternate dialect belongs here before implementation, even as an experiment.
Apply the [bootstrap scope checkpoint](AGENTS.md#scope-checkpoints): identify
the concrete compiler customer or required proof obligation, compare existing
languages and simpler refactors, and account for total audit cost and machinery
displaced. Await an owner decision; a task or prototype is not approval.

## Q1 — Source-visible bounded byte output (`BOUNDED-BYTE-OUTPUT-SURFACE`)

The unchanged `cli_mvp` customer needs `Console::read_line(&mut self.pause)`
to replace a bounded byte field, including its live length. The execution board
calls for a checked Omega adapter over `read_byte`, but its current
`out_line: &mut [u8]` parameter exposes no operation that can implement that
replacement.

Audit: [Slice](source/library/core/slice.omg) exposes length, bounded indexing,
and bounded subviews; it cannot observe spare capacity or change the owner's
live length. [FixedVec](source/library/core/fixed_vec.omg) and
[Vec](source/library/core/vec.omg) have owner-specific mutation contracts, not
operations on an arbitrary slice. Existing callers use the current slice
signature. [Boundary presentation](wiki/spec/terminal-psi/boundary_calls.md)
retains exact capacity and writeback, and the interpreter's external callback
can replace the field, but this supplies no source operation to a checked body.
The settled [whole-field store](wiki/spec/terminal-psi/structural_access.md)
requires an explicit destination field and immutable source bytes, not just the
provider's slice parameter.

Should `read_line` remain an external bounded-replacement operation, or should
checked composition use an explicit source-visible bounded-output contract?
For checked composition, which owner/window and live-length update operations
should the API expose, and what is the outcome when the input exceeds capacity?
Ordinary data with explicit storage and length is an alternative to extending
Slice semantics, but changing the current parameter/callers requires a settled
API direction. Raw fixed-array length and ordinary slice bounds must not change
implicitly; encoding qualification still follows ordinary checked contracts.

Until answered, defer the checked line adapter and any native mutable descriptor
introduced only to support it. Native byte leaves and independently motivated
bounded-field operations remain actionable.
