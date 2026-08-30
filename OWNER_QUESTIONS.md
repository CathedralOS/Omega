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

## Q2 — Define exact boundary-realization application evidence

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

Keep this distinct from D28's deliberately unimplemented universal-template
rule: every emitted artifact still closes its finite demanded application set,
even when a future checked generic body has already established symbolic
semantic coverage. Initially supporting only ordinary type binders is
acceptable if every other telescope category remains explicitly fail-closed.

### Alternates

- Acceptable: require one separately checked concrete realization identity per
  exact application rather than specializing one generic checked realization.
- Acceptable: forbid exact coverage for selected binder categories until their
  structural argument and substitution rules are implemented.
- Tempting but wrong: call a canonical list of use-site strings coverage,
  infer specialization from one successful generic declaration check, or erase
  binder categories behind an arity-only schema.

## Q3 — Fix the physical D19 Gamma application profiles

### Context

D19 settles the logical two-profile contract. `ConformanceBytesV1` requires
pure `main : Bytes -> Bytes`; `DeltaCompilerV1` requires the exact source-owned
`DeltaCompileOutcome`/`DeltaRejectReason` schema and D17's 26-code rejection
bijection. The Beta-written Gamma compiler now resolves and validates both
entry schemas before emission, independent of declaration order.

The final generated adapters are still not byte-exactly specifiable. No ruling
assigns a physical profile ID or source-plus-profile request envelope, either
profile's exact sealed-input maximum, the Conformance runtime observation
table, `GCOUT` magic and closed tables, or `DCOUT` magic and its resource and
internal-failure tables. Only the common four halt tags, 40-byte failure-frame
shape, and `DCOUT` rejection codes 1 through 26 are fixed. Guessing these facts
inside the compiler would make reconstruction depend on implementation folklore.

### Problem statement

Choose one exact version-1 realization covering:

1. the sealed compilation-request bytes that bind one profile ID to the exact
   Gamma source;
2. the exact maximum sealed application input for each profile and any maximum
   successful Conformance output;
3. the complete Conformance runtime status/resource observations;
4. `GCOUT` magic, rejection/resource/internal codes, and coordinate spaces,
   including selected-profile schema mismatch; and
5. `DCOUT` magic plus its resource/internal codes and coordinates, retaining
   D17's existing rejection table unchanged.

### Proposed direction

Use one small length-delimited `GCREQ` v1 envelope with explicit numeric profile
IDs; the envelope, profile ID, exact source bytes, and profile metadata all
participate in compiler identity. Keep both application-input maxima explicit
and conservative under the existing Gamma heap/`Int` bounds rather than
inferring them from stdin EOF or source names.

Publish `gcout-v1.tsv`, `dcout-v1.tsv`, and the Conformance observation table
beside the compiler and make the implementation and gates consume those exact
tables. Preserve the common halt tags and 40-byte family shape, but give each
edge distinct magic and closed codes. Schema mismatch is a `GCOUT` rejection;
generated-program exhaustion and contradiction use only the selected
application profile's outcomes.

### Alternates

- Acceptable: make profile selection a fixed out-of-band field in the checked
  compiler invocation rather than an encoded `GCREQ`, provided reconstruction
  retains that field exactly and it cannot be inferred from source or filename.
- Acceptable: choose different maxima for conformance and Delta compilation,
  provided both are exact D21-valid profile facts with adjacent fail-closed
  canaries.
- Tempting but wrong: assume 4 MiB because the current Gamma source reader uses
  that ceiling, derive wire codes from constructor order, reuse `BCOUT` magic,
  or let `main`/type names select the profile.

## Q4 — Complete Delta v1 type-formation rejection rules

### Context

D17 fixes Delta's type forms, declaration shapes, phase ordering, closed reject
set, and exact source-coordinate principle. The Gamma-written compiler can now
scan every named type against the complete D22/D24 owner census. Several
remaining type-formation cases do not have one exact result in the normative
contract, so implementing them would silently amend Delta v1.

`NAT` admits zero, but `InvalidArrayLength` has no stated triggering set. A
`data` must be exactly a record or sum, but the empty declaration has neither a
fixed classification nor an error coordinate. `never` and views have forbidden
positions, but the contract does not assign every such occurrence to
`TypeMismatch`, `InvalidDataShape`, or `EscapingView`. It also does not settle
ties when an outer forbidden form and a nested unknown or recursive type begin
at the same or competing type-formation coordinates.

### Problem statement

Fix one complete type-formation judgment covering:

1. the admitted array-length set and exact `InvalidArrayLength` coordinate;
2. empty and mixed field/case declarations under `InvalidDataShape`;
3. the exact rejection reason and coordinate for `never` and views in every
   field, payload, parameter, local, array, view, and return position;
4. whether the boundary owner `Console` is a general named capability type or
   is admitted only at the exact `Main.console` boundary position; and
5. deterministic reason selection when multiple type-formation defects share
   the earliest authored coordinate.

### Proposed direction

Admit array lengths `1..INT32_MAX`; reject zero at its literal as
`InvalidArrayLength`. Reject empty or mixed `data` declarations as
`InvalidDataShape` at the declaration name. Use `TypeMismatch` for `never` in
non-return positions and `EscapingView` at the outermost forbidden view token;
an earlier outer placement failure wins over defects nested inside that form.
Treat `Console` as a sealed capability type admitted only for the exact
`Main.console` field fixed by D17, with other placements classified by the
boundary/entry-shape check rather than ordinary nominal type formation.

### Alternates

- Acceptable: admit zero-length arrays, provided `InvalidArrayLength` receives
  another exact, source-reachable definition or is removed in a D17 revision.
- Acceptable: permit an empty record with an explicit zero-initialized meaning,
  provided it cannot also acquire an empty-sum interpretation.
- Acceptable: use one dedicated placement reason for all forbidden `never` and
  view occurrences, provided its coordinates and within-phase priority are
  total.
- Tempting but wrong: infer these results from a host layout, historical
  compiler, likely use by `D`, or whichever recursive check happens to run
  first.
