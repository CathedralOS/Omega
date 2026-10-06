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

**Measured 2026-10-05: a selective re-record is landable and sanctioned, but
one lane is held by its own permission denial and needs an owner grant.** An
earlier revision
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

What still needs the owner, and is the only live blocker here: the lane that
completed `psi: carry a record's linear member claims on its result`
(`fef955e6d01`, rebased, clean tree, verify receipt `exit_code: 0`) asks to
re-record the 5 entries for the files it authored -- `semantics/src/lib.rs`,
`validation/error.rs`, `validation/frontier.rs`,
`validation/frontier/operations.rs`, `validation/record.rs` -- leaving the 40
foreign entries untouched. Its earlier denial stated that it "applies to the
outcome, not only this exact command", naming "running the same command in
smaller pieces" and acting on flagged items separately, so by the denial's own
terms the narrow hand edit is covered and the lane declined to attempt it. That
refusal was correct: a coordinator's reasoning is not consent, and only the
permission decision can lift itself.

So two lanes performed the same sanctioned action with different outcomes --
`b474414158` landed its six, and this one may not touch its five. That
asymmetry, not a tooling gap, is what holds finished work.

Staleness was 47 when this question was filed, 40 at `b0a48cde065`, and 45 at
the blocked lane's own HEAD (5 its own, 40 foreign).

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

### Q4 - How does a concrete reference erase to `dyn` at a call-argument position?

Named decision: `dynamic-erasure-at-argument-position`.

**Customer:** `pass/traits/runtime_dyn_two_impl_dispatch_exit` calls
`Main::dispatch(&mut main.c)` where the parameter is `&mut dyn Shape` and the
argument is a concrete `&mut Circle`. This is an **implicit erasure at an
argument position**, with no `let erased: &mut dyn Shape = ... as ...;`
statement anywhere.

**Existing rules:** `collect_dynamic_conformance_selections`
(`checking/src/declarations/traits/dynamic.rs`) only visits
`StatementNode::LocalData` carrying a dynamic coercion cast. So for this
program **no `DynamicConformanceSelection` is recorded at all** and no dynamic
dispatch plan is possible. `validate_dynamic_call_arguments` does type-check
the erasure -- it confirms exactly one closed conformance for `Circle` -- and
records nothing from it. The refusal therefore surfaces far downstream, as
`base_type_identity` returning `None` for a `TypeReferenceNode::DynamicTrait`
in `calls/structural_arguments.rs`. **Do not widen `base_type_identity`:** a
`&mut dyn` argument is a two-word descriptor, not a structural operand, so its
having no structural-argument identity is correct.

**Decision needed, in two parts:**

1. Is a coercion form admitted at a call-argument position, and if so does the
   argument's erasure record a conformance selection the way a local coercion
   does?
2. How is the conformance selected **per call site**? This fixture calls one
   `Main::dispatch` from two sites that select different conformances, while
   `CheckedDynamicDispatchPlans::for_caller` admits exactly one plan per caller
   (`let [plan] = ... else { "duplicated" }`). The fixture's own comment calls
   this call-site monomorphization.

This is recorded as a question rather than implemented because part 2 changes
what a dispatch plan is keyed by, which is a language-level commitment and not
an implementation detail. `pass/traits/runtime_local_named_dyn_devirtualized_exit`
is the sibling fixture that does author a local coercion, is a **different**
defect, and is tracked on DYN-CONTINUATION-ROOT-REJOIN.

### Q5 - Is `&Self` inside a conformance body rejected everywhere?

Named decision: `conformance-body-self-spelling`.

**Customer:** `pass/proofs/named_witness_static_trait_plural_compile` declared
`machine produce(subject: &Self)` in a conformance body, left by the mechanical
self-retirement respell at `553022e1a1a`, whose own message records that the
fixture stayed red on static conformance dispatch. It was respelled to the
concrete type to close that row.

**The hazard, measured:** `&Self` in a conformance body is **accepted** on the
ordinary route and **silently mis-binds** through specialized static requirement
dispatch, where the substitution binds only the template's declared type
parameters. `pass/traits/runtime_local_named_dyn_devirtualized_exit` has
`Primary: Item satisfies Shape { machine code(subject: &Self) -> i32 }` and gets
past typing into lowering -- its recorded red is a unit-plan omission, not a type
error. So one route accepts a retired spelling and the other quietly binds it
wrongly.

**Existing rules:** `wiki/spec/language/conformances.md` settles
`trait-requirement-subject-binding` (2026-10-04): "A conformance has no data
subject, carrier type, implicit first type parameter, or privileged implementing
type. `Self` has no binding and is retired together with receiver `self`...
remove the old paths rather than add compatibility inference, aliases or modes."
Read literally, that rejects the spelling **everywhere**.

**Decision needed:** confirm that reading before anyone repairs the dispatch
path, because teaching the specializer a `Self`-to-subject binding would
re-entrench the construct the decision retires. If rejection is confirmed, the
work is a diagnostic plus a migration of the accepting fixtures and of
`source/library/core/layout.omg`'s three `&Self` requirements together with the
seven pass fixtures that match them -- `NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION`
scope, not a local fix. This is recorded rather than done because the failing
site invites exactly the local fix the spec forbids.

### Q6 - After receiver retirement, what carves out an unselected requirement call?

Named decision: `unselected-requirement-carve-out`.

**Customer:** `pass/core/interrupt_obligations_surface`, plus
`pass/memory/address_translation_canary`. Both reject because
`reject_unselected_direct_requirement_calls` demands a selected provider for a
public boundary requirement, and `63bca13481d` widened its gate by deleting the
clause that required the requirement to be **receiver-free**. The field that
clause read, `StateParameter::is_self`, no longer exists, so the carve-out
cannot be restored -- only replaced.

**Decision needed:** must a compilation select a provider for a public boundary
requirement that only an **unreachable** declaration calls?

**Why this is not an engineering call.** The mechanical translation of "has a
receiver" is "the requirement's first parameter is its owner data type", which
is the retired `Self`/receiver notion that `trait-requirement-subject-binding`
forbids reintroducing. That leaves **root reachability**, and the evidence is
split:

- For it: `wiki/spec/terminal-psi/boundary_calls.md:210` reconstructs
  "**reachable** fixed boundary rows ... from **executable operations**", :259
  says "Selected but unreachable imports retain identity without requiring
  execution inputs", and `wiki/spec/build/provider_selection.md:204` requires
  reconstructing "every **reachable** closed demand". The canonical
  reachability-keyed form of this very rule already exists in Omega stage 00,
  at `provider_admission/terminal_authority_review.rs:44`. So selected-dispatch
  holds a second copy of a rule the canonical site keys on reachability.
- Against it: the same file records "Select all Product roots, not a guessed
  reachable subset or callee names" (`boundary_dispatch.rs:82`).
- And `50da9413d4e`'s own message accepted this rejection, saying the fixture
  "needs a `satisfies` provider and a build selection this fixture does not
  declare", re-pinning its golden "rather than forced green".

**Measured cost of the reachability answer:** the gate is load-bearing.
Disabling its one call site greens both customers and makes
`fail/providers/boundary_requirement_direct_call_unselected` stop rejecting for
its authored reason. That fixture has no `build.omg`, so under reachability
keying nothing in it is reachable either -- closing this needs the rule **and**
a real entry selection for that fail fixture.

### Q7 - Which lane owns an operand-level float landing's qualification evidence?

Named decision: `operand-level-float-landing-custody`.

**Customer:** any comparison between a computed f32/f64 value and an unsuffixed
float literal, which does not lower. Two-line repro:
`machine code() -> bool { F64::square_root(64.0f64) == 8.0 }`. The owner's own
`pass/float/build_runtime_semantics_twins_*` fixtures use this spelling.

**Existing rules, and why they conflict.** `semantic_casts::requires_custody` is
recursive over the whole expression tree, so it is true when *any descendant*
needs custody. The retention filter at
`values/scalar/expression_plans.rs:185-193` reads that as "the computation graph
owns this coordinate" and drops the pure plan. But the computation graph claims
the coordinate only when the custody cast is the **root**
(`computations.rs:1180`), and its pure fallback at `:1222` is gated behind
`!requires_custody`. For a custody cast appearing as a mere **operand**, both
sides decline and the coordinate ends up empty.

**Decision needed:** which side owns an operand-level anonymous-rational float
landing? Three shapes are defensible and the choice is a semantics-ownership
call, not an engineering one:

1. Move the decision after computations are built -- retain a pure plan whose
   coordinate the computation graph did not claim.
2. Make the computation graph own what `requires_custody` actually says it owns,
   relaxing the `!requires_custody` gate on the pure-lowering arm when the
   custody cast is an operand that arm can lower in place.
3. Make the checker not plant an operand-level landing cast needing custody when
   the comparison's other side already fixes the carrier.

**Why it is recorded rather than done:** build order forecloses (1) as a local
change, since `values.scalar_expressions` is built at `facts.rs:166` and
`values.scalar_computations` at `:227` consumes it, so the drop cannot ask
whether the coordinate was claimed. And (2) and (3) each look like "recognize
one more shape" from the other's side, which the hard rule forbids choosing
arbitrarily. Once ruled, the repair is small and the two-line program pins it
either way.

### Q8 - Who supplies the realization of a bare-metal boundary requirement?

Named decision: `bare-metal-requirement-realization`.

**Customers:** `pass/memory/interrupt_table_canary`
(`InterruptAcknowledgement::complete`, `source/library/core/interrupt.omg:57`,
`reaches PortIo`, content = LAPIC end-of-interrupt write) and
`pass/memory/secondary_processor_canary` (`StartupEnvelope::confirm`, the
fixture's own `cathedral/processor_startup.omg`, `reaches MachineControl`,
content = application-processor startup handshake). Both are top-level public
`boundary requirement`s over **opaque linear boundary data**. Selection and
admission both succeed; only realization is missing. These are **one** question,
not two -- the earlier guess that they split by declaration site was wrong.

**All four routes are closed, each measured rather than inferred:**

1. **Compiler intrinsic** -- needs a new `CompilerIntrinsicExecutionIdentity`
   variant plus a `MachineSemanticKind`, per-target realizations, catalog and
   encoding opcodes, a legalized scalar graph and review-evidence capture. That
   makes the interrupt controller and the AP bring-up protocol **compiler-owned
   semantics**, against AGENTS.md's "Cathedral (the downstream OS) owns OS
   structures".
2. **External satisfier leaf** -- `via` admits only `Syscall(n)`,
   `DllImport{..}`, `VtableField(f)`, `TableFunction(f)`
   (`01_tokens-to-syntax-trees/src/contracts/conformance/external_binding.rs`).
   None expresses an MMIO or port write, and
   `pass/providers/external_boundary_requirement_via_exit` is `DllImport` over
   libc, which bare metal does not have.
3. **Authored body** -- refused by measurement: replacing the binding with a
   body gives ``linear value `envelope` reaches scope exit without being
   consumed or transferred``. A package cannot discharge a linear
   `boundary data` whose only declared discharge route is the requirement it is
   satisfying. Inline asm reaches `PortIo`/`MachineControl` but does not help
   the discharge, and `MachineControl`/`PortIo` are machine-less reach labels
   with no callable operation.
4. **Installation-route propagation** (`InstalledInterruptCompletionRoute` in
   `external-roots`) -- would make both provider declarations wrong and
   removable. `wiki/spec/resources/authority.md#completion-authority-and-implementation-selection`
   says only that "a public boundary requirement still needs a selected,
   admitted implementation"; it is **silent on what realizes one**.

**Decision needed:** (1) a fifth `ForeignBinding` case for a hardware-register
write, (2) a language discharge for a linear boundary obligation inside its
satisfying body, (3) installation-route propagation that removes these two
provider declarations, or (4) retiring both fixtures as not-yet-authorable.

**Do not resolve it by making the top-level-requirement lane answer
`Unsupported` the way the trait lane does.** Measured: that moves both fixtures
off this diagnostic and plausibly to `built` while neither program can run. It
is an admission-policy change disguised as a cleanup.
