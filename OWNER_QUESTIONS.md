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

## Q4 — Freeze Delta declaration namespaces and duplicate phase

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

## Q5 — Delegate package transport helper authority to the host

### Context

Package transport routing is settled as an ambient host concern. User and
system Git/SSH configuration, credential helpers, agents, identity files,
known-host policy, proxies, and the invoking environment are ordinary host
inputs. Omega owns the accepted locator and protocol surface, package-controlled
Git arguments, authenticated Git object graph, immutable snapshot, bounded
command lifecycle, and exact source receipt. It does not own host credential
custody or network topology.

The implementation still forces HTTPS through an Omega localhost CONNECT
broker and SSH through an Omega `ProxyCommand`. Removing only those overrides
does not finish the settled contract: macOS Seatbelt and Linux Landlock still
permit execution only from a compiler-preselected path set and confine writes
to the quarantine root. Host-selected `core.sshCommand`, `ProxyCommand`,
credential helpers, askpass programs, keychain tools, and noninteractive
known-host updates may therefore remain blocked even though ordinary Git and
SSH configuration is nominally inherited.

### Problem statement

Choose the remaining native child-policy boundary for networked Git phases.
Omega cannot both promise ordinary ambient Git/SSH behavior and pre-enumerate
every executable and writable host location that arbitrary trusted host
configuration may use. Parsing configuration does not close the set: includes,
shell commands, helper protocols, platform services, and helper-specific state
can select further behavior. Reproducing or brokering those mechanisms would
turn the package manager into a partial Git/SSH/credential provider without
making the invoking host more trustworthy.

### Proposed direction

For networked discovery and fetch, invoke the selected system Git under the
user's ordinary descendant-execution, filesystem, credential, and network
authority. Delete the CONNECT helper, forced HTTPS proxy, forced SSH command,
preselected SSH/helper executable set, broker transfer accounting, and native
claims that those ambient helper effects are confined. Keep only controls that
do not alter host transport behavior: closed package-selected protocols,
noninteractive execution, disabled repository hooks/replacements/redirects/
filters/submodules, bounded captured output and command lifetime, process-group
cleanup and honest native resource limits, quarantine publication, authenticated
object-graph validation, immutable snapshots, and exact locator/source receipts.

The universal receipt should bind the normalized requested endpoint, primary
Git executable, command construction and outcome, and final source. It must not
claim an observed socket peer, descendant helper identity, or network byte
ceiling that the universal path no longer measures. Non-network repository
initialization and inspection may retain their existing closed write and
execution policy because they have no ambient transport-helper requirement.
Stronger containment belongs to an explicitly selected host/CI environment,
outside package-authored semantics.

### Alternates

- Acceptable: drop native confinement for every Git phase, retaining only
  process lifecycle/resource controls and post-fetch validation. This is
  simpler and matches ordinary package-manager behavior, at the cost of giving
  local-only Git phases more ambient authority than they strictly need.
- Acceptable: retain a closed transport backend only as an explicitly selected
  host/CI profile that rejects unsupported ambient configuration up front.
  Package declarations cannot select it, and its receipts must remain distinct
  from the universal host-routed path.
- Acceptable but likely needless: discover and allow a bounded exact helper set
  for a deliberately restricted deployment profile. Failure to close the set
  must reject that profile rather than silently fall back or affect ordinary
  desktop resolution.
- Tempting but wrong: implement Git proxy discovery, SSH jump hosts,
  `ProxyCommand`, credential brokers, and platform key custody inside Omega to
  preserve a universal confinement claim.
- Tempting but wrong: remove only the command-scoped proxy overrides while
  native executable or write confinement still silently blocks the host
  configuration they expose.
- Tempting but wrong: retain zero-valued broker observations or a configured
  transfer ceiling as if either measured ambient host traffic.

## Q6 — Classify Gamma `Bytes` logical-length overflow

### Context

D16 makes `Bytes` an immutable finite byte sequence and fixes
`bytes_length : Bytes -> Int`, where `Int` is signed 64-bit. The compact runtime
representation permits constant-space concatenation: repeatedly concatenating
a rope with itself can exceed `INT64_MAX` logical bytes after roughly 63
descriptor allocations, long before the implementation exhausts its heap.
Neither D16 nor the Gamma guide says whether that operation traps, becomes a
private resource failure, or is excluded by another language invariant.

### Problem statement

Classify `bytes_concat(left, right)` when both operands are valid `Bytes` but
their mathematical combined length is not representable by `Int`. This is
observable Gamma meaning rather than merely a representation choice because a
compact implementation can reach it, `bytes_length` must return an `Int`, and
the compiler customer may construct output incrementally. Implementations must
not disagree by wrapping the length, fabricating a value, or conflating the
same authored operation with ambient heap exhaustion.

### Proposed direction

Trap deterministically before allocation when the exact combined length
exceeds `INT64_MAX`. Treat this as an authored operation outside the closed
`Bytes`/`Int` value relation, parallel to checked arithmetic overflow, not as
profile-dependent `Incomplete`. Do not mutate the heap or publish a partial
descriptor before the check completes.

### Alternates

- Acceptable: define a smaller fixed maximum `Bytes` length and trap every
  constructor that would exceed it, provided the limit is Gamma meaning rather
  than a private runtime capacity and `bytes_length` remains total.
- Acceptable: change the language to expose an unbounded length carrier, but
  that is a larger D16 revision and must update the compiler customer and all
  six built-ins together.
- Tempting but wrong: wrap signed length, call the deterministic value overflow
  `Incomplete`, rely on eventual physical allocation to make it unreachable,
  or silently flatten ropes until a private resource limit decides meaning.

## Q7 — Define the device-operation source contract

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

## Q8 — Attribute selected opaque representations across package reviews

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

## Q9 — Establish generic boundary-realization coverage

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
