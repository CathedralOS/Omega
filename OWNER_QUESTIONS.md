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

## Q2 — Establish generic boundary-realization coverage

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

## Q3 — Define exact boundary-realization application evidence

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

Keep this distinct from Q2: checking a finite demanded set does not establish
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
## Q4 — Fix the physical D19 Gamma application profiles

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
