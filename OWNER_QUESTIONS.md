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

Last pruned: 2026-08-31.

## Q1 — Complete the physical OCREQ/OCOUT v1 tables

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

## Q2 — Give the complete Gamma compiler an explicit Beta call-row profile

### Context

D23 fixes the Alpha-written Beta compiler at 1,024 non-builtin procedure-call
references. The retained `gamma_compiler.beta` now consumes exactly 965 rows
before any production entry, total returned-`Bytes` preflight, publication
replay, or D19 adapter is added. An adjacent probe accepts fifty-nine additional
calls and canonically refuses the sixtieth as
`Incomplete(call_rows, 1024, 1025)`.

An ordinary source-visible refactor already centralizes thirty repeated
immediate/conditional-jump pairs through two Beta helpers while preserving the
exact emitted instructions. It removed twenty-six call rows; the measurement
above is after that reduction, not the pre-refactor estimate.

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

## Q3 — Total Delta entry-shape diagnostics

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

## Q4 — Total Delta transition-pattern and coverage diagnostics

### Context

Delta fixes scalar and nominal-sum transition subjects, positional case-payload
binders, repeated selector/case rejection, final wildcard placement, and
static sum exhaustiveness. Its closed rejection set contains `TypeMismatch`,
`ArityMismatch`, `DuplicatePattern`, and `NonexhaustiveSum`, but the body/control
premise DAG does not order all relations that can fail at one pattern start.

A later repeated case can also have the wrong payload arity. A repeated scalar
selector or case can be incompatible with the subject category or nominal sum
owner. A known case can have both the wrong arity and wrong subject owner. The
general coordinate rules place each applicable reason at the later pattern's
first byte, so deriving the clauses independently produces D37's outer
`InternalFailure` rather than one Delta rejection. Restricting duplicate
identity to already complete patterns, or silently choosing category, duplicate,
or arity by traversal order, would add an unstated language rule.

Wildcard and absence shapes are also incomplete. A single nonfinal `_` is not
a repeated selector or case, two wildcards give both an overlap and repetition,
and `NonexhaustiveSum` has no offending pattern from which to derive its source
coordinate.

The Gamma-written compiler can retain a complete scalar or exact-sum subject,
resolved scalar-selector or exact-case identity, complete pattern and typed
binder custody after successful joins, and complete/missing/unresolved sum
coverage. It cannot promote the overlapping negative relations without an
authoritative order.

### Problem statement

Define one total transition-pattern premise DAG and exact source coordinates
covering:

1. a scalar selector used with a sum subject, or a case used with a scalar or
   different nominal-sum subject;
2. a known case with the wrong payload arity;
3. repeated semantic scalar selectors and exact cases, including whether a
   resolved but category- or arity-invalid earlier pattern participates in
   duplicate identity;
4. a wildcard followed by any authored arm and repeated wildcards; and
5. a sum transition that omits at least one declared case and has no final
   wildcard.

The ruling must preserve source-order arm checking and D37's rule that a failed
or unresolved pattern does not satisfy a coverage premise. It must state which
later relations are suppressed after each earlier failed premise, must not turn
a runtime scalar miss into a static error, and must not use reason-code order to
break a same-coordinate tie.

### Proposed direction

Resolve names first. Admit each resolved identity against the subject category
and exact nominal owner next; an incompatible identity contributes only
`TypeMismatch` at that pattern and does not enter duplicate or arity checking.
For a category-admitted selector or case, retain its semantic identity before
case-payload arity. A later repeated identity contributes only
`DuplicatePattern`, even when the earlier unique case later failed arity; a
unique case alone proceeds to `ArityMismatch`. Only a unique, category- and
arity-compatible pattern supplies complete pattern and binder facts.

Treat every wildcard with a following arm as `DuplicatePattern` at that
wildcard's first byte because it overlaps the remaining selector domain. A
later wildcard may also be repeated, but the earlier nonfinal coordinate wins.
Diagnose missing complete sum coverage as `NonexhaustiveSum` at the transition
subject's first byte. Coverage consumes a complete sum subject plus complete,
unique patterns, so it does not collide with an unresolved subject or pattern
relation.

### Alternates

- Acceptable: make duplicate admission precede subject compatibility or arity,
  provided the ruling explicitly says which resolved invalid patterns own
  identity and every same-coordinate combination has exactly one candidate.
- Acceptable: anchor `NonexhaustiveSum` at the transition keyword or closing
  brace, provided the coordinate is explicit and stable and incomplete pattern
  premises remain silent.
- Acceptable: use another existing closed reason for a nonfinal wildcard,
  provided its relation to a later repeated wildcard is total.
- Tempting but wrong: diagnose only duplicates whose patterns already completed,
  emit both category/arity and duplicate candidates at one pattern start,
  silently ignore arms after `_`, classify every scalar miss as a static
  rejection, report the missing case's declaration coordinate, add a new DCOUT
  reason without a version ruling, or choose whichever failure traversal
  encounters first.
