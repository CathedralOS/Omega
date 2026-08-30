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

## Q2 — Define the device-operation source contract

### Context

The memory and concurrency designs already distinguish DMA publication,
device acquisition, cache maintenance, MMIO notification, and posted-write
completion. The non-authorizing access-plan carrier retains each demand's full
mapped-subrange context, admitted schema/device correspondence, and nominal
ordering-scope identity, and it closes provider coverage exactly.

No source/core declaration or typed/checked operation currently emits any of
those demands. The design briefs describe their semantic effects, but do not
fix concrete operation signatures, argument/result custody, or how the
compiler constructs the nominal ordering scope. Inventing downstream checked
rows would therefore create producer-authored evidence with no language
operation to justify it.

### Problem statement

Define the source-visible or compiler-known operations that generate each of
the five requirement families. For every operation, settle:

1. the exact range, mapping, device/correspondence, request, and scope inputs;
2. which inputs are borrowed, consumed, or returned for retry;
3. the evidence or custody returned on success, including publication
   invalidation and completion-bound acquisition; and
4. whether ordering-scope identities are named by source, selected by an
   admitted provider, or issued by the compiler from a closed composition.

### Proposed direction

Expose sealed compiler-known operations with typed capability inputs rather
than user-constructible requirement rows. Derive the mapped-range and device
correspondence contexts from checked arguments, and issue the ordering-scope
identity during provider selection for the closed composition. Publication
returns range/write-state-bound evidence; acquisition consumes exact
request/device/range completion evidence and returns Stable custody only when
that completion returned device custody. Failed admission returns every
consumed candidate unchanged for retry.

### Alternates

- Acceptable: expose only complete DMA submission initially and keep the lower
  level five-operation vocabulary provider-private, provided checked drivers
  cannot claim to compose those operations independently.
- Acceptable: make the scope an explicit sealed capability argument, provided
  ordinary source cannot forge or compare its identity and provider admission
  still binds it to one closed composition.
- Tempting but wrong: emit synthetic requirements from tests or downstream
  access plans without a checked source operation, use compact range/device
  identifiers in place of retained contexts, or let erased proof values stand
  in for Terminal ordering events.

## Q3 — Attribute selected opaque representations across package reviews

### Context

Package review compiles dependencies first and compiles each package under that
package's own authoritative `build.omg`. A later consumer may select a concrete
`OpaqueRepresentation<Opaque>` conformance for dependency-owned opaque data.
The dependency's earlier review may therefore be unbound or may select a
different application for its own uses. The compiler now retains the exact
selected conformance and closes each actual by-value boundary use into a
target-, shape-, and calling-plan-bound application identity.

### Problem statement

Choose who owns canonical review evidence for that selection and what
"producer/consumer agreement" means before independently compiled artifacts
exist. Requiring the producer's source review to accept the consumer's choice
would invent producer authority that was never exercised. Requiring every
library build to preselect one representation would remove the target and
integration flexibility the build-owned selection was introduced to provide.
Conversely, recording only the consumer's compact application fingerprint
would not let closure review rejoin the selected conformance and carrier to the
producer's exact reviewed declarations.

### Proposed direction

Make the selecting consumer build own each demanded representation row. Retain
the exact selecting build machine, boundary requirement application, opaque
declaration, named conformance, concrete carrier, selected target, and strong
target-closed application commitment. Emit no demand row for an unused
selection.

Keep producer review factual: it publishes the opaque declaration and the
ordinary public conformance/carrier surface, but does not claim to accept a
consumer selection. Closure validation requires every foreign consumer demand
to rejoin those exact producer rows and the same locally checked source. The
single consumer compilation already uses one application on both sides of the
boundary. Independently compiled artifacts, replacement, and stable-handle eras
must later require equality of the same strong application commitment at their
actual composition boundary; source review must not pretend that composition
already occurred.

### Alternates

- Acceptable: let an opaque declaration publish one producer-fixed stable ABI,
  provided this becomes an explicit language contract and consumers cannot
  silently replace it through `build.omg`.
- Acceptable: retain consumer demand and producer availability as two distinct
  canonical row kinds rather than one role-tagged representation row, provided
  closure validation and diff rendering preserve the same exact joins.
- Tempting but wrong: treat publication of a conformance as producer acceptance
  of every consumer use, require a dependency's independent build selection to
  equal all future consumers, or copy a consumer decision into producer review.
- Tempting but wrong: call matching names, compact fingerprints, audit prose,
  or a lockfile string "agreement" without rejoining the exact declarations
  and strong compiler-issued application.

## Q4 — Establish generic boundary-realization coverage

### Context

An atomic boundary-operator family selection must cover every overload
coordinate. A coordinate with a static telescope may be covered either by the
complete concrete application set demanded by the artifact or by one genuinely
generic realization. Exact-application rows already have a closed structural
carrier and remain fail-closed when absent. No production carrier currently
states why one realization covers every admissible static application.

### Problem statement

Choose the compiler-recheckable fact that permits package and artifact evidence
to publish generic coverage. Ordinary type checking of a generic Omega body may
be sufficient for a checked realization, but a bodyless or external
realization has no body from which the compiler can derive parametric coverage.
Treating either a provider assertion or successful compilation of one concrete
application as universal coverage would manufacture a guarantee.

### Proposed direction

Permit generic coverage only for an exact checked Omega realization that the
compiler has checked under its complete symbolic static telescope. Retain the
exact binder categories and domains, requirement coordinate, realization,
selected plan, target, and transitive admissions needed to re-run that check.
This proves dispatch and plan coverage, not the truth of admitted external
behavior.

Bodyless, external, opaque, or separately supplied realizations do not acquire
generic coverage from an authored claim. They must provide the complete exact
application family demanded by the artifact, or use a separately designed and
explicitly admitted generic implementation contract.

### Alternates

- Acceptable: forbid generic coverage entirely and require canonical exact
  application families for every emitted artifact.
- Acceptable: define a recheckable generic implementation contract for foreign
  artifacts, provided it has an independent verifier and remains distinct from
  ordinary checked-body evidence.
- Tempting but wrong: accept a provider-authored `generic` flag, infer coverage
  from one successful application, or call a compiler/toolchain/version string
  a certificate that universal checking occurred.

## Q5 — Define exact boundary-realization application evidence

### Context

A selected boundary-operator plan retains one realization row, while one
artifact may demand many exact static applications of that coordinate. Package
review already has an `ExactApplications` row, but production does not attach
it or publish a checked use-side application carrier. The current application
schema records only arity and untyped semantic strings, although an operator
telescope may contain lifetime, type, const, machine, and proposition binders.
A provisional ordinary-type carrier was rejected before landing because making
it exact required independently re-solving those identity and substitution
rules before this question had settled their shared representation.

### Problem statement

Choose what the compiler must recheck before one demanded static application
may be published as covered. In particular, decide whether a generic checked
realization is specialized and rechecked once per retained application, or
whether each application must name a separately checked realization identity.
Also choose the tagged structural representation that binds every application
argument to its exact telescope category and domain. Merely copying the
artifact's demand strings beside one selected realization would prove demand,
not coverage.

### Proposed direction

Treat exact coverage as per-application compiler work. Reconstruct one tagged
application from the checked use, substitute it into the selected realization,
and re-run the applicable signature, contract, effect, target, and admission
checks for that specialization. Retain the exact requirement coordinate,
selected plan and realization, binder schema, tagged arguments, and rechecked
specialization identity. Deduplicate and order only after those joins succeed.

Keep this distinct from Q4: checking a finite demanded set does not establish
universal generic coverage. Initially supporting only ordinary type binders is
acceptable if every other telescope category remains explicitly fail-closed.

### Alternates

- Acceptable: require one separately checked concrete realization identity per
  exact application rather than specializing one generic checked realization.
- Acceptable: forbid exact coverage for selected binder categories until their
  structural argument and substitution rules are implemented.
- Tempting but wrong: call a canonical list of use-site strings coverage,
  infer specialization from one successful generic declaration check, or erase
  binder categories behind an arity-only schema.

## Q6 — Freeze the standalone Omega compiler request and outcome wire

### Context

D18 fixes the logical contents and custody of one sealed
`OmegaCompilationSubject` plus bound `OmegaInvocation`, and fixes the four
`OCOUT` outcome classes. It does not fix their authoritative byte encoding.
The maintained Rust compiler explicitly states that its host object layout is
not the standalone frame and that a future adapter must independently
reconstruct the canonical bytes. The Delta-written compiler `D` intentionally
has no `Main`: its parser retains relative source spans, but no source identity
or package custody can be attached until the sealed request is decoded.

### Problem statement

Freeze one byte-exact request and outcome contract shared by the Delta-written
and self-hosted Omega compilers. Choose together:

1. magic/version, integer width and endianness, count/length framing, exact-end
   rules, and canonical resource ceilings;
2. the ordered encoding of graph rows, package keys, requester-local aliases
   and edges, resolved source coordinates, optional accepted dependency
   instances, and every build-visible snapshot fact (files, directories,
   absences, links, metadata, raw-byte paths, and canonical directory order);
3. the invocation encoding for product, target profile, external admissions,
   and its non-substitutable binding to the exact subject;
4. validation order and the exact distinction between request `Reject`, private
   resource `Incomplete`, and compiler-contradiction `InternalFailure`; and
5. the complete versioned `OCOUT` failure schemas, including stable reason
   identities and request/source coordinates. Success remains the unwrapped
   requested artifact.

Without those choices, two conforming standalone compilers can accept different
bytes, derive different package/source identities, or publish incompatible
diagnostics while both claiming D18.

### Proposed direction

Use one domain-separated, versioned, little-endian table format with explicit
counts and byte lengths. Encode identities and snapshot facts structurally;
refer between validated tables by bounded indices, never host pointers or
compact fingerprints. Preserve canonical package, source-unit, raw-path, and
directory-entry order in the wire, and bind the invocation to a strong
commitment over the exact canonical subject bytes. Put every fixed ceiling in
the versioned request profile so exhaustion maps deterministically to
`Incomplete`. Require complete graph/custody/length validation and exact end
before source processing. Give `OCOUT` one closed versioned schema per failure
class, ordered first by phase and then by D18's request or source coordinate.

### Alternates

- Acceptable: use another closed deterministic table encoding, provided both
  compilers can decode it directly in their implementation languages and
  canonicality, ordering, exact-end validation, and all limits are explicit.
- Acceptable: split subject and invocation into separately committed canonical
  sections inside one sealed outer frame, provided substitution across those
  sections is impossible and the validation order is fixed.
- Tempting but wrong: serialize the Rust request object, use a host serde format,
  pass filesystem paths or a replay transcript instead of complete snapshots,
  infer product/target/source identity from filenames, or let `D` and `C`
  choose separate convenient encodings.

## Q7 — Complete Delta's census rules for transition binders and failures

### Context

D22 fixes Delta's grammar-selected namespaces, pre-type duplicate census,
active-local no-shadow rule, boundary-owner restriction, and globally earliest
later duplicate coordinate. Delta transition patterns also declare executable
payload binders in forms such as `Case::{left, right}`, but neither D17 nor D22
says whether those names enter the census, which active locals they may reuse,
or how their per-arm scopes interact.

One second ambiguity appears when the same program contains an authored body on
a boundary owner and an unrelated duplicate name. Both are discoverable while
collecting declarations. D17 orders the earliest packed offset within the
earliest failing phase, but D22 does not say whether `InvalidBoundary` belongs
to that same phase, whether duplicate census always precedes it, or which exact
token coordinates the boundary failure. Nor does it say how to classify a
qualified body when its owner spelling itself has conflicting boundary and data
declarations: treating the first owner row as authoritative would violate D22.
A collector cannot choose among those outcomes without changing accepted-
language diagnostics.

The product requirement is the complete D22 identity census in the canonical
Gamma-written Delta compiler. It must settle every declaration identity before
type formation so the Delta-written full Omega compiler `D` has one deterministic
accepted-language boundary.

### Problem statement

Choose together:

1. whether transition payload binders participate in declaration collection;
2. whether binders in one arm must be mutually unique and may shadow active
   machine parameters, state parameters, or earlier lets;
3. the exact lifetime and cross-arm relationship of those binder scopes; and
4. the phase and source coordinate for `InvalidBoundary`, including its ordering
   against unrelated `DuplicateName` failures and its behavior when the owner
   namespace is itself duplicate.

### Proposed direction

Treat transition payload binders as local-value declarations in the D22 census.
Binders in one arm are mutually unique and cannot reuse any machine parameter,
active state parameter, or earlier let from the containing body. They become
visible only in that arm's selected continuation; distinct arms are disjoint and
may reuse spellings. The binder-name token supplies a duplicate coordinate.

Keep `InvalidBoundary` in declaration collection. Combine it with duplicate
issues by minimum packed coordinate, using the owner-name token of the authored
qualified machine as the boundary coordinate. Thus neither traversal order nor
reason priority can hide an earlier collection failure. Classify a qualified
body as boundary-owned only after its owner has one unique boundary identity;
an owner with conflicting boundary/data declarations contributes its
`DuplicateName` issue but no first-row-derived owner kind.

### Alternates

- Acceptable: place boundary-owner validation in a distinct phase immediately
  after a complete duplicate census, provided `DuplicateName` then has explicit
  phase priority and the boundary coordinate is fixed.
- Acceptable: permit transition binders to shadow a precisely named subset of
  outer locals, provided initializer/continuation visibility and arm isolation
  are exact and the census still owns every resulting duplicate failure.
- Tempting but wrong: omit binders because they are nested in patterns, let the
  body checker report whichever collision it encounters, use first-wins local
  lookup, or choose `DuplicateName` versus `InvalidBoundary` from collector
  traversal order.
