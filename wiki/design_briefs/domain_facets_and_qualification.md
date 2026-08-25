# Design Brief: Domains And Qualification

Current design as of 2026-07-30. Chapter 8 carries the language-guide
surface. This brief owns domain meaning, establishment, `as`, semantic roles,
normalization, and the staged units model.

## One surface, independent internal aspects

A **domain** is a zero-cost static theory attached to a runtime carrier. One
domain may contribute any compatible combination of:

- predicate requirements, discharged by the prover;
- semantic declarations, such as denotation, dimension, operators, or
  conversions;
- exact trait requirements authorized to establish provenance; and
- transparent aliases over compatible domain atoms.

These aspects share one source declaration because they compose. `Utf8` has a
predicate requirement, `Km` contributes denotation and unit operations,
`Wrapping` contributes an arithmetic policy, `Percent` may contribute both a
range predicate and unit meaning, and `Reservation::Issued` is a routed
historical fact.

They remain separate compiler records and algebras:

```text
DomainTheory {
    carrier,
    predicate_requirements,
    semantic_contributions_by_role,
    authorized_establishment_requirements,
    alias_expansion,
}
```

The prover consumes predicate bodies. Static operator resolution consumes
semantic contributions. Establishment checks consume routes and receipts.
Multiplicity governs copying and outstanding obligations. Carry governs where
the resulting value or claim may travel. A domain declaration does not fuse
those systems.

Domains add no runtime tag, wrapper, hidden storage, or second object model.
Qualification changes the static theory carried by a value, not its runtime
representation.

## Predicates and establishment routes

The declaration separates propositions from provenance:

```omega
domain [u8]::Path
    requires no_nul(self);

pub domain Reservation::Issued
    established by Issues::issue;

domain Reservation::Confirmed
    requires has_seat(self)
    established by Confirms::confirm;
```

`requires` is uniformly propositional, exactly as it is on a machine. Every
predicate must hold. `established by` contains exact trait-requirement
identities; each comma-separated entry is an alternative authorized origin for
the domain.

The row must resolve to `Prop`. A machine returning `bool` does not become a
predicate merely because it is written in call shape. Primitive and transparent
proposition applications retain their exact normalized proposition identity.
Eligible total, pure machine calls may still occur *inside* a proposition as
denotational value terms under the ordinary fact-position rule; they do not run
when qualification is checked, and their complete checked meaning remains in
the proposition dependency. An executable validator is a separate ordinary
machine with its own reach, control, authority, and result contract. Calling it
may establish a structural proposition through an authored guarantee, but the
call itself is not a domain predicate.

For example, address-range geometry is stated rather than executed:

```omega
proposition no_wrap(base: addr, length: u64) =
    embed(base) + embed(length) <= addr::Bound;
```

`addr::Bound` is a target-semantic compile-time constant supplied through the
sealed target capsule described by
[`build_time_evaluation.md`](build_time_evaluation.md). The transparent formula
has no runtime Boolean result, reach, crash edge, or provider selection.

The establishment clause does not invoke those requirements. It licenses their selected
conformances to establish membership at exact qualified subject positions. A
result is established by the selected call. A non-`self` parameter is
established only at an installed external-root invocation and remains a
precondition at an ordinary call. A checked conformer proves every predicate at
the subject. A boundary conformer also requires selection and admission, whose
evidence remains attached to the subject.

The domain owner has no ambient minting privilege. A bare qualified result or
`ensures` clause supplies an obligation, not evidence. Owner code and
third-party code alike must prove the predicates or return through an exact
authorized route. Trait visibility controls who may conform; machine
visibility controls who may invoke an existing route. A public ordinary route
deliberately permits checked external conformers, while a public boundary
route admits opaque external providers.

An empty declaration has no establishment obligations:

```omega
domain i32::Km;
```

Every bare `i32` may therefore be explicitly qualified as `i32::Km`. This is
the vacuous case of the same rule, not an implicit owner grant.

Predicate-only and routed domains therefore have deliberately different
membership rules. Proving every predicate establishes a predicate-only
structural qualification. If `established by` is present, those same proofs are
necessary side conditions but cannot manufacture the routed provenance: one
exact authorized introduction or forwarding occurrence is also required. In
particular, proving `no_wrap(extent.base, extent.length)` never mints
`Extent::Granted`.

## `as`: exact coercion and explicit erasure

The governing rule is:

> **`as` never silently changes denotation: qualified targets preserve it;
> an explicitly bare target erases non-owning semantic meaning.**

It may change representation when one unique exact transformation is intrinsic
to the carrier types. It never invokes arbitrary user code or discovers a
domain-specific conversion.

| Axis | Requirement |
|---|---|
| denoted value or referent | unchanged |
| proof | predicates and representability discharged before lowering |
| reach and control | no service reach, allocation, suspension, failure, or user code |
| policy | no hidden loss, rounding, saturation, trapping, or ambiguous choice |

Consequences:

- `bytes as [u8]::Path` succeeds only when `no_nul(bytes)` is known;
- `5 as i32::Km` is direct qualification into an obligation-free domain;
- `byte as u16` is an exact integer coercion;
- `bounded_word as u8` succeeds only when representability is proved;
- `reservation as Reservation::Issued` fails because `as` cannot fabricate
  route provenance; and
- lossy, fallible, allocating, policy-bearing, or noncanonical transformation
  remains a named machine.

Unit coercion is an ordinary named library machine or heterogeneous operator
conformance. `as` may add an obligation-free unit domain or explicitly erase
one, but it does not infer a scale relation or inject the library's conversion
operation.

## Establishment, propagation, and conservation

All membership feeds one qualification judgment while retaining its evidence
source:

| Evidence source | What it establishes |
|---|---|
| prover | the domain's predicate requirements |
| vacuous qualification | a domain with no predicates or routes |
| authorized checked conformance | routed provenance plus its proved predicates |
| checked transformation | inherited or conserved evidence |
| admitted boundary conformance | routed provenance under its selected provider receipt |

An admitted membership assertion is valid only on an exact qualified subject
of a boundary requirement named by `established by`. A result establishes at
the selected call; a non-`self` parameter establishes only at an installed
external-root occurrence and remains a precondition at an ordinary call. A
direct accepted-machine membership guarantee is not authorization. Checked
proof facts retain the boundary trait, exact requirement signature, and
semantic subject position, and the artifact records them with the public
origin class and selected provider evidence where applicable. Private proof
steps and implementation witnesses remain private evidence.

Reconstructing equal carrier fields does not reproduce qualification. Existing
qualified values retain their facts through ordinary assignment, move, and
permitted copy. Mutation invalidates a subject-bound fact unless the operation
explicitly preserves or re-establishes it.

Multiplicity, not the domain declaration, governs duplication and debt:

- unrestricted carriers may copy;
- affine carriers may move or abandon but may not duplicate;
- linear carriers must move and eventually discharge.

A fact participating in a conserved resource transformation requires a
non-copyable carrier. A must-discharge obligation requires a linear carrier or
an independent linear token. A reusable historical fact such as
`Artifact::Admitted` may live on a reusable carrier.

Multiplicity does not imply divisibility. A content-bearing qualified claim
separately publishes one normalized projection into a compiler-owned partial
composition algebra. That projection governs establishment backing, authorized
access, n-ary decomposition/recomposition, and retirement accounting. The
initial resource vocabulary is an interval over proof-level natural bounds and
a counted quantity over proof-level naturals. The qualification owner publishes
at most one conformance to the core `Content<A>` projection requirement; the
conformance's selected algebra and normalized projection become semantic
identity. Ordinary claims and qualifications that carry no decomposable
resource content acquire no content entry at all: whole-claim conservation
already accounts for their transfer and cleanup. Domain facets retain their
predicate/semantic meaning, while permission attenuation, content, lineage,
and carry remain independent claim axes.

Content projection confirms that predicate and provenance obligations compose.
For example, an address-range authority combines one checked geometry predicate
with one routed origin:

```omega
domain Extent::Granted
    requires no_wrap(self.base, self.length)
    established by ExtentRootProvider::grant;
```

Every authorized route proves the predicate at its exact established subject.
Here that subject is the result. The content projection then embeds `base` and
`length` into proof `Int`, proves their nonnegative unbounded sum, and converts
the coordinates exactly into the proof-`Nat` interval algebra; it never relies
on wrapping runtime address addition.

Carry is independent. Mobility demands attach to the established value or
resource provenance and survive qualification forgetting until the underlying
claim is discharged.

## Semantic roles and operator coherence

Semantic contributions are keyed by a small compiler-known role vocabulary.
Compatible contributions in different roles compose into one operator
meaning; competing contributions to the same role reject. Cross-role
composition is not permission to run two unrelated operator implementations
in an arbitrary order: the selected contracts must determine one checked
meaning.
The initial roles are:

| Role | Examples | Consumer |
|---|---|---|
| predicate knowledge | `NonZero`, `Utf8`, ranges | prover |
| denotation/dimension | `Km`, `Metres`, `Degrees` | normalizer and operator result theory |
| arithmetic policy | `Exact`, `Wrapping`, `Saturating`, `Trapping` | primitive arithmetic lowering |

Later compiler releases may add roles such as rounding or comparison policy
when a real customer requires them. Role identity is closed and
compiler-owned; packages author theories within the admitted roles.

This distinction is load-bearing. `Km` and `Wrapping` both affect `+`, but in
different ways: `Km` determines dimensional meaning while `Wrapping`
determines overflow behavior. They compose. `Wrapping & Trapping` contributes
two arithmetic policies and rejects.

Predicate obligations compose independently. A standing range combined with
an arithmetic policy is legal only where every permitted operation preserves
the range; flow facts may instead be invalidated and later re-proved.

A domain predicate does not synthesize its operators. A domain-owned operator
still publishes a signature and relational contract, and its checked
definition or selected satisfier must discharge that contract. For a
normalized degree domain, returning a value in `[0, 360)` is necessary but not
sufficient: the contract must also relate the result to the operands modulo
360, or an implementation that always returned zero would pass.

The same example shows how roles compose without competing overloads. Two
normalized degree operands lie in `[0, 359]`, so their unreduced sum lies in
`[0, 718]`. The degree-addition realization can therefore prove that its
carrier addition is Exact, then reduce modulo 360. If the bindings also select
`Wrapping`, machine-width overflow remains unreachable and the two arithmetic
policies are observationally identical for that operation. This is a local
proof of policy independence, not a claim that Wrapping and Exact are globally
equivalent.

Operator resolution reads static binding qualifications, never incidental
flow facts. Resolution is compile-time, unambiguous, and recorded in the
checked artifact. Adding an unrelated import cannot inject a competing
meaning.

Named machine and requirement calls additionally admit result-domain overload
sets. Their dispatch projection contains normalized semantic-role
contributions, routed provenance, and empty explicit tags; a domain carrying
only predicates contributes no dispatch key. The expected result projection
must equal one declared projection, with the empty projection selected when no
expected result is available. Predicates and ordinary compatibility are checked
only after that lookup. Equal projections on the same path and parameter
signature are a declaration-site duplicate, even if the written results differ
by predicate refinements. Fixed operator spellings retain their separate
operand-directed rule.

## Arithmetic policies

`Wrapping`, `Saturating`, and `Trapping` are the closed core arithmetic-policy
vocabulary. Qualifying a value with one of them performs no runtime work.
Subsequent operations use the selected behavior:

- `Wrapping` reduces at the declared machine width;
- `Saturating` clamps representable-range overflow;
- `Trapping` emits a runtime result check and terminal trap.

The later operation may therefore cost work or terminate abnormally even
though qualification itself cannot.

The same policy role applies to integer and float carriers through their
ordinary operator requirements; it does not imply identical failure sets.
Float `Saturating` clamps magnitude overflow but does not assign a value to an
invalid operation such as `0.0 / 0.0`. Float `Trapping` rejects a non-finite
semantic result through the checked adapter; it never mutates the hardware
exception mask. `Finite & Saturating` therefore removes magnitude-overflow
proofs while retaining obligations such as a nonzero divisor.

Specification arithmetic uses the same selected operation only when it is
total. Exact partiality is discharged at term formation; Wrapping and
Saturating make representable-range overflow total while retaining independent
primitive obligations such as a nonzero divisor. Direct Trapping arithmetic
rejects in `Prop` because its partiality is resolved by runtime control.
`embed(value)` explicitly projects a
fixed-width integer or address payload into proof `Int`; a same-carrier `as`
instead removes the policy and selects Exact carrier arithmetic with its normal
obligations. Float denotation uses `FloatMeaning`. The complete bridge and crash
coverage rules live in
[Total Specification Arithmetic](total_specification_arithmetic.md).

Mixed arithmetic policies reject. Arithmetic-policy removal or replacement
changes only future operator selection; it does not reinterpret an already
stored payload.

An arithmetic-policy qualification may be explicitly erased to the
unqualified carrier, whose arithmetic is Exact by default. The current payload
is preserved and every later Exact operation must discharge its ordinary
safety obligations. This does not reinterpret earlier wrapping arithmetic as
exact mathematics. Selecting or removing Wrapping, Saturating, or Trapping is
explicit because it changes future operation behavior.

Core arithmetic-policy domains use the same empty or predicate-qualified
establishment rules as an obligation-free semantic domain such as `Km`. Their
primitive lowering
is special; their establishment and `as` behavior is not a second
qualification mechanism.

## The operation taxonomy

Denotation, carrier representation, and runtime work are independent:

| Operation | Denotation | Runtime behavior |
|---|---|---|
| exact coercion with `as` | preserved | compiler-derived carrier work only |
| explicit non-owning semantic erasure | discarded visibly | none |
| predicate weakening | preserved, fact forgotten | none |
| representation recast | same bits under its validated plan | none |
| validation | establishes a proposition | ordinary checked work |
| noncanonical conversion | operation contract defines it | ordinary named machine |

Numeric widening and proven exact narrowing belong to `as`. Unit-scale change,
and narrowing that wraps, saturates, traps, rounds, or returns a checked
result, select an ordinary named machine or an explicit policy domain.

## Weakening and forgetting

Weakening is evaluated independently for each normalized domain atom:

- predicate-only atoms may disappear implicitly;
- semantic atoms and non-owning routed provenance require an explicit `as`
  whose target omits them;
- a domain carrying both predicates and a route follows the routed rule; and
- owned claims cannot be cast away and must be consumed or transferred.

Thus an `i32::Km & Positive` may pass where `i32::Km` is expected but not where
bare `i32` is expected. `distance as i32` explicitly erases the semantic unit.
Direct `distance as i32::Degrees` rejects because no denotation-preserving
relation exists; an author can still write the conspicuous
`distance as i32 as i32::Degrees`.

## Transparent aliases

A public alias names a nonempty conjunction over compatible subjects:

```omega
pub domain Socket::Usable =
    Socket::Connected & Socket::Authenticated;
```

Expansion precedes normalization and identity hashing, so the alias and its
atoms have one normalized identity. Alias edits change every published
contract that expands them. Diagnostics report missing atoms rather than
stopping at the alias name.

Compiler-owned atoms may participate. `Carry::Portable` expands to the four
positive carry permissions; packages may publish their own aliases over that
closed vocabulary.

## Normalization is not entailment

The deterministic normalizer owns what a domain expression *is*: sorted and
deduplicated conjunctions, canonical closed index values, licensed symbolic
index forms, semantic roles, and alias expansion. Type identity, semantic
interface identity, and monomorphization keys depend on this normalized form.

The entailment engine proves propositions about that identity. Stronger future
proof automation may accept more programs but may not change normalized
identity or operator meaning.

Physical ABI remains the carrier's ABI. Semantic interface identity includes
the normalized domain theory.

## Indexed domains as the generic stress test

Units are an ordinary library customer, not compiler vocabulary. The useful
generalization is an erased domain family indexed by canonical proof-static
data. The generic declaration binds its carrier explicitly:

```omega
domain<T, const U: Unit> T::Quantity<U>;
```

It imposes no carrier-wide arithmetic requirement. A unit library may define
canonical values for `KM`, `M`, and `SECOND`, then use this one nominal
`Quantity` family across `f64`, `i64`, proof-only, or vector carriers. Ordinary
operator conformances state only the carrier operations they need. A
closed derived index such as `KM / SECOND` evaluates at build time; a generic
result such as `A / B` remains a normalized constraint fact until its equality
obligations discharge.

This does not add a wrapper: the physical ABI remains the carrier's. Semantic,
policy, and predicate facets continue to compose independently, so a
quantity-domain operator need not enumerate every `Positive` refinement and
does not acquire a combined `Quantity × Saturating × Positive` conformance.
The operation's `ensures` proves any predicate fact that survives. Erasure
removes metadata cost, not arithmetic cost: scaling, range checks, and rounding
remain visible work in the selected library operation.

The same capability serves coordinate frames, currencies, tensor shapes,
fixed-point scales, and protocol encodings. Unit conversions remain ordinary
named machines with explicit contracts. `as` neither recognizes units nor
dispatches to their conversion machinery.

## Implementation staging

The compiler already carries independent predicate, semantic-role,
establishment-origin, normalized route, alias, and receipt records. The source
parser and establishment checker use predicate `requires` plus exact authored
`established by` requirement routes; checked artifacts retain those identities,
and neither
owner placement nor boundary contract placement infers authority. The coercion
resolver now enforces denotation-preserving integer `as`: widening follows the
source carrier range and narrowing or signedness changes require a complete
representability proof. Proof-static indexed domains follow three ordered
implementation rungs. Per-atom weakening and
explicit erasure are enforced across ordinary value-flow boundaries, including
same-data-carrier provenance erasure; the remaining domain-theory artifact
fields still need to adopt the rest of this brief.

Migration should:

1. move domain propositions to ordinary `requires` and parse exact requirement
   identities from `established by` as alternative routes;
2. make an empty declaration obligation-free and remove ambient owner-package
   establishment;
3. remove the legacy core qualification relationship from domain
   establishment;
4. make an authorized route's exact result, or an exact non-`self` parameter at
   an installed external-root invocation, establish provenance only after
   every domain predicate is proved;
5. keep exact `as` limited to compiler-derived carrier coercion, direct
   qualification, and explicit erasure; domain-specific conversions remain
   ordinary machines;
6. preserve the implemented per-atom implicit weakening and explicit semantic,
   provenance, and arithmetic-policy erasure while ownership continues to
   govern claim removal; and
7. preserve those facts through generics, contracts, artifacts, and separate
   compilation.

Structured canonical const values and closed indexed family constraints are
implemented. One declaration spans carriers, canonical closed values identify
instances, direct destination binders survive generic signatures, and ordinary
per-pair operators attach to the family without affecting layout. Indexed
explicit qualification now selects either a closed value or a direct
destination binder and retains that exact instance through checked artifacts.
Const-machine call specialization now executes destination-parameterized
conversion for canonical result/parameter evidence, including distinct cloned
instances. The shipped `omega::language::std::units` package now exercises
named closed combinations, explicit conversion/scaling policy, and ordinary
per-pair operators across imports in both engines; implicit cross-index calls
reject. PDI2 is complete. PDI3 now carries computed open result indices, exact
licensed normalization authority, named compatibility conditions, and retained
closed/normalization/local-fact evidence. These extend the facts a domain may
carry without changing this qualification model or indexed-domain syntax.

## Cross-references

Chapter 8 owns the guide surface; chapter 5 owns primitive arithmetic;
chapter 10 owns proof machines; chapter 14 owns traits, complete named
conformances, and exact-requirement satisfiers;
chapter 16 owns terminal failure; and
`authority_values_and_boundary_evidence.md` owns authority provenance and
receipts.
