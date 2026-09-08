# Chapter 8: Domains

A domain is a zero-cost semantic theory attached to a value's unchanged
carrier.

A domain can state predicates, contribute a unit meaning or arithmetic policy,
name authorized establishment routes, or alias other domains. These roles are
checked separately. The [domain specification](../spec/language/domains.md)
owns the complete rules; this chapter shows how to use them.

Domains add no runtime tag or hidden storage. Their predicates do not execute
merely because a value is qualified. Validation and conversion are ordinary
machines with their own runtime work and contracts.

Every data declaration also has a default domain, formed by its field
constraints and `where` facts. A zeroed layout becomes an accessible value only
when those obligations hold; otherwise it is gated until established. See
[zero initialization](#declarations-and-the-zero-value) below and
[invariant windows](chapter_11_invariant_windows.md).

```omega
data Player {
    health: i32;
    in_cutscene: bool;
}

domain Player::Valid
    requires self.health >= 0
          && self.health <= 100;

domain Player::Dead
    requires self in Player::Valid
          && self.health <= 0
          && self.in_cutscene == false;

domain Player::Alive
    requires self in Player::Valid
          && self.health > 0;
```

`self` is the value being classified. A domain's `requires` clause states its
predicate obligations. They do not create fields and they do not execute at
runtime. Each row is a proof formula; naming an ordinary Boolean-returning
machine supplies neither evidence nor a general mathematical predicate. A total
pure call may occur as a denotational term under
[proof-contract rules](../spec/proofs/contracts.md#total-arithmetic).

Runtime validation is an ordinary separate machine. Its checked guarantee may
prove a structural predicate, but validation reach, effects, failures, and
authority remain on that machine rather than being absorbed into domain
identity.

This chapter assumes Chapter 7's contract model already exists. Domains do not
replace contracts; they give contracts reusable semantic names.

### Closed indexed domain families

A domain family may bind its carrier, invariant type indices, and proof-static
const parameters explicitly:

```omega
domain<T, const U: Unit> T::Quantity<U>;

domain<P, T> Extent::Resident<P, T>;
```

Bindings select closed canonical indices, such as
`i64 in Quantity<Units::METER>`, or use direct binders in a generic signature.
Type indices use exact normalized type identity and are invariant:
`Resident<PlanA, Header>` has no implicit relationship to
`Resident<PlanB, Header>`. Open type indices substitute structurally at generic
calls. Structurally equal const values have one semantic identity even when a
record literal spells its fields in a different order; different values are
different domains and cannot flow into one another implicitly. Every index is
retained in checked and terminal identity while the carrier remains unchanged
at runtime. Explicit qualification selects closed canonical indices or direct
binders without runtime work. Generic-carrier families such as
`T::Quantity<U>` and fixed-carrier families such as
`Extent::Resident<P, T>` are both permitted; every binder of the latter is an
invariant index, while `Extent` remains the exact runtime carrier:

```omega
machine retag<const To: Unit>(value: i64) -> i64 in Quantity<To> {
    transition { _ -> (value as i64 in Quantity<To>) }
}
```

Closed applications specialize the exact type and const indices. Computed
indices follow [static identity](../spec/language/domains.md#indexed-families),
not arbitrary runtime computation. One declaration owns the family's route set;
applying the family substitutes indices without adding routes.

## Domains In Contracts

Machines and states can require or guarantee domains.

```omega
machine PlayerSystem::respawn(
    player: &mut Player
)
    requires player in Player::Dead
    ensures player in Player::Alive
{
    player.health = 100;
}
```

This is shorthand for a named bundle of proof obligations. The caller must
prove `player in Player::Dead` before entering `respawn`. The machine must
prove `player in Player::Alive` before completing or transitioning to a target
that requires that domain.

Receiver state uses the same model:

```omega
data Game {
    phase: GamePhase;
    turns: u64;
}

domain Game::NewGame
    requires self.phase == GamePhase::NewGame
          && self.turns == 0;

domain Game::Playing
    requires self.phase == GamePhase::Playing;

machine Game::start_game(&mut self)
    requires self in Game::NewGame
    ensures self in Game::Playing
{
    self.phase = GamePhase::Playing;
}
```

## Establishing And Qualifying Domains

A domain declaration states two independent kinds of establishment evidence:

- `requires` contains propositions about `self`; all of them must be proved.
- `established by` contains exact trait-requirement or machine-declaration
    identities authorized to establish membership. Its comma-separated entries
    are alternative routes.

For a predicate-only domain, proving every `requires` row establishes the
structural qualification. For a domain with `established by`, those proofs are
only side conditions: an exact authorized route occurrence must also introduce
or forward the provenance. Copyable proof cannot mint Type-side authority.

```omega
domain [u8]::Path
    requires no_nul(self) == true;

pub domain Reservation::Issued
established by Issues::issue;

domain Reservation::Confirmed
    requires has_seat(self) == true
    established by Confirms::confirm;
```

A carrier-qualified domain is still an independently nameable declaration.
It is private unless marked `pub`; it does not inherit visibility from its
carrier. Conversely, publishing a domain does not publish a private carrier,
and any public signature that names both must be able to name both.

The `established by` clause does not execute its targets. A requirement route
authorizes valid selected conformers through that requirement; an exact-machine
route authorizes only that declaration's invocation. Normally the established
subject is the result. A matching non-`self`
parameter may instead be introduced when the requirement is invoked as an
installed external root. Every predicate obligation is checked at the
established subject once; later uses consume the resulting guarantee rather
than re-proving it.

Because an `established by` entry carries no call signature, its target path
must resolve uniquely. Ambiguity across overloads or declaration kinds rejects;
an expected result or selected conformance cannot choose the target. Adding an
overload can therefore break establishment clauses in other packages. The
signature-free addressing rule retains whether the target is a requirement or
an exact machine, rather than treating a satisfier as its requirement.

A public domain can keep issuance private. For example, if `issue_reservation`
names a private checked machine accessible to the domain author:

```omega
pub domain Reservation::Issued
established by issue_reservation;
```

Its authorized result occurrence supplies provenance only after the carrier,
predicates, and result/custody obligations are independently established. It
cannot assume `Issued` to prove those obligations. Naming this concrete machine
does not authorize other implementations of a trait it satisfies. A private
trait requirement is another valid route for a public domain; its visibility
prevents outside conformers. An open public trait deliberately permits them.

The public interface retains exact private issuer identities for verification,
not for downstream calls, conformance, or private-type access. A public wrapper
may return an issued value without becoming a new issuer. Private is not secret,
and neither this metadata exception nor an exact-machine route hides admissions
or mints resource capacity. Public carriers, predicates, and ordinary signatures
still obey their visibility rules.

The same parameter remains an ordinary precondition when checked code calls
the requirement directly. Direction comes from the installed external-root
occurrence, not from `boundary` and not from another source marker:

```omega
pub boundary trait InterruptEntry {
    machine enter(acknowledgement: InterruptAcknowledgement in Pending)
    reaches <= MachineControl + PortIo;
}

pub domain InterruptAcknowledgement::Pending
established by InterruptEntry::enter;
```

The same distinction matters for resource capacity: an ordinary call must
receive an existing claim; an installed occurrence may introduce only its
verified finite capacity. Implementing or selecting the requirement is not
itself the introduction event. The [content-custody contract](../spec/resources/content_custody.md#installed-introduction-schemas)
owns occurrence cardinality, lifecycle leases, and cohort establishment.

A predicate-free, route-free declaration has no added membership obligations:

```omega
domain i32::Km;
```

Every bare `i32` may therefore be explicitly qualified as `i32::Km`. This is
not a package-owner privilege. The same declaration means the same thing in
every package.

A route changes that rule. Neither the domain-owning package nor any other
code may manufacture `Reservation::Issued` with `as`; establishment must pass
through one of the exact routes named by the domain. Trait visibility
controls who may conform, machine visibility controls who may invoke the
selected machine, and a boundary requirement additionally requires provider selection
and admission. Public ordinary conformances are allowed when the domain author
deliberately publishes an open checked route.

`established by` belongs to the domain. A machine's `satisfies` clause realizes
the named requirement; an external `via` payload supplies its foreign binding.
These are separate relationships; naming an exact-machine route requires no
artificial trait or additional `satisfies` declaration.

Some compiler-owned domain classifications add closed semantic laws without
changing representation. A progress profile is explicit:

```omega
domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant_weak_fair;
```

[Progress profiles](../spec/language/termination.md#opaque-progress-profiles)
are owner-classified, predicate-free routed domains whose admitted receipts may
discharge progress premises. An empty predicate or a provider route alone does
not infer that classification.

Membership may also propagate from an existing qualified value or through a
checked evidence-preserving transformation. A qualified result type or
`ensures` clause is an implementation obligation rather than evidence by
itself unless it is the result of an authorized route. Likewise, a qualified
parameter is normally a caller obligation; it becomes introduced evidence only
at an installed external-root occurrence of its authorized route.

A parameter declared `value: T::D` imposes an implicit
`requires value in T::D` at every call boundary. Predicate-only `D` discharges
through proof; a routed `D` requires retained establishment evidence. The
callee may then treat the immutable parameter as qualified and forward that
fact. Matching the runtime representation of `T` never satisfies a routed
obligation by itself.

For a graph machine this obligation belongs to the exact state declaring the
parameter. A qualification introduced after machine entry may therefore cross
later constrained states without becoming a prerequisite of the machine's
outer entry call. Every transition into those states still proves the
state-local obligation.

### `as`

`as` is one explicit, compiler-derived coercion and erasure surface:

> **`as` never silently changes denotation: qualified targets preserve it;
> an explicitly bare target erases non-owning semantic meaning.**

It may change representation when the compiler derives one unique exact
transformation intrinsic to the carrier types. It never selects or invokes
arbitrary user code or discovers a domain-specific conversion.

| Axis | Requirement |
|---|---|
| denoted value or referent | unchanged |
| proof | predicates and representability discharged before lowering |
| reach and control | no service reach, allocation, suspension, failure, or user code |
| policy | no hidden loss, rounding, saturation, trapping, or ambiguous choice |

This includes value-preserving width changes, proven exact narrowing, and
adding an obligation-free domain. Unit conversion is an ordinary named library
machine or heterogeneous operator conformance, not an intrinsic `as` route.

```omega
let distance: i32::Km = 5;
let widened: u16 = byte as u16;
let narrowed: u8 = bounded_word as u8;
```

The last conversion is accepted only when representability is proved. Lossy,
fallible, allocating, policy-bearing, unit-scale, or otherwise domain-specific
transformations use named machines.

For a predicate-only domain, `value as T::D` succeeds only when the prover
discharges every proposition in its `requires` clause. `as` never performs
validation. For a routed domain, `as` cannot fabricate provenance.

Examples:

- `bytes as [u8]::Path` requires a proof of `no_nul(bytes) == true`;
- `5 as i32::Km` is direct qualification because `Km` states no obligations;
- `small as u32` succeeds only when representability is proved;
- `&card as &dyn PowerOrder` proves the named conformance fits and
  packages the same referent with its local dispatch table;
- `reservation as Reservation::Issued` fails when issuance requires
  `BoxOffice` state;
- `extent as Extent::Granted` fails when authority requires an admitted root
  or a conserved predecessor; and
- `distance as i32` explicitly erases its non-owning unit meaning; converting
  kilometres to metres remains a named unit-library machine.

An `as` lowering may emit a bounded intrinsic instruction such as
zero-extension or construct a fat reference. This is packaging, not an
invocation of user code. A narrowing numeric conversion with no proof is
rejected; truncating, saturating, or trapping behavior uses an explicitly
named operation.

### Qualification, erasure, validation, and conversion

An explicitly bare target erases non-owning semantic meaning:

```omega
let raw: i32 = distance as i32;
```

That erasure is never implicit. A direct cast from `i32::Km` to
`i32::Degrees` rejects because no denotation-preserving relationship exists;
the conspicuous two-step `distance as i32 as i32::Degrees` explicitly erases
and then relabels.

Weakening is checked per domain atom:

- predicate-only facts may weaken implicitly;
- semantic or non-owning provenance facts require explicit `as` erasure;
- a domain with both predicates and a route follows the stronger routed rule;
  and
- an owned claim cannot be cast away and must be consumed or transferred.

The last rule comes from ownership and custody, not merely from the presence
of a route. A non-owning historical fact may be explicitly forgotten; a live
`Extent::Granted` claim remains accountable.

| Operation | Denotation | Runtime behavior |
|---|---|---|
| exact coercion with `as` | preserved | compiler-derived carrier work only |
| explicit non-owning semantic erasure | discarded visibly | none |
| predicate weakening | preserved, fact forgotten | none |
| representation recast | same bits under its validated plan | none |
| validation | establishes a proposition | ordinary checked work |
| noncanonical conversion | operation contract defines it | ordinary named machine |

### Evidence and receipts

An admitted boundary may establish a routed qualification when it satisfies
one of the exact requirements named in the domain declaration. Provider
selection and admission produce the evidence. That evidence records external
trust; `boundary` remains on the machine or requirement where the crossing
occurs. Any predicate requirements on the same domain are proved at the
exact established subject.

A boundary receipt records the exact authorized requirement, established subject,
provider, and invocation. A checked adapter does not acquire that admitted
provenance on an ordinary direct call. Checking consumes the boundary contract
before execution redirects to its selected implementation.

For owned content, proving returned geometry is different from admitting fresh
issuance over external backing. Downstream containment and conservation are
checked relative to the disclosed premise. See [authority establishment](../spec/resources/authority.md)
for that trust boundary; a matching representation supplies no receipt.

### Transparent predicate aliases

A predicate alias gives a public name to a nonempty conjunction of compatible
facts:

```omega
pub domain Socket::Usable =
    Socket::Connected & Socket::Authenticated;
```

The alias expands before normalization and identity, so it names the same facts
as the explicit intersection. It grants no new evidence. Aliases can also bundle
compiler-owned carry permissions. Expansion must be acyclic and legal to expose;
see [aliases and identity](../spec/language/domains.md#aliases-and-identity).

### Weakening

Weakening is evaluated independently for each domain atom. An `i32::Km &
Positive` may implicitly shed `Positive` when an `i32::Km` is expected, while
`Km` prevents the same value from implicitly becoming bare `i32`.

A predicate-only atom weakens implicitly because forgetting a proved
proposition cannot make the carrier invalid. A semantic atom or a non-owning
provenance atom requires an explicit `as` to the target without that atom.
When one declaration carries both predicates and an establishment route, the
route governs removal. An owned obligation cannot weaken or cast away; custody
requires consumption or transfer.

### Domains in result overload selection

Explicit named machines and requirements may overload one path and parameter
signature by their normalized result-domain set. Predicate knowledge alone
never selects such an overload. Semantic-role contributions, routed
provenance, and empty explicit tags do; a mixed domain selects by its identity
and still owes its predicates after selection. This partition is derived from
the existing normalized domain record and requires no source marker.

The expected result's dispatch-bearing set must equal the declaration's set.
Implicit predicate weakening and proof entailment run only after selection;
semantic or provenance weakening never helps resolution. Declarations with the
same parameter signature and dispatch set are duplicates even when one return
type names extra predicate-only domains. Chapter 3 specifies the lookup and
identity rules. Fixed operator spellings remain selected from their operands.

Runtime encoding validation is an ordinary checked machine. Its successful
result carries the established domain; `as` does not insert a scan. The
[text example](#establishing-the-domain-construction-validation-and-the-wire)
below separates validation from qualification.

### Declarations And The Zero Value

A declaration may name a domain that excludes zero. The storage can still start
zero-filled, but the value is gated until its facts are established:

```omega
data Config {
    level: u8 [0..=100];
    rank: u8 [1..=9];
}

let ready = Config { rank: 1 };  // omitted level is zero-valid
```

Construction must supply gated fields and prove the whole default domain.
Machine-owned storage likewise establishes gated fields before observation.
Gating propagates through embedded values and arrays; a zero-valid first sum
case need not establish the unused payloads of later cases. See
[default domains](../spec/language/dependent_values.md#default-domains-and-zero-initialization).

## Domains And Ordinary Validity

Domains classify values that are valid for their type.

```omega
data Player {
    health: i32;
}

domain Player::Valid
    requires self.health >= 0;

domain Player::Alive
    requires self in Player::Valid
          && self.health > 0;

domain Player::Dead
    requires self in Player::Valid
          && self.health == 0;
```

The type definition defines ordinary `Player` validity. Domains name semantic
subsets inside that valid space. A domain may include another domain with
`self in Type::Domain`, which imports that domain's proof facts instead of
duplicating them.

Membership never licenses a value outside its carrier's ordinary validity.
Well-formed domains may have no satisfying values, even when the contradiction
is obvious to the compiler:

```omega
domain i32::Impossible
    requires self > 0 && self < 0;
```

This is a legal uninhabited domain, not a paradox or an assertion that an
impossible integer exists. A generic specialization may likewise have no members;
declaring the classification does not require a nonemptiness proof.

A signature may mention this domain. A machine taking such a qualified argument
reasons under a hypothetical entry contract, but a caller must prove membership
for its actual argument. The target qualification cannot prove itself. A failed
establishment may report only that membership could not be proved; the compiler
need not solve satisfiability or diagnose every contradiction. Proved
uninhabitedness may inform diagnostics, not invalidate the declaration.

Do not confuse uninhabited domains with predicate-free domains. A predicate-free
domain with authorized establishment routes certifies provenance; one without
routes permits explicit qualification of any compatible valid carrier value.
Neither rule changes. See [uninhabited domains](../spec/language/domains.md#uninhabited-domains)
for the declaration and proof rules.

An invariant window does not establish membership; actual contents remain
available to ordinary flow reasoning, but consumption requiring the domain
must wait until its obligations hold again.

## Domain Patterns

Some operations naturally produce one of several semantic states.

```omega
machine Game::apply_move(
    &mut self,
    pos: BoardPos
) -> MoveResult
    requires self in Game::Playing
    ensures self in Game::Playing | Game::Finished
{
    // Schematic: the checked move implementation is omitted.
}
```

`self in Game::Playing | Game::Finished` means the compiler knows `self` is in
one of those domains after the machine, but not which one until control flow
splits again.

Callers split that union by matching the value against type-qualified domain
patterns:

```omega
match game {
    Game::Playing -> continue_game()
    Game::Finished -> show_result()
}
```

Matching a data value with `Type::Domain` means "check whether this value is in
that domain." The selected arm receives the domain's facts in its proof
context.

Domain patterns can be interleaved with ordinary data patterns and guards:

```omega
match player {
    Player::Dead -> respawn(player)
    Player { beans, .. } if beans > 69 -> handle_beans(player)
    Player::Alive -> continue_playing(player)
    _ -> report_invalid_player(player)
}
```

This is an ordered match like Rust's `match`: earlier arms win. That means
overlapping domain patterns are allowed in ordinary value matching because the
source order is part of the program.

## Sub-Domains

A domain's predicate requirements are its classifier facts; there is no separate
classifier declaration. A domain that participates in
matching is tested through its predicate requirements; a leading cheap field
comparison remains cheap without another declaration.

Refinement is expressed structurally, by nesting the name: `A::B::C` is a
**sub-domain** of `A::B`, and its predicate requirements auto-include the
parent's facts.

If the parent is routed, its authorized provenance is still required. A nested
name cannot make that membership available from predicate checks alone.

```omega
domain Game::Playing::RoundStart
    requires self.turns == 1;
// Includes Game::Playing and turns == 1.
```

To test a subdomain, the compiler tests the parent first, so a cheap parent (`phase == Playing`) gives a tag-switch and an
expensive one (a byte scan) is paid honestly — the cost follows the facts, not a
keyword.

```omega
match game {
    Game::Playing::RoundStart -> opening()   // tests Playing, then turns == 1
    Game::Playing             -> mid()
    Game::Finished            -> over()
}
```

`A::B::C` is single-parent (one name path). A domain that refines two unrelated
parents still writes the explicit intersection in `requires` (`self in X & Y`) —
the name path is for the common refinement chain, `&` for the DAG cases.

A domain pattern is executable when its predicate requirements are pure, finite, and
runtime-checkable:

```omega
if player in Player::Dead {
    respawn(player)
}
```

This lowers to the body's comparisons and narrows the true branch with `player
in Player::Dead`. Domains with quantifiers, opaque proof calls, or
non-executable facts cannot be used as runtime checks.

A match over a known union must cover it; ordinary value matches are ordered.
Each arm receives its selected facts and proves its transition target's
requirements. Predicate tests cannot create routed provenance: that must
already be established. See [executable membership](../spec/language/domains.md#refinement-and-executable-membership).

### Named Predicates (horizontal reuse)

Sub-domains reuse facts *vertically* (a refinement chain). To reuse a fact
*horizontally* — a shared condition across unrelated domains — name a pure
bool-returning machine and call it. There is no separate `predicate` binder; a
predicate is an ordinary total machine with an ordinary brace body:

```omega
machine in_span(g: Game) -> bool {
    g.turns in 1..=9
}

domain Game::Opening
    requires self.phase == GamePhase::Playing && in_span(self) == true;
domain Game::Sudden
    requires self.phase == GamePhase::Playing
          && in_span(self) == true
          && self.turns == 9;
```

The distinction: a **sub-domain** is a named membership set with identity (you
`match` / `as` / `require` it); a **named predicate helper** is an ordinary machine supplying a reusable
condition, not another domain identity (a helper like `in_bounds`). Use the first for a
meaningful state, the second for a shared fact-bundle. They compose — a
sub-domain `requires` clause may call named predicates, and a predicate may reference
membership.

## Overlap And Intersections

Domains may overlap when they are just proof facts.

```omega
domain Password::Valid
    requires self.len >= 12 && self.has_symbol;

domain Password::Secure
    requires self.entropy_bits >= 80;
```

A value can be both:

```omega
requires password in Password::Valid & Password::Secure
```

Overlapping domains are fine in ordered value matches:

```omega
match password {
    Password::Secure -> accept_strong(password)
    Password::Valid -> accept_basic(password)
    _ -> reject(password)
}
```

Here `Password::Secure` wins when both domains hold because it appears first.
If source code needs an unordered, exhaustive split of a known domain union,
the domains must still be distinguishable by mutually exclusive bodies.

## Domain-Sensitive Operators

Domains may contribute proof facts and independently contribute semantic
meaning. Semantic contributions are keyed by compiler-known roles so
compatible orthogonal meanings compose while competing meanings reject.
Composition must still select one checked operator meaning; it never means
running unrelated overloads in an arbitrary order.

The intuition is that operators are shorthand for resolved semantic
operations. If a value's declared qualifications contribute a `+`, `-`, or
similar operator meaning, the compiler resolves the operator through those
domain contracts. **Activation is a property of bindings, not of values and
not of proof state**: a binding declared, explicitly qualified, or
`requires`-qualified into `Degrees` resolves `+` through Degrees within its
scope; a plain `i32` binding never does, regardless of what has been proven
about the value it holds.

For example, a package may choose a canonical representation for cyclic
degrees:

```omega
domain i32::Degrees
    requires self >= 0 && self < 360;

operator + add(
    left: i32::Degrees,
    right: i32::Degrees
) -> sum: i32::Degrees
    ensures degree_sum(left, right, sum) == true;
```

The literal `+` in the declaration head binds the fixed token. The descriptive
path and normalized signature remain the operator's canonical identity.

`45 as i32::Degrees` is accepted when the prover discharges the
predicate. An arbitrary runtime integer uses an ordinary checked machine such
as `Degrees::normalize(raw)`, which performs Euclidean reduction and
guarantees the predicate afterward; `as` never performs that normalization.
`normalize` is a package-authored machine. Here `degree_sum` denotes an ordinary
pure, total Boolean helper specifying this decidable relation; its body is
omitted. The contract explicitly requires its result to equal `true`.
Neither name is compiler-known or a special formula declaration.

The domain declaration does not synthesize `+`. The named operator publishes the
semantic contract, including the relation between its operands and result.
Its checked definition or selected satisfier must prove both that relation and
the result qualification. Merely returning some value in `[0, 360)` would not
establish degree addition.

The operand bounds also make the carrier step simple: an unreduced sum lies in
`[0, 718]`, so the operator can prove its primitive `i32` addition Exact before
reducing modulo 360. A realization reached with `Wrapping` operands may explicitly
erase that policy with `as i32` for the proved-safe Exact carrier step. The qualification does not add
a second `+` overload. Machine-width overflow is unreachable in this
realization, so Wrapping and Exact happen to agree for this operation; angular
reduction modulo 360 remains the domain operator's job.

This stays strict:

- Only binding-site selections (declaration, explicit qualification, `requires`) participate in
  operator resolution; flow-established membership never does.
  `if x in Degrees { x + delta }` proves the range fact — the `+` stays
  ordinary exact addition.
- Resolution reads the complete static operand-domain tuple and must be
  unambiguous; competing meanings are compile errors, never ranked.
- Compatible semantic contributions in different roles compose.
  `Km & Wrapping` combines dimensional meaning with overflow behavior. A
  domain operator whose contracts cannot compose with the selected arithmetic
  policy must provide an explicit combined meaning or reject. Two
  contributions to the same role, such as `Wrapping & Trapping`, reject.
- Adding a qualification to an operand binding can expose a new ambiguity,
  which is a loud error; adding flow proof knowledge cannot change operator
  meaning at all.
- No hidden runtime tag is introduced for dispatch.

This is especially attractive for semantic abstractions such as encoded byte
sequences and quantities. For example, `[u8]::Utf8` and `[u8]::NoNul` may want
the `+` spelling to resolve through concatenation while preserving whichever
domains the operation can soundly guarantee.

## Operator Definitions And Domain Contexts

A fixed operator token names an ordinary operator declaration with a signature
and proof contract. Its qualified name or unambiguous declared-domain operands
supply its semantic home. The selected domain does not grant establishment
authority.

Resolution is static over the complete operand-domain tuple. Inactive same-carrier
operators may coexist; participating competing meanings reject. An unrelated
import cannot alter an existing expression's meaning. [Operator-family ownership](../spec/language/expressions.md#operator-families)
and fixed-token declaration rules are separate from proving a domain predicate.

Arithmetic-policy erasure is explicit: `value as i32` selects Exact arithmetic
for later operations without undoing earlier wrapping. In contracts, every
operation must be total. Use `embed(value)` for its mathematical integer payload,
not as an executable conversion. See [total arithmetic](../spec/proofs/contracts.md#total-arithmetic).

Normalization determines identity; entailment proves facts about it. Reordering
or repeating conjunction atoms does not change the normalized type. Proving
compatibility between indexed applications does not rename their identities.
See [domain identity](../spec/language/domains.md#aliases-and-identity).

## Domains On Strings And Encodings

Text combines a byte container, an encoding-validity domain, and ordinary codec
operations. The container owns the layout; the domain states what the bytes
mean and which operations preserve that meaning.

```omega
&[u8] in Utf8         // borrowed encoded bytes
Vec<u8>::Utf8         // owned, allocator-backed encoded bytes
[u8; 64] in Utf8      // fixed-length encoded byte array
```

A quoted literal is raw bytes, not automatically text. An explicit qualification
must prove the selected encoding predicate. Known literal bytes may make that
proof possible at compile time; runtime input needs a validator's checked
contract. Encodings are library subjects, not a compiler-known `String` type or
an intrinsic UTF-8 privilege.

A byte-domain declaration is independent of a particular bounded capacity.
Two capacities may use the same normalized domain name when their fact sets
agree; conflicting meanings reject. The [encoding-domain contract](../spec/language/domains.md#byte-containers-and-encoding-domains)
separates that semantic identity from storage capacity.

### This is not "hide the byte operations"

Reading a byte from an encoded view is ordinary byte access. Preserving UTF-8
while slicing requires codepoint-boundary endpoints; mutation likewise must
preserve or re-establish validity. The contracts belong on those operations:

```omega
operator [] slice(s: Slice<u8>::Utf8, range: Range) -> Slice<u8>::Utf8
    requires char_boundary(s, range.start) == true
          && char_boundary(s, range.end) == true;
```

This is an illustrative requirement signature. A concrete checked implementation
must prove the relation; naming `char_boundary` supplies no proof. An operation
that intentionally drops the encoding can expose an unqualified byte view.

### Proving it without a byte-level tax

Validate once at ingestion, then preserve the fact through checked operations.
For example, concatenation of valid UTF-8 and slicing at proved boundaries can
publish UTF-8 guarantees. Callers use those guarantees rather than rescan.
Writes invalidate affected facts unless their contracts preserve or re-establish
them. Domain annotations never bless arbitrary incoming bytes.

The executable validator is an ordinary total machine. Its recursive or state
walk must prove termination and bounds like any other code. It returns a success
carrying qualified bytes or an ordinary failure; proof search failure itself
is not a runtime validation outcome.

### Establishing the domain: construction, validation, and the wire

There are two useful establishment patterns:

- Construction proves known bytes meet the predicate. A bounded carrier also
  proves live byte length fits capacity, including in arguments and results;
  it never truncates. A raw fixed array instead has an exact-length rule.
- Runtime validation checks previously untrusted bytes and returns the fact
  through its successful outcome. Multiple classifications use a sum:

```omega
data Decoded {
    case Ascii(text: &[u8]::Ascii);
    case Utf8(text: &[u8]::Utf8);
    case Invalid(error: DecodeError);
}
```

A classifier's checked result contract establishes the case payload's domain.
Matching imports that fact; it does not validate again. Returned views retain
ordinary borrow lifetimes and cannot outlive their input backing.

Wire decoding ordinarily establishes structure and raw bytes, not encoding
validity. A schema can explicitly request validation and report an invalid
encoding as a decode error. This is an opt-in under the
[codec contract](../spec/layouts/codecs.md#boundary-establishment), not trust
granted by transport. Abstract codepoint text, when needed, is a mathematical
[quotient](../spec/proofs/quotients.md), not a mandatory storage wrapper.

## Domains On Foreign Types

A package may declare a domain over another package's carrier, for example
`domain Entity::Quarantined requires ...;`. The carrier owner need not approve
that independent classification. It cannot change the carrier's invariants,
expose private state, or alter another domain's minting routes.

Use ordinary explicit imports. Importing the declaring module exposes its
directly declared public domains in this source file; importing one domain
exposes only it. No exposure spreads from sibling files, descendant modules, or
transitive imports. Importing a carrier or one machine does not expose sibling
domains merely because their source was loaded.

An exact import such as `use ui_policy::screen::Point::OnScreen;` selects the
domain owned by `ui_policy::screen`; its `Point` attachment was resolved in the
declaring context to the exact carrier. Direct qualified selection exposes no
sibling domains. Caller aliases cannot change the carrier or domain owner.

Distinct visible declarations with the same carrier-qualified name reject,
including carrier-owned declarations; neither inherent ownership nor import
order wins. Repeated imports of one exact domain are not a collision. Broad
imports can encounter new collisions as a module grows; narrow imports avoid
unrelated additions. Diagnostics identify the owners and exposing imports.

Importing enables selection, not membership or authority. A returned value may
carry a foreign qualification and its evidence without importing it for authored
lookup or exposing the package's other domains. See
[foreign declarations](../spec/language/domains.md#visibility-and-foreign-declarations)
and [import scope](../spec/language/modules.md#import-scope-and-exposure).

## No Hidden RTTI

A domain adds no hidden runtime tag. When classification needs a tag, write an
ordinary field whose type represents it:

```omega
data GamePhase {
    case NewGame;
    case Playing;
    case Finished;
}
```

Domains can classify a game through that field. The data declaration owns its
layout; the domain and machine contracts own what may be proved about it.
