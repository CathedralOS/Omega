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

## Q3 — Complete GCOUT boundary priority and schema coordinates

### Context

D30 fixes the physical `GCREQ` envelope, profile registry, compiler-resource
limits, `GCOUT` table, and broad phase order. The standalone Beta-written Gamma
compiler must therefore produce one byte-identical failure frame for every
request. Two boundary cases still admit incompatible implementations.

A declared Gamma-source length above 4 MiB may also describe a truncated or
trailing request. D30 does not say whether exact-end framing wins before the
`source_bytes` resource check, nor which request byte identifies an unknown
profile or oversized length field. After a valid frontend pass, codes 19
through 21 distinguish `missing_entry`, `entry_schema_mismatch`, and
`profile_schema_mismatch`, but a missing declaration has no authored byte and
the present schema validators can discover several defects at once.

### Problem statement

Fix one deterministic compiler-boundary judgment covering:

1. whether exact request truncation/trailing-byte validation precedes an
   oversized declared Gamma-source length;
2. the exact `GCREQ` coordinate for malformed framing, unknown profile, and
   `source_bytes` exhaustion;
3. the precise partition between `missing_entry`, `entry_schema_mismatch`, and
   `profile_schema_mismatch` for both V1 profiles;
4. the coordinate used when a required entry, type, constructor, or reason is
   absent; and
5. deterministic priority when multiple entry/profile schema defects coexist.

These are public offline-compiler interoperability requirements, not gate-only
diagnostics: independently built admitted Gamma compilers must emit the same
`GCOUT` frame for the same request.

### Proposed direction

Validate the complete structural envelope and exact end first. Report a first
missing or trailing byte as `malformed_request`; then reject an unknown profile
at byte 8; then report an oversized declared source as
`Incomplete(source_bytes)` at byte 12 with the declared value as `requested`.
Treat a zero-based source EOF position as an admissible schema coordinate for
a missing required declaration. Classify a missing `main` as `missing_entry`,
a present `main` with the wrong signature as `entry_schema_mismatch`, and every
other selected-profile nominal-shape or reason-bijection defect as
`profile_schema_mismatch`. Retain all schema candidates and choose the earliest
coordinate, breaking exact-coordinate ties by codes 19, 20, then 21.

### Alternates

- Acceptable: let a self-contained oversized length field win immediately,
  provided its coordinate and priority over truncated/trailing bodies are
  explicit and every implementation can decide it without buffering beyond
  the selected resource limit.
- Acceptable: reserve a non-source coordinate space for absent declarations,
  provided `gcout-v1.tsv` and the frame contract are revised together rather
  than calling EOF a source byte implicitly.
- Acceptable: use a fixed schema-category priority rather than earliest source
  order, provided every defect combination and coordinate is total.
- Tempting but wrong: expose whichever row the current validator encounters
  first, use coordinate zero for every absence, merge the three settled codes,
  or defer the choice to adapter-emission order.

## Q4 — Give the complete Gamma compiler an explicit Beta call-row profile

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

## Q5 — Place D29 physical boundary-application coverage

### Context

D29 requires every closed boundary-operator application to rejoin its exact
selected provider plan, role-specific realization, concrete representation,
layout, calling plan, and emitted execution. Checked lowering can now retain
the application demand, provider settlement binds a strong selected-plan
commitment to the checked use, and selected dispatch can identify checked-body
or compiler-intrinsic semantic realization.

The current pipeline does not establish the remaining physical facts before
Terminal Psi. Checked-to-Terminal owns the semantic operation/callee mapping;
target lowering later computes call placement, assignment later fixes physical
locations, and native realization later emits bytes and relocations. The
current D29 wording nevertheless says complete physical coverage enters
Terminal and that Terminal replay recomputes it.

### Problem statement

Choose the authority boundary for D29's physical half:

1. reorder representation, calling, assignment, and emission planning so a
   complete physical coverage row can enter Terminal; or
2. split D29 evidence at the existing semantic/physical boundary, retaining
   the closed application, selected plan, role payload, and operation join in
   Terminal while publishing concrete placement/emission custody only in the
   realized native artifact.

This is not a choice about weakening coverage. It decides which artifact can
truthfully own facts that the current compiler computes only after Terminal.

### Proposed direction

Use the split boundary. Terminal retains the exact checked-use to source-free
operation join, closed typed application, strong selected-plan identity, and
role-tagged semantic realization. Native realization carries that row through
target lowering, assignment, relocation, and successful emission, then
publishes a physical child receipt in `NativeArtifact`. Package review may
publish semantic D29 evidence for checked/package claims; a native or external
execution claim must additionally recheck the native receipt.

This keeps Terminal source-independent and semantically complete without
pretending it owns plans that do not yet exist. It also avoids creating a new
nominal IR stage or moving the entire physical backend earlier solely to match
evidence wording.

### Alternates

- Acceptable: move a genuinely reusable, target-owned calling/layout plan
  before Terminal and let Terminal retain it, provided assignment, relocation,
  and emitted-byte custody remain explicitly later rather than being claimed
  by that earlier plan.
- Acceptable: make one realized-artifact envelope contain Terminal plus its
  physical receipt, provided the two evidence classes and their independent
  replay rules remain distinguishable.
- Tempting but wrong: hash emitted bytes without retaining the checked-use to
  Terminal operation to target/assigned operation and relocation join.
- Tempting but wrong: promote provider-plan fingerprints, selected-dispatch
  rewrites, or provider-asserted indexed-application strings into physical
  coverage.
- Tempting but wrong: reorder the whole backend or add a nominal `Chi` stage
  solely so existing prose can continue saying that Terminal owns facts which
  are naturally produced later.
