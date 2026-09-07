# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
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

Last pruned: 2026-09-05.

## Q1 — Package acceptance at native build

**Context.** `cli_mvp` now passes checked package review, but native compilation
requires `--package-root-policy`. Install/update already record package acceptance
in `omega.lock`. The [build model](wiki/design_briefs/build_and_package_model.md)
says unchanged accepted trust must not require recurring approval, yet its native
root-policy section requires a separate file whenever fresh initial blockers exist.

**Problem.** Native preparation preserves accepted source pins but discards the
accepted policy. Both the native coordinator and its admission consumer compare
the freshly checked closure against an empty admission baseline. The additional
file means “accept these current package findings,” not proof of an audit or of
their claims. Its fingerprints include the whole candidate source closure, not
just changed capabilities. The shipped CLI consumes this record but cannot author
it: install/update use a different review format. The Rust `root_policy` example
only supports unlocked UEFI x86-64 projects, not this hosted workflow.

At `09bab8091c` on macOS, a disposable one-claim application using the existing
[`compile_project.rs` test shape](omega-rust/omega/packages/manager/src/operations/compile_project.rs)
reproduces the mismatch with the actual CLI and the `linux_x86_64` source profile:

1. `omega update --project <fixture> --target linux_x86_64` exits 3.
2. Accept the one boundary-guarantee finding in the generated review document;
   `omega update --resume --project <fixture>` exits 0 and publishes the lock.
3. `omega audit packages --project <fixture> --target linux_x86_64` exits 0,
   reporting `requires-review false` and `accepted-policy equal-to-fresh`.
4. `omega --target linux_x86_64 <fixture>/main.omg` still exits 1 with
   `fresh package review has blocking rows but no explicit --package-root-policy`.

**Proposal.** Use one package acceptance workflow. Recheck the actual sources and
target, reconcile fresh findings with accepted lock policy, and reuse matching
project decisions. Missing acceptance or new required findings use the existing
review flow. Retire the second user-authored package-approval file instead of
extending its UI. The lock remains trusted project intent, never proof.

Keep genuine compiler/native obligations separate: reject unproved contracts,
retain exact source/provider identities and artifact verification, and preserve
the independent receiving permission policy. Fresh permission rows must still
match retained production and selected mechanisms. Mapping accepted policy to
these reconstructed rows must be checked; neither a lock nor blanket approval
may manufacture native authority.

**Alternates.** A distinct native approval is justified only for a named decision
absent from package acceptance and existing compiler/receiving policy; give that
decision to its actual owner. Keeping a second acceptance codec simply because
the current implementation requires it is not a justification. Automatically
accepting every reconstructed row or removing native verification is also wrong.

**Decision requested:** consolidate package acceptance as proposed, or identify
the additional user decision the native package-policy file must represent?
Native approval workflow changes are **OWNER-BLOCKED** pending this answer;
independent writer lowering is still engineering work.
