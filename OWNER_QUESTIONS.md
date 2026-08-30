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

## Q2 — Select the generated Gamma application profile

### Context

D16 fixes Gamma as a pure typed language and fixes the Beta-written Gamma
compiler as a direct Gamma-to-Alpha compiler. Gamma programs have no byte I/O;
an emitted application therefore needs a generated adapter. The Delta compiler
customer requires the `DCOUT` adapter and reason table fixed by D17, while
ordinary Gamma conformance programs need different entry and observation
profiles. `GCOUT` governs rejection by the Gamma compiler itself and does not
select the adapter of the program it emits.

The canonical Gamma compiler invocation is presently described as consuming
Gamma source and producing Alpha tape. The fixed Gamma grammar contains no
trusted application-profile declaration, and Alpha tape has no ambient command
line from which the emitted program can recover one.

### Problem statement

Choose how a canonical Gamma compilation selects the generated program's entry
declaration, argument construction, result validation, and boundary profile.
The choice must keep the source question and reconstructed compiler edge exact.
It cannot infer boundary authority from package-controlled declaration or type
names. Hardwiring `DCOUT` would make the compiler unable to compile the general
Gamma programs required by its own language suite.

This blocks only adapter publication and the final compiler tape. The complete
lexer, parser, resolver, type checker, typed IR, profile-independent lowering,
and emitter remain implementation work under D16.

### Proposed direction

Make the application profile an explicit, sealed compiler input alongside the
Gamma source. Use a closed profile ID whose specification owns the entry
signature, generated adapter, boundary identity, and exact reason-code table.
The compiler validates the selected profile against the resolved entry type
before emission. The first production profile is D17's Delta compiler; compact
closed profiles may serve Gamma language conformance. Include the profile ID in
the exact compilation question and reconstruction evidence rather than in
ordinary Gamma syntax.

### Alternates

- Acceptable: publish separately identified compiler artifacts whose only
  difference is one fixed generated adapter, provided every artifact and edge
  binds that profile explicitly and shares the same checked Gamma semantics.
- Acceptable: add an explicit compiler-owned application declaration to Gamma,
  provided it is ruled as language syntax and cannot counterfeit another
  boundary merely by naming its types or constructors.
- Tempting but wrong: infer `DCOUT` from declarations named `main`, `Complete`,
  `Reject`, or `DeltaCompileOutcome`.
- Tempting but wrong: hardwire the general Gamma compiler to the Delta customer,
  use an ambient host flag absent from the reconstructed edge, or let a script
  rewrite the emitted tape afterward.

## Q3 — Freeze Gamma declaration identity and lexical scope

### Context

D16 fixes Gamma's grammar, mutual declaration visibility, static type checks,
and evaluation order. It does not say whether type, constructor, function,
parameter, or pattern-binder names must be unique. It also does not define the
scope of `let` and pattern bindings or whether those bindings may shadow an
outer binding.

The temporary type checker currently resolves the first matching global row and
the last matching local row. It therefore accepts duplicate globals and binders
with accidental first-wins or last-wins behavior. The untyped interpreter has a
positive `let`-shadowing example, but D16 explicitly classifies that executable
as an oracle rather than a language authority.

### Problem statement

Choose the declaration-identity and lexical-scope rules required for one
deterministic Gamma resolver. Without them, two conforming compilers can assign
different meaning to the same accepted source. This blocks the resolver/type
checker portion of the Beta-written compiler, not lexical validation, strict
grammar parsing, target ABI work, or profile-independent emission machinery.

### Proposed direction

Require user type names, constructor names, and function names to be unique in
their respective namespaces. Require parameter names to be unique within one
function and constructor-pattern binders to be unique within one pattern;
duplicate binders reject rather than assert an equality constraint.

Evaluate a `let` initializer in the outer environment, then bind its name only
within the body. Let and match-arm bindings may shadow outer parameters, lets,
or pattern bindings. Catch-all and constructor-pattern bindings scope only over
their arm, and bindings in different arms are independent. Keep type and
constructor namespaces separate, so one spelling may deliberately name both a
type and its constructor. `Int`, `Bytes`, keywords, and the closed `bytes_*`
built-ins remain reserved as already required by D16.

### Alternates

- Acceptable: forbid all lexical shadowing, provided the rule is uniform and
  every resolver rejects it rather than selecting an accidental table row.
- Acceptable: merge the type and constructor namespaces, provided D16 states
  that change explicitly and existing source/gates are updated together.
- Tempting but wrong: preserve the temporary checker's first-global/last-local
  lookup as semantics merely because it currently accepts focused examples.
- Tempting but wrong: allow duplicate pattern binders to imply equality without
  adding an explicit pattern rule and executable comparison semantics.

## Q5 — Freeze Delta declaration namespaces and duplicate phase

### Context

D17 requires whole-closure two-pass checking, forward visibility for top-level
declarations and states, unique names in each declaration scope, and the
earliest packed offset in the earliest failing phase. It does not fully define
which top-level forms share a namespace or which unique-name checks belong to
declaration collection rather than type or ordered body checking.

The Gamma-written Delta compiler now parses every D17 grammar form. Its next
phase must collect exact identities before type formation. A concrete collector
can compare source spans without copied strings and can find the globally
earliest later declaration, but it cannot choose the missing namespace and
phase rules without changing accepted Delta meaning. This directly affects the
Delta implementation language used to author the full Omega compiler `D`.

### Problem statement

Choose together:

1. whether boundary/data owner names and unqualified machine names inhabit one
   namespace or separate type-owner and machine namespaces;
2. whether a boundary member `Owner::name` and a top-level qualified machine
   body with the same owner/name are duplicate declarations;
3. whether parameters and ordered locals are precollected for `DuplicateName`
   before type formation, or checked in signature/body phases; and
4. whether locals and state parameters may shadow an active machine parameter
   or another binding from the enclosing machine invocation.

Without these rules, two conforming Delta compilers can choose different
declarations or report different rejection reasons when one source contains
both a duplicate and a later-phase type/body error.

### Proposed direction

Keep one type-owner namespace for boundary and data declarations, and one
machine namespace keyed by `(optional owner, machine name)`. A type owner may
therefore share its spelling with an unqualified machine without ambiguity.
Boundary members occupy their owner's machine slots; a program-authored
qualified machine may not redefine one, and a boundary owner admits no extra
program-authored machine bodies.

Make all unique-name failures part of declaration collection, including
boundary signatures, data fields/cases and payload parameters, machine/state
parameters, states, and `let` declarations. Preserve ordered local visibility
for use-before-initialization, but prewalking names makes `DuplicateName` phase
priority independent of later type resolution. Delta v1 has no lexical
shadowing: an active machine/state binding or earlier local cannot be
redeclared in the same effective body environment. Report the first byte of
the globally earliest later declaration in the collection phase.

### Alternates

- Acceptable: check parameter duplicates during type formation and local
  duplicates during body checking, provided D17 explicitly fixes that phase
  ownership and how their offsets compete with other failures in that phase.
- Acceptable: allow precisely defined local or state-parameter shadowing,
  provided lookup, initializer scope, state entry, and forward-reference rules
  are explicit and deterministic.
- Acceptable: merge type-owner and machine namespaces, provided owner-qualified
  lookup and every existing Delta example are updated consistently.
- Tempting but wrong: let the collector silently choose separate namespaces,
  first-wins lookup, or a partial set of duplicate scopes because that is easy
  to implement.
- Tempting but wrong: report whichever duplicate or type error an implementation
  happens to encounter first without preserving the frozen phase priority and
  minimum packed coordinate.
