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

Last pruned: 2026-08-30.

## Q1 — Own proof-only FloatMeaning equality and source correspondence

### Context

The Float catalog already defines exact binary32/binary64 meaning projection:
finite values map to nonzero rationals, signed zero and infinity remain
distinct, NaN payloads erase only in proof meaning, and cross-format projection
rejects. Checked and Terminal evidence retains exact projection invocations,
operators, operands, formats, equality coordinates, tables, and provenance.
The wider proof/`Real` connection now reaches the proof-kernel boundary rather
than lacking an executable Float model.

### Problem statement

The proof kernel accepts equality only over its existing scalar-term carrier;
it has no proof-only `FloatMeaning` or general `ProofValueId` term. Two
independently authored meaning projections also do not retain one shared
landed-source identity, while Terminal equality rows currently have neither an
exact contract owner nor an evidence-provenance lane that can authorize their
coalescing. Implementing any one of those choices privately would decide which
proof terms exist and when two authored projections denote the same value.

Choose together:

1. the kernel term that carries proof-only `FloatMeaning` values;
2. the accepted equality rule for that sum, including NaN-payload erasure and
   signed-zero distinction;
3. the exact source-coordinate identity and coalescing rule for independently
   authored projection invocations; and
4. the contract/evidence owner that Terminal replay must bind before such an
   equality can discharge an obligation.

### Proposed direction

Add a closed proof-only semantic-term carrier whose Float child is the existing
`FloatMeaning` sum, not a runtime scalar or tagged ABI. Bind every projection
term to the exact checked source value/projection occurrence and its canonical
Float table identity. Kernel equality compares the semantic sum structurally
under the documented payload erasure; coalescing is permitted only when the
retained landed-source identity and projection contract are identical.
Terminal evidence names that contract owner and source correspondence
explicitly, and the verifier independently reconstructs both before invoking
the equality rule.

### Alternates

- Acceptable: introduce a general proof-value term encompassing other
  proof-only sums, provided its equality rules are closed per carrier and do
  not create a runtime representation.
- Acceptable: avoid source-occurrence coalescing by retaining one explicit
  theorem/contract application that relates the two projections, provided its
  owner and complete evidence provenance survive Terminal replay.
- Tempting but wrong: encode `FloatMeaning` as a runtime scalar, compare raw
  float bits, or collapse signed zero merely because NaN payloads erase.
- Tempting but wrong: equate independently authored projections by matching
  operator names, format labels, compact fingerprints, or coincidentally equal
  values without a shared source/contract owner.

## Q2 — Complete the physical OCREQ/OCOUT v1 tables

### Context

D18 fixes the standalone Omega compiler's logical package subject, invocation,
build evaluation, generated-source custody, and publication model. D25 fixes
the eight-byte `OCREQ` identity, outer subject/invocation lengths and exact end,
the canonical facts each section must carry, the shared `OCOUT` header, and its
sole package/source-coordinate tail. `D` now validates that exact outer request
envelope without interpreting either inner section.

The landed ruling does not enumerate either section's byte order, row
envelopes and widths, numeric variant tags, reserved bytes, or the exact
subject-commitment domain and preimage. It likewise refers to edge-owned
`OCOUT` reason/resource/internal tables, phase order, and scalar provisions
without publishing them. The Rust request object is explicitly
nonauthoritative. Consequently the Delta-written and Omega-written compilers
can satisfy the prose while accepting incompatible byte streams or emitting
incompatible failures.

### Problem statement

Publish one complete version-1 physical profile covering:

1. the exact subject and invocation field order and every row envelope;
2. numeric tags and reserved-byte rules for source lineage, immutable
   resolution, package role, snapshot rows, products, targets, and admissions;
3. exact structural encodings for names, paths, revisions, identities, graph
   indices, dependency aliases, and the selected root;
4. the subject-commitment domain separator, preimage, and 32-byte digest
   placement;
5. closed `OCOUT` Reject, Incomplete, and InternalFailure code/coordinate
   tables plus fixed diagnostic phase precedence; and
6. every named request/compiler scalar provision that can produce
   `Incomplete(limit, requested)`.

### Proposed direction

Publish one normative `ocreq-v1` field table and adjacent closed `ocout-v1`
code/resource tables. Use explicit little-endian length-delimited rows and
zero-based validated graph indices; give every closed sum a fixed numeric tag
and zero reserved bytes. Domain-separate the commitment with a fixed literal
followed by the exact subject-section bytes, not a reconstructed object.
Represent each private capacity as one named scalar resource and order framing,
canonicality, graph, snapshot, admission, commitment, source, build, and later
compiler phases explicitly.

### Alternates

- Acceptable: split subject or invocation into independently versioned nested
  sections, provided the outer D25 exact-end frame remains unchanged and every
  nested identity and extent is canonical.
- Acceptable: choose a different structural identity or commitment preimage,
  provided both compilers can reconstruct it solely from the same request bytes
  and no host/Rust representation participates.
- Tempting but wrong: infer tags from Rust enum order, serialize the current
  Rust request, let `D` define one private layout and `C` another, or postpone
  reason/resource numbers until publication code happens to need them.

## Q3 — Give the complete Gamma compiler an explicit Beta call-row profile

### Context

D23 fixes the Alpha-written Beta compiler at 1,024 non-builtin procedure-call
references. The retained `gamma_compiler.beta` now consumes exactly 994 rows
before any production entry, total returned-`Bytes` preflight, publication
replay, or D19 adapter is added. An adjacent probe accepts thirty additional
calls and canonically refuses the thirty-first as
`Incomplete(call_rows, 1024, 1025)`.

This is the realistic next-rung compiler closure named by D12, not a synthetic
stress case. A straightforward allocation-ordered, cycle-rejecting and
DAG-sharing-aware `Bytes` preflight alone crosses the bound long before both
PC-zero adapters and their failure framing exist. Encoding that logic as an
opaque preassembled blob, host-generated table, or private higher-level
operation would evade rather than satisfy the direct Beta-to-Alpha edge.

### Problem statement

Choose one auditable way for the complete Beta-written Gamma compiler to fit:

1. revise the D23 Beta compiler's non-builtin call-row capacity and rebuild its
   exact artifact/admission subject;
2. retain 1,024 rows but require a specific source-level direct-emitter
   representation that materially reduces call references without introducing
   a hidden compiler stage or opaque generated program; or
3. replace another settled private Beta resource arrangement with an explicitly
   bounded profile that makes the same complete source admissible.

The ruling must keep Beta's language and Alpha's instruction set unchanged,
preserve canonical `Incomplete` behavior at the new adjacent bound, and leave
the Gamma compiler as ordinary audited Beta source.

### Proposed direction

Advance the private D23 call table from 1,024 to 2,048 rows, relocate its
adjacent compiler work tables coherently, and rebuild the Alpha-written Beta
compiler tape plus exact source/tape admission subject atomically. Keep every
other Beta language and compiler-boundary rule unchanged. Before ratification,
stage the complete Gamma adapter implementation against that candidate ceiling
and confirm an exact call count with useful headroom; if 2,048 is not enough,
select the smallest measured power-of-two profile that is.

### Alternates

- Acceptable: retain 1,024 after a measured, source-visible refactor of the
  direct emitter that leaves enough room for the complete adapters and remains
  simpler to audit than the present repeated calls.
- Acceptable: add a generic checked instruction-plan emitter owned entirely by
  `gamma_compiler.beta`, provided the plan is authored in Beta, validated by
  the existing direct-emitter/fixup substrate, and demonstrably reduces total
  source and proof complexity rather than hiding a bytecode blob.
- Tempting but wrong: raise the table silently, weaken total `Bytes` preflight,
  publish before validation, generate Beta source or Alpha bytes on the host,
  add an Alpha opcode, or treat the 1,024-row refusal as invalid Gamma source.

## Q4 — Total Delta entry-shape diagnostics

### Context

Delta v1 fixes the accepted headline shape: one `Console` boundary with four
specified callable signatures, a record `Main` with exactly one sealed
`console: Console` field plus ordinary program fields, and exactly
`machine Main::main(&mut self)` with no value parameters or return. D31 also
assigns every other authored `Console` type occurrence to `InvalidEntry` at the
type token.

The remaining entry judgment is not total. `LANGUAGE.md` anchors
`MissingEntry` at source extent, but does not say whether it means only absence
of the exact `Main::main` identity or also absence of `Main`, `Console`, or
`Main.console`. It assigns malformed, missing, duplicate, and “competing” entry
shapes to `InvalidEntry` without defining their complete candidate set or
coordinates. It also displays one exact boundary declaration without saying
whether member order and parameter binder spellings are semantic.

This matters because entry and ordinary body/control failures share the final
checking phase. The compiler must derive all candidates and choose the smallest
packed coordinate; it cannot promote whichever entry defect its traversal sees
first or use the DCOUT reason-code order to break a tie.

### Problem statement

Fix one complete entry-shape judgment covering:

1. the exact partition between `MissingEntry` and `InvalidEntry` for every
   absent, malformed, extra, or competing `Console`, `Main`, `Main.console`, and
   `Main::main` component;
2. the source coordinate for each authored defect and for each required but
   absent component;
3. deterministic priority when entry candidates coincide with one another or
   with ordinary body/control candidates; and
4. whether boundary-member order and parameter binder spellings participate in
   validity, or only member identity, positional parameter types, and return
   type do.

### Proposed direction

Treat boundary members as an unordered identity/signature set and parameter
binder spellings as nonsemantic. `MissingEntry` means only that no exact
`Main::main` machine identity was authored and is anchored at source extent. A
present `Main::main` with the wrong receiver, parameters, or return is an
`InvalidEntry` candidate at that machine declaration. Extra or malformed
authored boundary/Main components are `InvalidEntry` at the first byte of the
offending declaration, member, field, or type token. When the exact entry
machine exists but a required component is absent, use source extent for that
`InvalidEntry` candidate. Merge these with every body/control candidate solely
by packed coordinate; distinct reasons at one exact coordinate are an internal
contradiction unless this ruling assigns a specific structural suppression.

### Alternates

- Acceptable: require the displayed boundary member order or binder spellings,
  provided each mismatch and its exact coordinate are explicit and declaration
  packing order is not accidentally promoted into callable identity.
- Acceptable: reserve distinct synthetic coordinates for absent entry
  components, provided they are part of the Delta judgment and DCOUT coordinate
  contract rather than compiler-private sentinels.
- Tempting but wrong: let declaration traversal choose the first defect, use
  reason-code order to break ties, classify a malformed present entry as
  `MissingEntry`, or publish executable golden coordinates before the judgment
  is total.

## Q5 — Settle compiler-owned native builtins without fake package evidence

### Context

Terminal Psi retains bodyless boundary requirements and native lowering
correctly rejects any requirement with no exact realization. The native
realization API currently admits every boundary through a
`NativeProviderSettlement` carrying `ProviderExecutionEvidence`, including
compiler-owned target operations such as the standard console exit operation.
The production compiler driver passes an empty settlement list. Consequently,
ordinary rooted native compilation reaches target lowering and fails with
`MissingBoundarySettlement`, including canaries whose checked in-module
provider dispatch is otherwise complete.

Package-installed or foreign provider code genuinely needs admitted execution
custody. A compiler-owned target builtin is different: the compiler/backend is
the implementation authority, and no package build or external execution was
audited. Minting a package-style execution receipt for it would make the type
system appear satisfied without adding an independent fact.

### Problem statement

Decide how compiler-owned target builtins enter native boundary settlement
without pretending they carry package-installation evidence. The decision must
retain exact requirement, selected ProviderPlan row, target, lowering identity,
and emitted-artifact custody while preserving the stronger evidence requirement
for installed and foreign providers.

### Proposed solution

Give compiler-owned builtin bindings a distinct native settlement lane. The
backend derives the exact builtin realization from the selected ProviderPlan
row and native target, rejects unknown or duplicate mappings, and retains that
compiler/target identity in the native artifact as a TCB fact—not as
`ProviderExecutionEvidence`. Keep `NativeProviderSettlement` and admitted
execution evidence for package-installed and foreign implementations only.

This makes the authority honest: the compiler can prove which builtin it
selected and emitted, but does not claim an external audit or installation
event occurred.

### Alternates

- Acceptable: have target selection produce a first-class
  `CompilerBuiltinSettlement` before native realization, provided it remains a
  separate evidence class and is rejoined to the exact selected plan and
  target during lowering.
- Acceptable: model compiler builtins as part of the trusted target profile
  rather than as provider executions, if the profile retains the same exact
  requirement-to-lowering mapping.
- Tempting but wrong: fabricate an object implementing
  `ProviderExecutionEvidence`, special-case the console call by source name or
  numeric boundary ID, globally permit unresolved boundaries, or require users
  to install/audit code that is actually shipped inside the compiler backend.

## Q6 — Settle Delta `.as_slice` receiver validity

### Context

Delta needs one allocation-free conversion from a fixed array to the immutable
`&[T]` view accepted by compiler helpers and boundary machines. D17 names
`.as_slice` and says it returns the full view, but the surrounding paragraph
discusses both arrays and views without stating whether an already existing
view also owns that postfix member. The two readings accept different source
programs; neither parser shape nor lowering convenience can decide validity.

The array conversion is an independently required language capability. The
view-on-view form adds no capability because its receiver already has the exact
result type, but its acceptance still must be explicit so independent Delta
compilers agree.

### Problem statement

Choose whether `.as_slice` is valid on:

1. fixed arrays only; or
2. both fixed arrays and immutable views as an identity conversion.

D37 fixes the rejection reason and anchor for a receiver outside the accepted
set.

### Proposed direction

Admit `.as_slice` only on a fixed array and return an immutable full-range
`&[T]` with no assignable place. An existing `&[T]` is already the desired
value and should be used directly; do not preserve a redundant view-on-view
member merely because the grammar accepts arbitrary postfix names.

### Alternates

- Acceptable: admit the operation on both arrays and views, specifying that the
  view form preserves the exact current bounds and evaluates its receiver once.
- Tempting but wrong: let one compiler silently accept the redundant form,
  infer validity from a generic field lookup, or leave it dependent on whether
  lowering can erase the operation.
