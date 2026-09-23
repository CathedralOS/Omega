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

### Q1 - Must a mutable method preserve its receiver's declared field domains?

Named decision: `mutable-self-receiver-declared-field-rows`.

**Context:** [Default domains](wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization)
must hold at calls, but newly zeroed machine storage may contain inaccessible
fields. The contract does not explicitly join these rules for `&mut self`.

**Problem:** After `player.update()`, the caller can lose a previously proved
bound on `player.health`. Ordinary `&mut` parameters preserve their declared
field domains; receivers currently preserve only zero-satisfiable or framed-off
facts. This blocks receiver handback in **NOMINAL-FIELD-FLOW**.

**Proposed solution:** Treat `&mut self` like other mutable referents: callers
prove declared field domains, callees assume them and re-prove them on return.
Keep partially initialized construction distinct from readable method entry.

**Alternatives:**
- Keep receiver defaults internal; require explicit `requires`/`ensures` for
  non-zero-satisfiable domains. This preserves construction flexibility but
  makes receiver contracts different from ordinary parameters.
- Tempting but wrong: retain the caller's facts without checking the callee's
  writes and return obligations. That preserves unproved invariants.

### Q2 - How must Delta report exhaustion of its evaluator's pair storage?

Named decision: `delta-compiler-pair-arena-profile`.

**Context:** [Delta's request contract](bootstrap/3_delta/LANGUAGE.md) requires
compiler-owned outcomes. [Gamma status 252](bootstrap/2_gamma/EVALUATOR_PROFILE.md)
reports evaluator heap failure with no compiler output.

**Problem:** An admitted input exhausted the former 40,265,318-pair arena.
The selected arena is now 3,422,453,760 pairs; the
[allocation study](bootstrap/3_delta/implementation/boundary/execution_storage.md#whole-producer-pair-study-measured)
projects 417,063,339, but that projection is not a proof covering every admitted
input. **DELTA-COMPILER** still needs containment or a compiler-owned failure.

**Proposed solution:** Complete containment for the selected arena first.
Provisioning is engineering work under [minimization](bootstrap/MINIMIZATION.md),
not a new language decision. Escalate only a required change to the observation
contract if containment cannot be established.

**Alternatives:**
- Add an allocation ledger and a specified `DCOUT` resource outcome below the
  evaluator limit. This gives explicit failure but adds compiler/checker cost.
- Ratify raw status 252 as a request outcome. This changes the current guarantee
  and needs an explicit owner decision.
- Tempting but wrong: call measured headroom a proof, or invent a `DCOUT` heap
  code without changing its contract.

### Q3 - How should Terminal Psi identify crashes from trapping arithmetic?

Named decision: `terminal-operation-level-trap-crash-site`.

**Context:** [Trapping operations](wiki/spec/terminal-psi/structural_predicates.md)
already own crash sites. The versioned
[observation profile](wiki/spec/terminal-psi/observations.md) encodes only
terminator-edge crashes and `BoundaryCall` crashes.

**Problem:** A trapping addition or conversion has neither an edge coordinate
nor a boundary identity. **ARITHMETIC-POLICY-REALIZATION** needs an operation
coordinate that the verifier, interpreter, and native realization agree on.

**Proposed solution:** Add a non-boundary operation-crash group keyed by machine,
block, operation, and cause. Specify ordering and tags, version the profile,
and update independent reconstruction together.

**Alternatives:**
- Unify edge and operation sites under a tagged coordinate, reducing separate
  groups but changing every ordinary crash-site reader.
- Generalize boundary-crash rows to all operations, with boundary identity only
  where applicable. This reuses ordering but broadens that group's meaning.
- Tempting but wrong: fabricate an edge or boundary identity, or silently change
  the existing wire schema. Neither preserves the published observation contract.

### Q4 - How should the compiler bound aggregate proof work?

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

### Q5 - Does AArch64 need a separate semantic entry wrapper?

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

### Q6 - How should interpreted components execute checked inline assembly?

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

### Q7 - Does Psi or Omega own post-handoff writer plans?

Named decision: `post-handoff-writer-ownership`.

**Context:** The [firewall](AGENTS.md#the-psiomega-ownership-firewall) gives Psi
target-neutral semantics and Omega realization and execution. Psi's `layout-plans`
currently derives, validates, and executes post-handoff writes; its consumers
are in Omega.

**Problem:** Execution belongs in Omega. The disputed part is whether writer-plan
derivation is portable layout semantics or realization policy. That classification
determines where the carriers and stored-integer validation belong.

**Proposed solution:** Keep genuinely target-neutral plan derivation and validation
in Psi; move execution, reusable-fragment ABI, and encoding to Omega. Use one
validation owner rather than copies in each consumer.

**Alternatives:**
- Move the entire writer program to Omega, leaving Psi at `MaterializationAction`.
  This groups the program with its consumers but requires a deliberate home for
  the shared write/fit rules.
- Tempting but wrong: rename consumer modules and call the ownership fixed, or
  weaken the firewall merely to legitimize the current placement.

### Q8 - Is passing a Binding by value a move or a copy?

Named decision: `service-carrier-argument-multiplicity`.

**Context:** Already settled: [component publication](wiki/spec/build/component_publication.md#bindings-and-era-entry)
defines `Binding<R>` as affine. Passing it by value moves it; ordinary argument
passing does not duplicate authority.

**Problem:** `capabilities/uses_caller_folder` passes `self.folder` by value from
borrowed receiver storage without replacement. The checker rejects that move;
the fixture expects acceptance. This is an implementation/fixture issue, not an
open multiplicity decision.

**Proposed solution:** Preserve affine custody and lend `&Binding<Folder>` when
the callee only needs access. Keep a negative control for an unrepaired move out
of borrowed storage, and close the stale escalation through the owning task.

**Alternatives:**
- Transfer ownership with valid replacement, or use protocol-authorized checked
  duplication where an actual second binding is required.
- Tempting but wrong: make Binding copyable only in argument position, or merely
  reclassify the pass fixture without preserving its legitimate borrowed-call use.

### Q9 - How should Terminal represent structural replacement, recast views, and fresh linear values?

Named decision: `terminal-vocabulary-for-unit-bodies`.

**Context:** [Structural access](wiki/spec/terminal-psi/structural_access.md) and
[ownership](wiki/spec/terminal-psi/ownership.md) govern independently checked
Terminal operations, not source-shape exceptions.

**Problem:** Unit-body lowering lacks three routes: replacing a non-vacated,
disposal-free structural field; forming a checked typed view into bytes; and
establishing custody for a fresh linear record. `StoreStructuralField` currently
repairs move windows, while `EstablishRecord` excludes linear claims. Customers
include case-field assignment, descriptor views, and owned receipts.

**Proposed solution:** Specify independently checked operations for replacement,
recast referents, and fresh root custody. Carry bounds, layout, borrow, and
claim-lifecycle evidence through lowering and execution. Missing implementation
alone needs no owner ruling; isolate any genuinely new semantic rule first.

**Alternatives:**
- Extend existing operations where their invariants genuinely cover the new
  behavior; use distinct operations where combining rules would obscure checking.
- Tempting but wrong: remove repair checks, fake a move window, trust a producer's
  claim assertion, or equate fresh linear custody with permission to mint authority.

### Q10 - What portable identity should a root without a package declaration have?

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

### Q11 - Should a bare case name resolve as a value?

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

### Q12 - Does normal completion consume an owned receiver, including an affine one?

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
