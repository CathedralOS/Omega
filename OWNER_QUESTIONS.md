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

Psi representation and encoding design is delegated; human review is deferred
under [the current policy](omega-rust/pipeline.md#psi-implementation-and-deferred-human-audit).
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

Bootstrap design exploration is delegated engineering work, not an owner
question merely because it compares a different implementation or language
facility. Apply [whole-chain minimization](bootstrap/MINIMIZATION.md): identify
the next compiler/checker customer and compare complete audit cost. Experiments
remain non-authoritative; changes to the trust boundary or required assurances
must be surfaced before relying on them.

Name the contracts checked and found silent. A question that cannot cite them
has not shown its decision is open. Specifications are organized by mechanism
while questions arrive by symptom, so the answering clause is routinely in a
document whose subject is not the question's: a containment rule sits with
activation inputs rather than with the vocabulary it confines, and an
instantiation rule sits with contract import rather than with the value class
it instantiates. A reviewer verifies those citations before the framing.

## Open questions

Proposed solutions below are recommendations, not owner rulings. Remove an
entry when the contract answers it; retain any unfinished implementation on
its owning task.

### Q1 - What carries a portable service's per-family raw binding?

Named decision: `portable-service-per-family-binding-carrier`.

**Customer:** one portable filesystem API over per-OS raw services, which
[platform services](wiki/spec/build/platform_services.md) makes the shape for
clocks, consoles and networking too. **FILESYSTEM-PLATFORM-SERVICES** rules
that `FilesystemHost` splits into `PosixFilesystemHost` and
`WindowsFilesystemHost`, each fully covered by the provider bound per target.
The raw-seam half of that needs no decision: a program holding
`Binding<FilesystemHost>` directly can hold `Binding<PosixFilesystemHost>`
instead, and eleven red `tests/omega/pass/filesystem` fixtures are exactly
that shape. The portable half has no spelling, and it is where the item's own
customer lives — `samples/cli/systems/file_journal` and four wrapper
fixtures declare the portable `Filesystem`, not a raw binding.

**Existing rules:** `source/library/std/filesystem.omg:377` declares one
portable `pub data Filesystem { host: Binding<FilesystemHost>; … }`. After the
split that field's requirement differs per target, and none of the three ways
to say so exists.
[Machines](wiki/spec/language/machines.md#target-scoped-families) scopes
families to machines only: "A machine declaration prefixed with a target name
is one body of a family keyed by target under one path", so `data` cannot be
target-scoped. Carrying one field per family does not work either, because
`collect_service_fields`
(`omega-rust/omega/compilation/realization/selected-dispatch/src/service_custody/root.rs:408`)
walks every `Binding<R>` field of the entry receiver recursively and demands a
selected Fused provider for each one regardless of reach (the "requires a
selected Fused provider for boundary" rejection at `root.rs:160`) — a
`Filesystem` holding both families would demand a POSIX provider on Windows.
A generic `Filesystem<H>` with `host: Binding<H>` types correctly, since
`Binding<R>` is already generic (`source/library/core/binding.omg`), but there
is no target-scoped type alias to keep the family out of the portable API, and
the spec requires that: "Raw handle types are per family (`PosixDescriptor`,
`WindowsHandle`) and never appear in the portable API".

**Decision needed:** three candidate answers, in the order I would pick them.

1. **Keep one granted portable boundary and demote the families to requirement
   namespaces.** `Filesystem.host` stays `Binding<FilesystemHost>`, where
   `FilesystemHost` holds only the portable operations; each family's
   OS-specific primitives become top-level boundary requirements under a
   per-family provider data type, reached under the portable grant, exactly as
   the macOS `TimeHost` provider reaches `TimeHostNativeProvider::poll`
   (`source/library/std/targets/macos_arm64/time_host_impl.omg`).
   [Provider selection](wiki/spec/build/provider_selection.md) already admits
   that slot kind: "Slot resolves to one exact boundary trait,
   package-qualified same-path boundary-operator family, or top-level boundary
   requirement." Each requirement is then its own slot, so coverage is per
   operation and a target selects only the family it has — no sentinel rows,
   no per-family carrier, and no language change. It costs the ruling's exact
   words: the portable wrapper would not be bound "over `PosixFilesystemHost`",
   and a raw-seam program would name a family requirement rather than hold
   `Binding<PosixFilesystemHost>`.
2. **Demand only reached `Binding` fields**, so `Filesystem` may carry one
   field per family and each target demands the one its selected bodies reach.
   This weakens a custody rule that currently holds unconditionally and makes
   the demand reach-dependent — a larger change to admission than to the
   library.
3. **Add target-scoped `data`, or a target-scoped type alias for a generic
   `Filesystem<H>`.** The literal reading of the ruling and the cleanest
   library; the only one needing new language surface.

All three change what `FilesystemHost` means relative to the spec's three
layers, which is why none of them is the implementer's call: (1) makes
`FilesystemHost` the portable raw seam rather than a per-OS one, (2) changes a
custody rule, (3) adds language surface.

**Implementation customer:** `tests/omega/pass/filesystem/{dir_walk_wrappers_exit,windows_wrapper_results_exit,wrapper_lock_metadata_exit,wrapper_open_with_exit}`
and `samples/cli/systems/file_journal`, all currently rejecting with
"selected ProgramEntry Binding field `Main::fs::host` requires a selected
Fused provider for boundary `FilesystemHost`" or its reachable-requirement
twin. The verified member classification, call-site confinement and the ten
Rust consumers the split moves stay on **FILESYSTEM-PLATFORM-SERVICES** in
`TASKS.md`; that work is ordinary implementation once this is answered.

### Q2 - May the trusted-surface ledger be re-recorded on a proof of no semantic change?

Named decision: `trusted-surface-digest-reconciliation`.

**Customer:** `python3 tools/trusted_surface_digests.py` reports **47 stale
digests** on a clean `main`, and 19 `TEST_ONLY_SOURCES` rows name files that no
longer exist. Nobody may refresh a digest for a file they did not author, so the
drift has no owner and has sat unreconciled.

**Existing rules:** `sites.rs` pins a SHA-256 for every bound verifier
implementation file, and terminal-verifier's
`recorded_digests_match_the_working_tree` enforced it. `tools/landing.py`'s
`stale_trusted_sites` guard refuses a candidate that changes a pinned file
without re-recording, but by design it inspects only the candidate's own
changes, so drift already on main never blocks anyone.

**What is measured, so the decision costs no re-reading:** all 47 files were
last touched by one commit, `55a502d211b`, which also deleted that enforcing
test. Every one of the 47 diffs is a **pure deletion** with zero non-blank added
lines, and the 49 removed regions are all `#[cfg(test)]`-gated -- 48 beginning
at the attribute, one at the doc comment above a
`#[cfg(test)] pub(crate) fn proposition_observes_write`. Twelve of the 47
removed only `#[cfg(test)]` + `mod <name>;`, their tests having lived in the
sibling file the same commit deleted. **No verification judgment in any of the
47 changed**, so revalidating the entries that cite them is satisfied by that
fact rather than by re-reading 47 rules.

**Decision needed:** may the ledger be re-recorded on that proof, and by whom?
Three edits close it and must land together, or the restored test is red:
delete the 19 dead `TEST_ONLY_SOURCES` rows; run
`tools/trusted_surface_digests.py --write` for the 47; and restore
`recorded_digests_match_the_working_tree` as a `[[test]]` under
`terminal-psi/verification` (`autotests = false`). That test calls
`check_source_coverage(&repository_root())` and hashes files -- it reads no
Omega source, so the test-cut policy keeps it, and it was collateral of a
43-crate sweep rather than a decision about this gate.

Until the third edit lands the ledger has no enforcement outside the landing
queue, and the queue cannot see drift already on main, so a re-record alone
buys one clean report before the next sweep silently reopens the hole. This is
recorded rather than done because `sites.rs` is a trust record: an automated
edit to it was refused as audit tampering, which is the correct outcome.

**Measured 2026-10-05: a selective re-record is landable, so the drift blocks
nobody, and the historical 40 remain the whole question.** An earlier revision
of this entry claimed the rule and the landing gate contradict each other for
any lane editing a pinned file. That is **refuted by demonstration**:
`b474414158572c522ad113f56bd5328a1eed333e` changed six pinned verifier files
and carried `sites.rs` with exactly six digest updates, one per file it
authored. Verified independently at `b0a48cde065`: the ledger still reports
**40** stale entries, unchanged, and none of that lane's six files appear in
the stale list. A lane refreshing only its own authored entries is a different
act from the blanket `--write`, is what the trust rule already permits, and
needs no new capability.

A path-scoped mode on `trusted_surface_digests.py` therefore remains worth
having as a convenience -- it would make the narrow act the easy one rather
than the careful one -- but it is not a precondition for anyone's landing and
is not a fourth required edit. The three edits above still stand on their own.

The one measurement that did change: staleness was 47 when this question was
filed and is 40 at `b0a48cde065`.

### Q3 - What does a locally declared boundary schema mean against the target's own?

Named decision: `entry-slot-schema-resolved-by-bare-name`.

**Customer:** `tests/omega/fail/build/uefi_program_entry_wrong_calling_policy`
is the FAIL canary for the two-surface program entry's physical
calling-convention check -- `uefi_x86_64::ProgramEntry` declares MicrosoftX64,
the package's `WrongUefiPolicy` plans SystemVAMD64, and
`build-evaluation/src/admission/selection.rs` must refuse the mismatch. It
never reaches that check. It is one of only two fixtures in the corpus
rejecting for the wrong reason, a condition `tools/corpus_gate.py` now names on
every run.

**Existing rules:** the entry slot resolves its schema **by bare name** over
every loaded boundary trait, and bundled std declares an application schema for
all six targets that name one (`UefiApplication`, `MacosApplication`,
`LinuxX86_64Application`, `LinuxArm64Application`, `MacosX64Application`,
`WindowsX86_64Application`). A package declaring its own therefore always makes
two, and the slot stops at "requires exactly one loaded `UefiApplication`
boundary schema, but found 2". Measured: the duplicate does not come from the
fixture's imports -- dropping `use omega_language_std::calling;` leaves the
count at 2, because the library's target entry loads with the target rather
than with the import. And the policy cannot be varied from the library surface:
`CallingPolicy` lives only in `std/calling.omg` and the library schema is bound
to `Calling<UefiX86_64>`, which plans MicrosoftX64 on every route. So a wrong
policy requires a local schema, and a local schema is what stops you.

**Decision needed:** does a locally declared schema shadow the target's own or
conflict with it, and should the slot resolve by owning package or requirement
identity rather than by bare name? Resolving by identity gives this rule a
source-level customer; declaring it a conflict means the rule can only ever be
exercised from a package that owns its target. Do not repin the fixture onto
the duplicate -- that would cement one answer. The rule has no Rust-side
customer either, since `BoundaryCallingPlanRealization` seals
`exact_boundary_entry_plan` and `materialized_signature` to
`provider-planning`, so a `build-evaluation` unit test cannot construct one.
Until this is answered the physical calling-convention check is unexercised by
any program or test.
