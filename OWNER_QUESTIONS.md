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

## Q4 — Retire resolver execution custody that adds no source guarantee

### Context

The settled source-resolver boundary delegates Git, HTTPS, SSH, credentials,
helpers, proxies, and same-user operating-system authority to the invoking
host. Omega still owns the package-controlled boundary: bounded locators and
commands, immutable commit/tree selection, object and path validation,
compiler-owned snapshot materialization, drift rejection, process cleanup, and
concrete resource ceilings.

The implementation also retains an older execution-attestation layer. It
hashes and repeatedly checkpoints the selected Git executable, constructs a
closed matrix of native confinement “guarantees,” records every prepared
command and completion under those rows, hashes the result into a
`GitSourceReceipt`, and requires local Seatbelt/Landlock machinery even though
networked Git deliberately uses ordinary host authority. No package admission,
lock, compiler, or manager consumer relies on these rows. They cannot close a
same-user check-to-exec race or attest Git, SSH, credentials, the operating
system, or an audit.

The command policy also permits only the locator's original HTTPS or SSH
transport. Normal host `insteadOf` configuration that rewrites one admitted
transport to the other therefore fails even though both transports are valid
and host Git configuration is otherwise an operator input.

### Problem statement

Decide which execution controls are actually part of Omega's source guarantee,
without preserving self-issued telemetry as security evidence or overriding
ordinary host Git behavior. In particular:

1. whether executable content hashes and metadata checkpoints survive after
   one absolute Git path has been selected outside package-controlled roots;
2. whether execution-policy observations, command-completion provenance, and
   `GitSourceReceipt` have any admitting consumer or enforceable claim;
3. whether local repository initialization/inspection should impose
   Seatbelt/Landlock executable and filesystem policy on the operator's Git;
   and
4. whether host configuration may rewrite between the two admitted production
   transports, HTTPS and SSH.

### Proposed solution

Delete executable hashing and drift checkpoints, the execution-guarantee
matrix, canonical command/completion provenance, `GitSourceReceipt`, and
Seatbelt/Landlock executable/filesystem confinement. Select one absolute Git
path before package-controlled input, reject paths inside controlled roots,
and use that exact path for the complete operation without a later bare-name
lookup.

Retain argument separation, noninteractive execution, protocol closure to
HTTPS and SSH, redirect/hook/replacement/filter/submodule rejection, object
fsck, bounded output and command count, deadlines, process-tree cleanup,
portable resource ceilings, verified object traversal, immutable snapshot
materialization, cache/source-size bounds, and ordinary concurrent-drift
checks. Permit host configuration to rewrite between HTTPS and SSH while
continuing to deny HTTP, `git://`, `file`, `ext`, and every unselected
production protocol.

### Alternates

- Acceptable: retain local-phase OS confinement only if it protects a concrete
  package-controlled mutation boundary without restricting operator-selected
  Git wrappers or descendants, and describe it solely as optional defense in
  depth rather than source evidence.
- Acceptable: retain bounded operational diagnostics outside canonical source,
  review, lock, or admission identity, provided their absence cannot reject an
  otherwise valid resolution.
- Tempting but wrong: keep self-hashed policy rows because they are detailed,
  treat a Git binary hash as trust or continuous immutability, require an
  unused sandbox executable to remain unchanged, or forbid an operator's
  HTTPS-to-SSH rewrite while claiming host Git configuration is inherited.

## Q5 — Keep carrier cleanup out of opaque semantic ownership

### Context

D26 gives a semantic opaque value one build-selected concrete carrier. General
layout can now derive the carrier's exact target shape, while existing checked
ownership and Terminal plans retain semantic multiplicity, transfers, and
cleanup for ordinary source values. The missing join is finalization when the
selected carrier itself has an attached nominal cleanup machine or otherwise
owns independently disposable state.

The opaque declaration's multiplicity and discharge belong to the semantic
type. Importing the carrier's source-level ownership policy would silently add
semantics to an empty `OpaqueRepresentation<Opaque>` relationship; ignoring a
real carrier finalizer would be equally unsound.

### Problem statement

Decide whether D26 v1 admits carriers with independent nominal cleanup and, if
so, how one exact carrier finalizer is reconciled with opaque transfer,
success/failure ownership, and the rule that ABI byte copies relocate one
logical owner rather than manufacture another.

### Proposed solution

Keep D26 v1 narrow: a selected carrier must be compiler-proven transitively
relocatable and independently cleanup-free. The opaque declaration remains the
sole authority for semantic multiplicity and discharge. ABI-required byte
copies are physical relocation of that one semantic value, never semantic
copying; finalization of the representation itself is no-code. Reject a
selection whose carrier requires nominal cleanup instead of guessing how its
resource ownership maps onto the opaque value.

This is sufficient for the current interrupt carriers and lets the compiler
finish an honest movement/finalization commitment without inventing a general
resource-owning representation protocol.

### Alternates

- Acceptable: define a later explicit, compiler-checked lifecycle relationship
  that binds the opaque application to one exact carrier finalizer and total
  ownership rules for success, failure, return, and abandonment.
- Acceptable: admit a broader structurally derived cleanup-free carrier class,
  provided the compiler can prove it has no independently invoked finalizer and
  its physical copies remain relocations of one semantic owner.
- Tempting but wrong: require opaque and carrier multiplicities to be equal,
  automatically run the carrier's ordinary `drop`, ignore carrier cleanup,
  treat an ABI copy as semantic copying, or publish a D26 demand before the
  lifecycle join is complete.

## Q6 — Define the closed terminal-authority classification policy

### Context

The two-axis authority model settles the containment judgment: an exact
service identity and normalized schema permit terminal-authority classes, and
a target-qualified exact selected binding plus provider context exercises
classes. Package review must reject an exercised class outside the permitted
set, and unknown terminal mechanisms fail closed.

The implementation has exact selected-provider rows for normalized imports,
the temporary string-backed import bootstrap, syscalls, compiler intrinsics,
vtable/table operations, and checked adapters. It does not have the normative
closed policy that assigns classes to arbitrary target-qualified syscall,
import, firmware, or table coordinates. The existing seven service risk
classes do not determine those assignments. For example, neither a Linux
syscall number nor a Windows export name says whether it performs filesystem
content access, namespace mutation, process control, or no authority without a
compiler-owned target policy row.

### Problem statement

Define the first closed terminal-authority policy, including:

1. the exact target, binding payload, provider-context, and service-schema
   coordinates that key every row;
2. the exercised class set for each currently admitted syscall, import,
   compiler intrinsic, vtable/table operation, and checked physical operation;
3. whether known authority-free terminal mechanisms require explicit empty
   rows or are derived by a separately closed purity catalog; and
4. the normalization rule for `StringBackedImportBootstrap` where its current
   payload cannot express an ordinary target locator (notably versioned ELF
   and Mach-O).

Without these rows, deleting the filename classifier either rejects existing
supported provider closures as unknown or invents authority semantics from
names and numeric coordinates. That choice changes the language's installed
authority claim rather than merely selecting an implementation strategy.

### Proposed direction

Publish a versioned compiler-owned table keyed by the complete tuple above.
Require explicit rows, including explicit empty exercised sets, for every
terminal mechanism accepted into package review. Reuse the existing closed
compiler-intrinsic execution atoms where available. Normalize the bootstrap
form before lookup only when it reconstructs one exact ordinary foreign
locator; otherwise reject it until the retained carrier is enriched. Traverse
checked adapters only through their exact selected closure and reject cycles,
missing leaves, and unclassified physical operations.

### Alternates

- Acceptable: land a first table covering only the already closed compiler
  intrinsics and reject every other terminal binding until target policy rows
  are supplied, provided existing supported-package fallout is intentional.
- Acceptable: define policy in target-owned modules rather than one central
  table, provided the combined vocabulary is closed, versioned, exact, and
  exhaustively validated.
- Tempting but wrong: classify by service name, source filename, package role,
  import spelling alone, or syscall number without target and context; assume
  unknown means harmless; or map every binding beneath a service to that
  service's permitted class set.

## Q7 — Fix the zero-parameter Delta state-transfer spelling

### Context

Delta transition continuations are postfix expressions. D36 explicitly
requires every machine application, including a zero-parameter machine, to
author an argument list. State transfers initialize state parameters, but the
contract does not say whether a zero-parameter state may be transferred to as a
bare spelling or must likewise author `()`. The distinction affects callable/
state ambiguity, arity admission, and the diagnostic for a known bare state.

### Problem statement

Choose whether every state transfer requires an authored argument list or a
zero-parameter state additionally admits a bare spelling. If the bare spelling
is rejected, assign its exact closed Delta rejection reason and anchor. The
Gamma-written Delta compiler needs this rule to total transition-continuation
classification without inferring syntax from the resolved arity.

### Proposed direction

Require an authored argument list for every state transfer, including `()` for
a zero-parameter state. A known bare state is not a transfer and contributes
`InvalidControlTarget` at the continuation expression start. This keeps state
and machine application syntax uniform and resolves state/machine identity
before arity rather than letting zero arity select a meaning.

### Alternates

- Acceptable: admit a bare spelling only for a uniquely resolved zero-parameter
  state, provided a same-spelled unqualified machine still yields D36's
  `InvalidControlTarget` before arity.
- Tempting but wrong: let the compiler try both spellings and select whichever
  declaration has matching arity, or let source order choose state versus
  machine.

## Q8 — Assign invalid Delta `self` diagnostics

### Context

D36 permits an authored receiver only on an owner-qualified data machine. In
that scope, `self` has the owner's nominal value and storage place and supports
the required receiver-qualified calls. The grammar also parses `self` in an
unqualified or receiverless machine, but the Delta contract assigns no reason
or anchor to that use. Inventing a value would infer an owner D36 forbids;
silence would leave an authored expression with no public rejection.

### Problem statement

Assign the rejection reason and exact source anchor for `self` outside a
receiver-bearing qualified data machine. This is needed to close expression
resolution and all downstream place/call premises while retaining the rule
that no global owner or storage is inferred.

### Proposed direction

Treat `self` as one grammar-provided local identity that exists only when the
current machine declares `&mut self`. Outside that scope it contributes
`UnknownName` at the `self` keyword and no value/place fact. Inside the valid
scope it retains the exact current data-owner declaration and a nominal storage
place; receiver mutability is therefore not guessed from use.

### Alternates

- Acceptable: use `TypeMismatch` at the `self` keyword if the language defines
  `self` as a known contextual expression of the wrong machine category rather
  than an absent binding.
- Tempting but wrong: infer `Main`, infer the only data owner, create a typeless
  recovery receiver, or defer the failure until a later field/call suffix.

## Q9 — Anchor a resultless Delta call used as an argument

### Context

D37 makes authored argument checking a sibling of callable admission and
arity. It requires a resultless call used where a value is required to
contribute `TypeMismatch`, and it requires a wrong-arity application to retain
independent failures inside its arguments. `InvalidTerminal` already anchors a
`never` argument at that exact call. The contract does not assign the analogous
resultless value-use failure a coordinate.

The general call relational anchor is the enclosing application start, but
`ArityMismatch` uses that same coordinate. Applying both clauses to a
wrong-arity call with a resultless argument creates two distinct reasons at one
coordinate, which D37 classifies as outer `InternalFailure`. Anchoring the
failure at the argument expression avoids that contradiction, but would be a
new language rule rather than an implementation choice.

### Problem statement

Assign the exact source anchor for `TypeMismatch` when a resultless call is
used directly as a call or constructor argument. The rule must preserve D37's
independent argument branch when the callee is unresolved or inadmissible and
when arity is wrong, without turning ordinary rejected Delta into compiler
`InternalFailure`.

### Proposed direction

Anchor the category failure at the resultless argument expression's first
byte. Reserve the enclosing application start for arity and the later
all-value argument-type relation. This lets a wrong-arity candidate coexist
with the independently checked argument without a same-coordinate reason
collision, and it treats grouping around the resultless call as the authored
value-use expression rather than moving the anchor to the callee.

### Alternates

- Acceptable: assign another explicit argument-owned coordinate, provided it is
  stable under callee resolution and cannot collide with the application-
  anchored arity/type relations by construction.
- Tempting but wrong: silently use the application start and produce
  `InternalFailure`, suppress the argument because arity or callee admission
  failed, choose an anchor by traversal order, or manufacture a value/error
  type for the resultless call.

## Q10 — Total Delta block exits and machine continuations

### Context

D17 requires a resultless machine to return without a value or fall off a
reachable block, a value-returning machine to return a value on every reachable
normal path, and a `never` machine to have no normal return path. D37 completely
anchors an authored return relation and makes the first ordinary statement
after a successful standalone `never` call `InvalidTerminal` at that following
statement. The compiler can implement those two relations without guessing.

Three adjacent exits are not total. An explicit `return` or `transition` after
the standalone `never` call is not an ordinary statement, so D37 assigns no
failure or coordinate. Reachable falloff in a value-returning or `never`
machine violates D17 but has no rejection reason or anchor. Finally, a
transition continuation may invoke an unqualified machine, but the language
does not say whether that call is a tail return whose result category/type must
match the enclosing machine, a discarded call followed by falloff, or a
separate nonreturning control transfer.

These are product semantics for the compiler-host language, not implementation
recovery choices. Until they are fixed, a state/control-flow graph cannot
honestly classify every reachable block exit or prove the return obligation.

### Problem statement

Define one total block-exit judgment covering:

1. an explicit return or transition after a successful standalone `never`
   call, including reason and exact coordinate;
2. reachable falloff from resultless, value-returning, and `never` machines,
   including reason and exact coordinate; and
3. the return/termination effect of an unqualified machine application used as
   a transition continuation.

Also state whether transition reachability conservatively includes every
authored arm or may use subject-value feasibility. The judgment must support
cyclic states without mistaking divergence for a normal return path.

### Proposed direction

Give every reachable block one closed exit fact: resultless falloff, explicit
return, exact `never`, state transfer, or machine tail transfer. Treat every
authored transition arm as a possible edge; constant-value feasibility is an
optimization and does not suppress static checking or return obligations.
Diagnose an explicit terminal after `never` as `InvalidTerminal` at that
terminal's first byte, matching the existing first-following-statement rule.
Diagnose forbidden falloff as `TypeMismatch` at the closing brace of the
falling-off machine entry or state body. A machine continuation is a tail
transfer and must have the same resultless/value/`never` return category and,
for values, the same structural type as the enclosing machine.

Compute the obligation by a cycle-safe fixed point over exact retained machine
and state identities. A cycle with no normal-exit edge satisfies a `never`
obligation; it does not manufacture a return value.

### Alternates

- Acceptable: anchor forbidden falloff at the owning body or state declaration
  start, provided the coordinate is explicit and stable for empty and nonempty
  bodies.
- Acceptable: define machine continuations as discarded calls followed by
  falloff, provided that behavior and its effect on each enclosing return
  category are explicit.
- Tempting but wrong: choose a brace, earlier `never` call, or continuation
  result based on compiler traversal; treat only the first transition arm as
  reachable; infer termination from a bounded simulation; or call the missing
  diagnostic an implementation detail.

## Q11 — Total Delta transition-pattern and coverage diagnostics

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
