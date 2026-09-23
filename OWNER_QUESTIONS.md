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
under [the current policy](AGENTS.md#psi-implementation-and-deferred-human-audit).
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

Proposed solutions below are recommendations, not owner rulings. Where a
contract already answers a question, the entry says so explicitly.

### Q1 - How should the compiler bound aggregate proof work?

Named decision: `compile-time-proof-work-ceiling`.

**Context:** [Proof outcomes](wiki/spec/proofs/publication.md#outcomes) already
allow named resource/search exhaustion as `Incomplete`, not `Reject`.
[Kernel budgets](wiki/spec/proofs/kernel_metatheory.md) are implementation
policy, not calculus or source-language limits.

**Problem:** Individually bounded proof calls can collectively take impractical
time. **C2L-PROOF-SEARCH-BLOWUP-CONTAINMENT** identifies repeated whole-module
reconstruction and oversized certificates; **RC-PCC-REPLAY** depends on the repair.
The previous question incorrectly treated any aggregate limit as a language change.

**Proposed solution:** Fix the redundant work, then bound aggregate work with
explicit accounting and report exhaustion as `Incomplete`. Follow the
[compiler request](wiki/spec/build/compiler_request.md) for its representation.
No owner permission is needed for the already-settled outcome distinction.

**Alternatives:**
- Use configurable budgets or cancellation while retaining the same incomplete
  outcome. This accommodates larger workloads but still needs defined accounting.
- Tempting but wrong: reject valid source, accept without required evidence, or
  add only a warning and claim execution is bounded. A cap also does not excuse
  the known algorithmic defect.

### Q2 - Does AArch64 need a separate semantic entry wrapper?

Named decision: `aarch64-semantic-wrapper-arrival-shape`.

**Context:** [Program entry](wiki/spec/build/entry_roots.md) follows target-authored
arrival plans. UEFI x64 passes copies of two Extents indirectly; the AArch64
plans pass four value words in `x0` through `x3`.

**Problem:** The existing wrapper recipe assumes x64 copies and addresses.
However, macOS/Linux AArch64 use `HostedApplication` with a hosted bridge, not
UEFI's two-visible-Extent schema. **NATIVE-WRAPPER-ENCODING-AARCH64** must establish
a real wrapper customer before adding an encoder.

**Proposed solution:** Keep the UEFI-specific wrapper on UEFI and implement
AArch64 semantic arrival through its hosted bridge. Verify both shipped profiles
against their authored plans; do not invent a profile to justify wrapper tests.

**Alternatives:**
- If an actual AArch64 profile needs this wrapper, forward the unchanged value
  registers, potentially by a tail branch. Add instruction-aware `imm26`
  relocation support where that route needs it.
- Tempting but wrong: pass pointers under the same value-register plan fingerprint,
  copy x64 shadow-space rules, or patch an AArch64 branch as a raw x86 displacement.

### Q3 - How should interpreted components execute checked inline assembly?

Named decision: `interpreted-inline-assembly`.

**Context:** [Embedding](wiki/spec/build/embedding.md) includes interpreted
Cathedral startup and replacement. [Checked assembly](wiki/spec/language/assembly.md)
remains legitimate source; moving all device operations behind new interfaces
has not been ratified.

**Problem:** The contracts do not select live hardware versus modeled target
state, or the correspondence evidence needed for port I/O, idle, and external
entry. Only the assembly-dependent part of **INTERPRETED-CATHEDRAL** is blocked.

**Proposed solution:** Admit a bounded set of instruction contracts against an
explicit target-state/environment model. Preserve authority, memory, registers,
flags, ordering, control, and faults; reject unsupported instructions explicitly.
Validate one real device sequence and one idle/external-entry sequence.

**Alternatives:**
- Bind checked alternate realizations to the same instruction contracts, with
  correspondence evidence. This can avoid emulating each instruction.
- Define a limited interpreter profile that rejects assembly-bearing components;
  honest scope, but it does not deliver assembly-dependent Cathedral startup.
- Tempting but wrong: replace hardware idle with interpreter pause, redefine
  instructions through provider names, or move OS policy into the compiler.

### Q4 - What portable identity should a root without a package declaration have?

Named decision: `unmanaged-root-package-identity`.

**Context:** [Package identity](wiki/spec/build/declarations.md) uses declared
name and source lineage. Neither it nor [package sources](wiki/spec/packages/sources.md)
defines an undeclared root's identity. [Domain collision rules](wiki/spec/language/domains.md)
assume distinct declaration owners already have identities.

**Problem:** An unpackaged root and injected standard-library declarations can
both carry `package_identity: None`, collapsing separate owners onto keys such
as `[u8; N]::Utf8`. **MODULE-NAMESPACE-RESOLUTION** must not erase independent
collision checking to work around this.

**Proposed solution:** Require an explicit portable owner key when an unpackaged
root needs exported nominal identity; a package declaration supplies the existing
route. Keep ordinary local compilation separate from portable publication.

**Alternatives:**
- Derive identity from canonical content. This avoids an authored key but changes
  owner identity on edits and needs exact canonicalization and dependency rules.
- Keep such declarations forbidden without a package. Simpler, but less convenient.
- Tempting but wrong: use host paths or import order, exempt toolchain declarations,
  or assume distinct owner identities make competing visible names unambiguous.

### Q5 - Should a bare case name resolve as a value?

Named decision: `bare-case-value-names`.

**Context:** [Data literals](wiki/spec/language/data_and_literals.md) define
`Light::On`. [Name resolution](wiki/spec/language/modules.md) does not give bare
case names a lookup step; [operator-family inference](wiki/spec/language/expressions.md#operator-families)
does not establish a general case-name rule.

**Problem:** `arithmetic/bare_name_scopes` expects `let signal: Light = On;`,
but `exact_case_reference_owner` requires qualification. A fixture's expectation
does not establish a language requirement.

**Proposed solution:** Require `Light::On` unless a concrete customer justifies
bare-case inference. Correct the fixture rather than inventing lookup semantics
solely to make it pass.

**Alternatives:**
- Resolve through the expected carrier after lexical bindings. Concise, but
  unavailable without an expected type and relevant to proof narrowing/equality.
- Expose cases with their visible carrier, using ordinary ambiguity rejection.
  Works without an expected type but brings more names into scope.
- Tempting but wrong: choose the first matching case by import order or silently
  prefer a case over a local binding.

### Q6 - Does normal completion consume an owned receiver, including an affine one?

Named decision: `owned-self-receiver-implicit-retirement`.

**Context:** [Terminal ownership](wiki/spec/terminal-psi/ownership.md) requires
one accounted disposition: transfer, explicit consumption, eligible cleanup, or
validated affine discard. It does not name implicit receiver completion separately.

**Problem:** `consume_terminal_self_receiver` removes every owned receiver before
cleanup validation, regardless of multiplicity. An authored affine `DiscardRoot`
then fails with `ScalarReturnAffineDiscardsMismatch`. Two interpreter tests pin
discard ordering after edge charge; **OWNED-SELF-RECEIVER-AFFINE-DISCARD** owns the repair.

**Proposed solution:** Restrict completion-based receiver consumption to linear
receivers and explicitly account for it in the contract. Affine receivers keep
their charged cleanup/discard route; preserve the ordering tests.

**Alternatives:**
- Require an existing explicit disposition for every owned receiver. This avoids
  a special completion rule but may require producer changes for linear methods.
- Define completion-based consumption for both multiplicities, including its
  charge and cleanup semantics, then update the tests to that ratified rule.
- Tempting but wrong: silently remove an owned place or repin the failing tests.
  Passing tests alone would not explain where the ownership obligation went.

Settled mathematical binding and proof rules live in the
[mathematical source contract](wiki/spec/proofs/mathematical_bindings.md) and
[foundation](wiki/spec/proofs/foundation.md). Their implementation and required
proofs remain on `TASKS.md`; genuinely new semantic or trust choices belong here.
