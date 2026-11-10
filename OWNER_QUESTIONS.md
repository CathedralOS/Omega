# Owner Questions

Only the owner answers, closes, removes or rewrites an entry here, including
one that an agent believes the specification already answers. Agents add
questions and may research existing ones, but they report their findings and
citations to the user and leave the entry, the specification and the boards
unchanged; they never edit the specification or a task to settle a question.

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

### Q1 - Should native check removal require kernel-checked evidence and fall back to guarded native code when an obligation lacks it, amending `psi-execution-admission`?

Decision name: `evidence-gated-check-removal`.

**Customer.** Today Omega 00 turns every `OperationObligationReference` in a
unit into an `AcceptedObligationFact` with a zero 32-byte
`proof_bundle_fingerprint`
(`00_terminal-psi-to-abstract-operations/src/optimization.rs`), and the
evidence census (`#2284`) counted `0` of `814` accepted operation obligations
carrying kernel-checked evidence. Omega 01 (`proof_check_elision.rs`) and Omega
03 (`selection_scalar_graph.rs`, `selection_views_and_bytes.rs`) then delete
`x / x` and `x % x` and select unguarded integer arithmetic, division, and
indexed byte and element loads whenever that fact is present. Any front-end
soundness bug (such as the dead-crash window skip pinned in `#2283`, or an
unverified contract-automation path) therefore strips runtime overflow,
division, and bounds checks from native code while the Terminal interpreter
still enforces them. During the lowered-form checker rebuild
([check on the lowered form](wiki/drafts/designs/check_on_lowered_form.md)),
[What each stage may trust](wiki/drafts/designs/stage_trust.md) proposed gating
runtime-check removal on kernel-checked evidence, which requires Zac's ruling
because it amends ratified specification policy.

**Gap.** Ratified decision `psi-execution-admission` (2026-11-07) in
[Psi product](wiki/spec/terminal-psi/product.md#publication-and-separate-consumption)
establishes three rules for portable artifacts and native lowering:

- For a trusted producer's exact artifact going to native code: *"Reuse checked
  judgments for optimization and lowering under the selected installation
  contracts."*
- For carried certificates in the portable envelope: *"Their presence grants no
  truth, execution authority or producer trust, and receipt does not require
  kernel replay"*, and *"The consumer does not rediscover source theorems or
  require transported derivations."*
- For an untrusted artifact going to native code: *"Refuse. This contract
  provides no guarded native lowering route."*

Rule 2 of [What each stage may trust](wiki/drafts/designs/stage_trust.md)
proposes instead that a fact may license unguarded native code only when it
carries evidence (or a sealed fingerprint of evidence) accepted by
`proof/admission`, and that a fact without it keeps its runtime check and crash
route in native code. Adding a wire field to `PSITERM\0` is delegated
implementation work under this file's policy and
[Canonical encoding](wiki/spec/terminal-psi/encoding.md#canonical-form) once a
policy is chosen (same-invocation compiles currently carry certificates in
memory beside `TerminalModule`, which are explicitly absent after a
`CanonicalTerminalArtifact::from_bytes` cache or disk read). The policy
question for Zac has two parts:

- **(a) Artifact contract across separate consumption:** Should `PSITERM\0`
  distinguish obligations backed by kernel-checked evidence from obligations
  accepted without kernel evidence across separate consumption, or does a
  host-authorized producer's established judgment remain uniform regardless of
  how the producer discharged it?
- **(b) Guarded native lowering fallback:** When an obligation lacks
  kernel-checked evidence, should Omega 01 and 03 retain runtime checks and
  crash routes (introducing a guarded native lowering route where
  `psi-execution-admission` currently defines none), continue reusing
  host-authorized producer judgments unguarded, or refuse native lowering?

**Options.**

- **Amend `psi-execution-admission` for both (a) and (b) (proposed in
  `stage_trust.md`):** Carry per-obligation kernel-evidence status or sealed
  fingerprints in `PSITERM\0`, require Omega 01 and 03 to check that status
  before eliding a runtime guard, and emit guarded native operations and crash
  routes whenever an obligation lacks kernel-checked evidence. Missing evidence
  then costs runtime checks rather than native safety.
- **Keep `psi-execution-admission` unchanged:** A host-authorized producer's
  artifact licenses unguarded native lowering for every obligation it marks
  checked without distinguishing kernel-evidenced obligations at the boundary
  or adding a guarded native lowering route; requiring kernel-checked evidence
  stays a Psi checker acceptance gate in
  [check on the lowered form](wiki/drafts/designs/check_on_lowered_form.md).
- **Adopt (a) in Psi without guarded native lowering (b):** Require Psi
  publication (once the lowered-form checker serves the language) to seal
  kernel-checked evidence for every discharged operation obligation and refuse
  to publish unproven ones, keeping native lowering unguarded-only for trusted
  artifacts.

### Q2 - How do carried derivations across specialization compose with fresh dependency checking?

Decision name: `specialization-certificate-reuse`.

**Customer.** Rebuilding the checker on the lowered form
([check on the lowered form](wiki/drafts/designs/check_on_lowered_form.md))
retires stage 05's source-text hash cache for dependency-body judgments
(`05_typed-trees-to-checked-trees/src/checking.rs`). Generic specializations
and consumer dependency checks need one settled rule for Zac on what proof work
may be carried forward versus repeated in each consumer.

**Gap.** Two ratified specification clauses address reuse on adjacent axes:

- [Generic declarations](wiki/spec/language/generics.md#closed-applications-and-canonical-identity):
  *"Root Build controls recorded proof-discovery effort. An unresolved
  obligation may need a larger search allowance or an explicit theorem, but an
  unequal key is not evidence of mathematical inequality. Carry checked
  derivations through specialization and published artifacts without
  re-searching under a consumer's smaller discovery budget. Ordinary formation,
  custody and trust obligations remain in force."*
- [Validation subjects](wiki/spec/build/validation_subjects.md#what-this-rule-does-not-do)
  (ratified 2026-11-08): *"No result is reused: a consumer checks every
  dependency declaration it needs afresh, and the full-package gate is a claim
  about the library, not evidence a consumer consumes in place of checking."*

**Options (including a proposed reading for Zac, not a settled answer).**

- **Carry certificates through specialization and re-verify them in the kernel
  per consumer (proposed reading):** Read `generics.md` as governing *proof
  discovery* (do not re-run proof search under a consumer's discovery budget
  when a generic template or published declaration already produced a
  certificate) and `validation_subjects.md` as governing *verdict reuse* (do
  not accept an earlier compile's pass verdict in place of checking a reached
  dependency declaration). Specialization substitutes concrete arguments into
  the carried certificate, and each consumer's fresh check runs
  `proof/admission` to re-verify that certificate without re-searching.
- **Re-prove every specialization and dependency declaration from scratch in
  every consumer:** Read `validation_subjects.md`'s *"No result is reused"* as
  forbidding carried certificates across consumer checks as well, running full
  proof search on each specialized body in every consumer and updating
  `generics.md`'s discovery-budget clause to match.
- **Accept checked template verdicts across admitted specializations without
  per-specialization certificate re-verification:** Treat checking a generic
  template against its declared bounds as sufficient for all admitted concrete
  selections within a consumer check, re-checking only instantiation bounds.

### Q3 - What source forms spell a brand, a stored theorem selection, an explicit choice between conflicting parent selections, and a computed record offered to Build?

Decision name: `operation-record-source-forms`.

**Customer.** `OPERATION-RECORD-IMPLEMENTATIONS` on [TASKS.md](TASKS.md) is a
human-set priority item, and `OWNED-ACTIVATIONS` is sequenced after it. Its
acceptance list in
[operation-record implementations](wiki/drafts/designs/operation_record_implementations.md#acceptance)
(ratified 2026-11-09 as documentation) names four behaviors that an author
must be able to write: *"a branded pair is two"*, *"a stored theorem selection
yields two"*, *"conflicting parents reject without an explicit choice"*, and *"A
computed record offered to Build is checked for slots, realization and
admission"*. The product requirement is the one the design states: a library
that reuses existing machines in an implementation, and that keeps two policies
with identical selections apart on purpose, without forwarding members.
Implementation of the decided parts (selection rows, structural identity,
static callable selections, the schema as a record type) is proceeding; these
four acceptance cases cannot be written as programs.

**Gap.** Contracts checked and found silent on the source form:

- [Conformances](wiki/spec/language/conformances.md#implementations-are-operation-records):
  *"a deliberately distinct policy carries an explicit brand"*. No section of
  `conformances.md`, `machine_values.md`, `domains.md` or
  [chapter 14](wiki/language_guide/chapter_14_traits.md) gives a declaration,
  field or clause that is a brand, or says how a brand enters identity.
- The same section: *"A strict proof is irrelevant, a stored theorem selection
  retains its declaration identity"*. Traits declare laws today as resultless
  `machine name(...) ensures ...;` requirements
  (`source/library/core/algebra.omg`). Nothing says which source form makes a
  law field a strict proof and which makes it a stored selection, so a row
  `Trait::law = some_lemma;` has no defined sort.
- The same section: *"a diamond duplicates nothing and conflicting selections
  need an explicit choice"*. A diamond has one originating requirement and so
  one slot; the text does not say what a conflict is in that case or how the
  choice is written. Qualified rows (`Left::set = a; Right::set = b;`) already
  separate same-named requirements of different origins.
- The same section and
  [provider selection](wiki/spec/build/provider_selection.md#build-owned-target-selection):
  *"Build may consume a computed record"*. `Build::select_provider<Slot,
  Provider>(builder)` takes a named implementation as a static argument. The
  text does not say whether a `const` record of the schema is admissible as
  `Provider` or whether another Build operation is meant.

**Questions for Zac.**

- **(a) Brand:** What is a brand in source: a field of the schema, a clause on
  the `satisfies` block, a declared marker type, or an ordinary relevant field
  the author adds? Does it take part in identity as a relevant field value?
- **(b) Law evidence:** Is `Trait::law = lemma;` a strict proof, so identity
  ignores which lemma was cited, with a separate form for a stored theorem
  selection? If so, what is that form?
- **(c) Explicit choice:** Is the choice a qualified row naming the parent
  whose selection wins, and is a conflict two parent implementations selecting
  different machines for one originating requirement?
- **(d) Computed record to Build:** Is a `const` value of the schema admissible
  wherever `select_provider` takes a named implementation, checked for slots,
  realization and admission as today?

One reading the implementer applies meanwhile, stated so it can be corrected:
structural identity unifies two all-row `satisfies` blocks with the same
selections, and does not unify a block with an inline member body with any
other, because *"An inline body still declares a new machine"*.
