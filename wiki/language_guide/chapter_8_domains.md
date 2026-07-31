# Chapter 8: Domains

A domain is a zero-cost semantic theory attached to a value's unchanged
carrier.

> **Current domain model
> ([domain_facets_and_qualification.md](../design_briefs/domain_facets_and_qualification.md)).**
> One domain declaration may contribute predicate requirements, semantic
> declarations, exact authorized establishment routes, and transparent alias
> expansion. Those aspects share a source name because they compose, but the
> compiler stores and checks them independently.
>
> `Utf8` has predicate requirements, `Km` contributes denotation and unit
> operations, `Wrapping` contributes an arithmetic policy, `Percent` may carry
> both a range predicate and unit meaning, and `Reservation::Issued` is a
> routed historical fact.
>
> Flow inference changes what is known. Static binding qualifications determine
> which semantic roles participate in operator resolution. Multiplicity governs
> copy/discard obligations; qualified claim content governs decomposition;
> permissions govern operations; and carry governs mobility.

Domains are not runtime tags, wrapper types, hidden storage, or a second
object model. Domain evidence adds no runtime metadata. Exact `as` coercions
may change representation while preserving denotation; validation and
noncanonical conversion remain ordinary operations and may perform runtime
work.

> **Every data type has a DEFAULT DOMAIN.** The invariants a
> `data` type "always has" are its default domain — the one domain that is always in
> scope for the value and travels with it everywhere, so it need not be named or
> tracked. Named domains like `Player::New` or `Quantity::Additive` are **subdomains**
> refining that base with tighter invariants, operators, or facts, selected at a qualification
> point (`as`) when provable. This is why a per-field constraint and a domain are the
> *same* mechanism (see [Chapter 7](chapter_7_types_constraints_invariants.md)):
> single-field constraints are standing invariants of the default domain; cross-field
> invariants live there too, with stores the checker cannot prove domain-preserving
> carried as [invariant windows](chapter_11_invariant_windows.md) until the next
> consumption point. The first implementation slices now
> cover declaration clauses, exact construction gates, range sugar, propagated
> zero-validity, standing scalar bounds, direct/nested/indexed windows, and
> witness-pinning loans, flow-proven local construction, and byte-predicate
> domain facts through construction and windows. Versioned expression
> provenance discharges identical and affine correlations across construction
> and adjacent writes; `.len` measure facts cover zero values, literals, place
> copies, construction, and writes. Other operator facts and broader relational
> discharge remain open.

Zero expressibility and value establishment are separate. Storage for every
data layout may be zero-filled, but a default-domain fact or field range that
excludes zero gates when those bytes become an accessible value. It does not
make the declaration illegal. Construction must explicitly initialize omitted
gated fields, and machine-owned zero storage must establish every gated field
before its first read or other consumption point.

The gate composes structurally through embedded data, fixed-array elements,
common sum fields, and the payload of the zero-tag (first) case. Establishing a
nested child contributes to establishing its parent; a debt-free zero case may
honestly absorb gates carried only by later cases. Whether zero establishes the
whole value is derived from this structure and cannot be asserted by a type
property. Semantic emptiness or reset behavior is an ordinary domain fact
established by authored machines.

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
predicate obligations. They do not create fields and they run only when the
program explicitly asks for a runtime diagnostic/checking build.

This chapter assumes Chapter 7's contract model already exists. Domains do not
replace contracts; they give contracts reusable semantic names.

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
    board: Board;
    winner: Optional<PlayerId>;
}

domain Game::NewGame
    requires self.phase == GamePhase.NewGame
          && self.turns == 0
          && self.board.empty
          && self.winner == None;

domain Game::Playing
    requires self.phase == GamePhase.Playing
          && self.winner == None;

machine Game::start_game(&mut self)
    requires self in Game::NewGame
    ensures self in Game::Playing
{
}
```

## Establishing And Qualifying Domains

A domain declaration states two independent kinds of establishment evidence:

- `requires` contains propositions about `self`; all of them must be proved.
- The body contains exact trait-requirement identities authorized to originate
  membership. Each body entry is an alternative establishment route.

```omega
domain [u8]::Path
    requires no_nul(self);

pub domain Reservation::Issued {
    Issues::issue;
}

domain Reservation::Confirmed
    requires has_seat(self)
{
    Confirms::confirm;
}
```

The body does not execute those requirements. It authorizes their selected
conformances to establish the domain at the requirement's qualified return
position. Every predicate obligation is checked there once; callers consume
the resulting guarantee rather than re-proving it.

An empty declaration has neither predicate nor provenance obligations:

```omega
domain i32::Km;
```

Every bare `i32` may therefore be explicitly qualified as `i32::Km`. This is
not a package-owner privilege. The same declaration means the same thing in
every package.

A route changes that rule. Neither the domain-owning package nor any other
code may manufacture `Reservation::Issued` with `as`; establishment must pass
through one of the exact requirements named by the domain. Trait visibility
controls who may conform, machine visibility controls who may invoke a
conformer, and a boundary requirement additionally requires provider selection
and admission. Public ordinary conformances are allowed when the domain author
deliberately publishes an open checked route.

Membership may also propagate from an existing qualified value or through a
checked evidence-preserving transformation. A qualified result type or
`ensures` clause is an implementation obligation rather than evidence by
itself unless it is the return of an authorized route.

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

It may change representation or the carrier's stored numeral when the compiler
derives one unique exact transformation from normalized type and domain
semantics. It never selects or invokes arbitrary user code.

| Axis | Requirement |
|---|---|
| denoted value or referent | unchanged |
| proof | predicates, bounds, and divisibility discharged before lowering |
| reach and control | no service reach, allocation, suspension, failure, or user code |
| policy | no hidden loss, rounding, saturation, trapping, or ambiguous choice |

This includes value-preserving width changes, proven exact narrowing, adding
an obligation-free domain, and exact scale conversion between compatible unit
domains. Unit conversion reuses the same normalized dimension, kind, and scale
algebra as operator resolution; it is not a separately authored conversion
registry.

```omega
let distance: i32::Km = 5;
let meters: i32::M = distance as i32::M;
let widened: u16 = byte as u16;
let narrowed: u8 = bounded_word as u8;
```

The last conversion is accepted only when representability is proved.
`meters as i32::Km` likewise requires exact divisibility and range proofs.
Incompatible dimensions reject. Lossy, fallible, allocating, policy-bearing,
or otherwise noncanonical transformations use named machines.

For a predicate-only domain, `value as T::D` succeeds only when the prover
discharges every proposition in its `requires` clause. `as` never performs
validation. For a routed domain, `as` cannot fabricate provenance.

Examples:

- `bytes as [u8]::Path` requires a proof of `no_nul(bytes)`;
- `5 as i32::Km` is direct qualification because `Km` states no obligations;
- `small as u32` succeeds only when representability is proved;
- `&card as &dyn Card::PowerOrder` proves the named conformance fits and
  packages the same referent with its local dispatch table;
- `reservation as Reservation::Issued` fails when issuance requires
  `BoxOffice` state;
- `extent as Extent::Granted` fails when authority requires an admitted root
  or a conserved predecessor; and
- `distance as i32::M` converts kilometres to metres exactly while preserving
  the represented physical quantity.

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
| exact coercion with `as` | preserved | compiler-derived intrinsic work only |
| explicit non-owning semantic erasure | discarded visibly | none |
| predicate weakening | preserved, fact forgotten | none |
| representation recast | same bits under its validated plan | none |
| validation | establishes a proposition | ordinary checked work |
| noncanonical conversion | operation contract defines it | ordinary named machine |

### Evidence and receipts

An admitted boundary may establish a routed qualification when it satisfies
one of the exact requirements named in the domain declaration. Provider
selection and admission produce the receipt. The receipt records external
trust; `boundary` remains on the machine or requirement where the crossing
occurs. Any predicate requirements on the same domain are proved at the
route's return position.

For admitted qualification, “exact subject” means the requirement spells the
bare `result`, and the result's unqualified carrier matches the domain target.
A membership `ensures` written directly on an accepted machine is not an
establishment route; the provider inherits the guarantee by satisfying the
boundary requirement. Checked artifacts retain the authorizing trait,
requirement signature, and provider receipt independently.

A checked adapter satisfying such a requirement is executable provider code,
not a second establishment route. Calling it directly exposes only what its
checked body establishes. Calling the selected boundary-trait slot consumes the
admitted requirement and receipt; compilation preserves that semantic call
until checking is complete, then redirects execution to the selected adapter.

Predicate and authority evidence remains erased. Runtime data discovered at a
boundary, such as a firmware range's base and length, remains in the carrier.

### Transparent predicate aliases

A predicate alias gives a public name to a nonempty conjunction of compatible
facts:

```omega
pub domain Socket::Usable =
    Socket::Connected & Socket::Authenticated;
```

The alias expands before fact normalization, contract identity,
compatibility, and admission. A contract mentioning `Socket::Usable` therefore
has the same normalized fact identity as one spelling both constituent facts.
Alias definitions form an acyclic expansion graph, and every constituent of a
public alias must be legal to publish for the same subject.

Aliases add names rather than evidence. Establishing an alias establishes its
expanded facts, and ordinary fact forgetting may retain any chosen subset.
Diagnostics expand the alias and report the unmet atomic fact.

Predicate atoms may be compiler-owned while aliases remain openly nameable.
Carry uses this shape: its four atomic permissions are a closed compiler
vocabulary, while `Carry::Portable` is the standard transparent alias for
their conjunction. Type-associated packages may name additional compatible
bundles while the axis set remains compiler-owned.

Changing an alias expansion changes normalized contracts that cite it. Adding
a conjunct strengthens requirements and guarantees; removing one weakens them.
The resulting compatibility effects are reported at the affected callers,
implementations, or consumers.

> **Surface status (2026-07-28).** Declared-domain aliases are implemented
> across parsing, resolved and typed domain theory, constrained-type identity,
> proof/contract compatibility, admission, executable membership, validation,
> and atomic diagnostics. `pub` legality is retained for domain aliases:
> publishing a private declared constituent rejects. The compiler-owned
> `Carry` atoms and their standard `Carry::Portable` declaration land with the
> separate per-claim carry migration; aliases over ordinary declared atoms work
> now.

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

Establishing a domain over *runtime* data is therefore ordinary code, not a
compiler builtin. To turn untrusted bytes into `&[u8]::Utf8` you write a
machine that reads the bytes, guards that each unit is valid, and casts in the
arm where the whole sequence is proven:

```omega
data Utf8Scan {
    case NotText;
    case Text(view: &[u8]::Utf8);
}

machine Scanner::scan(&mut self, bytes: &[u8]) -> Utf8Scan {
    self.i = 0;
    transition { _ -> step(bytes) }

    state step(&mut self, bytes: &[u8]) -> Utf8Scan {
        transition self.i < bytes.len {
            true -> check(bytes)
            _    -> (Utf8Scan::Text { view: bytes as &[u8]::Utf8 })  // all units proven
        }
    }
    state check(&mut self, bytes: &[u8]) -> Utf8Scan {
        transition bytes[self.i] < 128 {                               // the invariant, guarded
            true  -> next(bytes)
            false -> (Utf8Scan::NotText)
        }
    }
    state next(&mut self, bytes: &[u8]) -> Utf8Scan {
        self.i = self.i + 1;
        transition { _ -> step(bytes) }
    }
}
```

> **Surface status (2026-07-04).** This example illustrates the settled *model*.
> Explicit `as` qualification into an **arithmetic** domain works today
> (`x as u8::Saturating`; see `expressions/arithmetic_domain_cast_exit`). The
> `as` qualification into a **reference/encoding/layout** domain shown here
> (`bytes as &[u8]::Utf8`) is the recast surface that is **not yet
> implemented** — it is pending on qualification plus the
> invariant-prover's reach). The shape above is the intended spelling, not
> currently compilable. Existing arithmetic forms that also change numeric
> width are compatibility conversion syntax rather than the final
> qualification model.

The compiler generates none of this — and generates nothing at all for domain
membership. Its only job is to accept or reject the `as`, by asking whether the
domain's invariants are proven at that point. The reach of that prover is the
ceiling on what can be qualified: a fact the prover cannot yet discharge (a
whole-buffer property established across a loop) simply cannot be cast until the
prover grows to reach it. This is the anti-serde principle at its end — the only
path from raw bytes to a trusted fact is a proof the machine actually checked.

If you want a `Valid | Invalid`-style result, you declare that sum type yourself
(`Utf8Scan` above); there is no built-in verdict type and no generated codec.

### Declarations And The Zero Value

The one place a domain appears without a written `as` is a declaration —
`x: T::D`, or a `data` field `f: T::D`. The qualification is implicit there,
but it
is still checked: the compiler proves the **ZII default** (the zero value)
satisfies `D`. A domain that excludes its zero value therefore cannot be
default-declared.

```omega
data Config {
    level: u8 [0..=100];    // ok: the ZII default 0 is in [0..=100]
    // rank: u8 [1..=9];    // COMPILE ERROR: ZII 0 is not in [1..=9] -- nothing proves the default
}
```

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

A domain may not specify facts that violate the ordinary validity rules of the
type it classifies.

```omega
data Player {
    health: i32;
}

// Invalid when the ordinary validity rules say health must stay positive.
domain Player::Dead
    requires self.health == 0;
```

An invariant window may temporarily suspend a required fact inside a machine
body, but that does not make a mid-window value a member of a domain. Domain
membership is a fact about values that satisfy the type's ordinary validity
rules — and nothing can observe a value mid-window (Chapter 11).

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

A domain's predicate requirements are its classifier facts; there is no separate classifier clause
(there is no separate classifier declaration). A domain that participates in
matching is tested through its predicate requirements; a leading cheap field
comparison remains cheap without another declaration.

Refinement is expressed structurally, by nesting the name: `A::B::C` is a
**sub-domain** of `A::B`, and its predicate requirements auto-include the
parent's facts.

```omega
domain Game::Playing
    requires self.phase == GamePhase.Playing && self.winner == None;
domain Game::Playing::RoundStart
    requires self.turn == 1;
// RoundStart ≡ { self in Game::Playing; self.turn == 1 } — the parent facts are inherited
```

Membership is a lattice and the name is the edge. To test a sub-domain the compiler tests the
parent first, so a cheap parent (`phase == Playing`) gives a tag-switch and an
expensive one (a byte scan) is paid honestly — the cost follows the facts, not a
keyword.

```omega
match game {
    Game::Playing::RoundStart -> opening()   // tests Playing, then turn == 1
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

For domain matches the compiler checks:

- A non-wildcard match over a known domain union must be exhaustive.
- Bodies must be mutually exclusive when the program relies on unordered
  domain-union reasoning (a sub-domain is never mutually exclusive with its
  parent — order it before the parent).
- Each arm receives the facts from the selected domain.
- Each transition target must accept the facts established by its arm.

### Named Predicates (horizontal reuse)

Sub-domains reuse facts *vertically* (a refinement chain). To reuse a fact
*horizontally* — a shared condition across unrelated domains — name a pure
bool-returning machine and call it. There is no separate `predicate` binder; a
predicate is an ordinary machine with an ordinary brace body:

```omega
machine in_span(g: Game) -> bool {
    g.turn in 1..=9
}

domain Game::Playing
    requires self.phase == GamePhase.Playing && in_span(self);
domain Game::Sudden
    requires self.phase == GamePhase.Playing
          && in_span(self)
          && self.turn == 9;
```

The distinction: a **sub-domain** is a named membership set with identity (you
`match` / `as` / `require` it); a **named predicate** is an anonymous reusable
condition with no identity (a helper like `in_bounds`). Use the first for a
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

operator add(
    left: i32::Degrees,
    right: i32::Degrees
) -> sum: i32::Degrees
    spelling +
    ensures degree_sum(left, right, sum);
```

`45 as i32::Degrees` is accepted when the prover discharges the
predicate. An arbitrary runtime integer uses an ordinary checked machine such
as `Degrees::normalize(raw)`, which performs Euclidean reduction and
guarantees the predicate afterward; `as` never performs that normalization.
`normalize` and the pure relational predicate `degree_sum` are package-authored
machines, not compiler-known names.

The domain body does not synthesize `+`. The named operator publishes the
semantic contract, including the relation between its operands and result.
Its checked definition or selected satisfier must prove both that relation and
the result qualification. Merely returning some value in `[0, 360)` would not
establish degree addition.

The operand bounds also make the carrier step simple: an unreduced sum lies in
`[0, 718]`, so the operator can prove its primitive `i32` addition Exact before
reducing modulo 360. A realization reached with `Wrapping` operands may weaken
them to Exact for that proved-safe carrier step. The qualification does not add
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

Operator overloading is trait-like in spirit but semantic-binding-aware in
resolution. Predicate proof state may discharge the selected operator's
contracts, but it never selects the meaning.

Rust maps a fixed operator spelling such as `+` or `[]` to a trait method such
as `Add::add` or `Index::index`. Omega has a similar semantic home for
operators, but with one extra axis: the semantic domains selected on operand
bindings determine which domain-owned meanings participate.

A fixed operator spelling is declared with an optional `spelling` clause on a
named `operator`. The named operator carries the full signature and proof
contract; the `spelling` clause only binds the surface symbol that resolves to
it.

```omega
domain Quantity;

operator add(left: Quantity, right: Quantity) -> Quantity spelling +;
```

Operators associated with a domain remain ordinary named declarations; the
domain body is reserved for establishment routes. Domain-sensitive resolution
selects among spelled candidates by
the complete operand-type tuple plus the bindings' selected semantic domains.
Competing participating meanings for the same use are a compile error; inactive
same-carrier declarations may coexist.

Decided model:

- Fixed operator spellings are declared with an optional `spelling` clause on a
  named `operator`.
- Core types such as `Slice`, `Array`, and `Vec` can expose
  operator definitions whose implementations are bound to boundary primitive
  compiler/runtime operations below the public core surface.
- User/library types can expose ordinary operator definitions when the language
  supports that surface.
- Domains may provide or select operator meanings when the value's *binding*
  is declared, explicitly qualified, or `requires`-qualified into that domain — never from
  flow-established membership.
- Resolution must be static and unambiguous, over the complete operand-domain
  tuple. Commutative flips are one-line delegations
  (`operator add(left: Metres, right: Km) -> Km = add(right, left);`).
- **Coherence.** Operator families are closed by default; an open family
  declares a designated dispatch-owner position (one owner per implementation
  key, killing independent sibling claims on the same cross-domain tuple).
  A package owning neither operand writes an explicit adapter domain.
  Candidates come only from the participating domains, their owning packages,
  and the declared family — imports are never scanned. The test: **adding an
  unrelated dependency cannot change resolution, invalidate typechecking, or
  introduce a new collision for an existing expression.**
- **Resolution is a compile-time decision recorded in the checked artifact;
  runtime dispatch never repeats domain resolution.** At swap boundaries,
  bodies swap gated by the declaration's contract and laws; resolutions never
  travel (a declaration-surface change is a version change requiring dependent
  recompilation, chapter 22 — never a silent runtime rebind).
- Operator declarations with the same name may form an overload set only when
  their parameter types differ. Return-only overloads and alpha-equivalent
  generic duplicates are ambiguous and should reject before resolution. Generic
  comparison is structural, so reordered type parameter declarations do not
  manufacture a distinct candidate.

For slices, this means the source form:

```omega
let value: Item = items[index];
let tail: &[Item] = items[1..];
```

can be modeled as calls to core slice operators whose contracts require bounds
proofs. The compiler may lower those operator bodies through internal pointer
and descriptor machinery, but the user-facing meaning belongs to `Slice`.

For domain-sensitive quantities, the same spelling resolves through the
operator selected by the complete static binding qualifications. A parameter
declared or `requires`-qualified into `i32::Degrees` therefore participates in
the `Degrees` operator family; merely proving at runtime that a plain `i32`
happens to lie in `[0, 360)` does not change its operator selection.

Ambiguity is an error:

```omega
requires value in Angle::Degrees & Angle::Turns
```

If both domains expose different `+` meanings for the same expression, the
program must choose a clearer operation or narrow the proof context before using
operator syntax.

Not every domain contributes operator meaning. The initial semantic role
vocabulary separates denotation/dimension from arithmetic policy.
`Wrapping`, `Saturating`, and `Trapping` occupy the compiler-owned arithmetic
policy role because primitive arithmetic needs direct lowering. Unit domains
occupy the denotation/dimension role. Domain-sensitive operators resolve from
static binding-site selections, not runtime type mutation or the flow-fact
environment.

Arithmetic-policy erasure is explicit. A value selected into `Wrapping`,
`Saturating`, or `Trapping` may use `as` to become unqualified, where
arithmetic is Exact by default: its current payload is unchanged, and every
later operation must prove the Exact obligations anew. This does not recover a
mathematical value lost by earlier wrapping. Selecting or removing a non-Exact
policy is explicit because it changes future operator behavior.

**Normalization is not entailment.** A small deterministic, confluent,
terminating normalizer owns what a domain expression *is* (canonical
dimension vectors, scale products, kind tags); type identity, semantic
interface identity, and monomorphization keys depend only on it. The
entailment engine proves propositions *about* expressions and can never
redefine canonical identity. Physical ABI remains the **carrier's** ABI
(representation erasure holds); semantic interface identity includes
normalized domains. Units and the full quantity model (dimension x kind x
scale x presentation) are specified in the design brief.

For the currently authored conjunction form, normalization is concrete:
declared terms resolve to their semantic-domain identity, arithmetic-policy
terms use their closed canonical identity, conjunctions are sorted and
deduplicated, and nested constraint shells flatten before identity is
computed. Thus `T::A & B`, `T::B & A`, and `T::A & A & B` are one
semantic type and one monomorphization key. `T::A` and `T::B` remain
distinct even though diagnostic renderings happen to contain the same number
of constraints. Human-readable type rendering is never an equality or cache
key.

## Domains On Strings And Encodings

Text is not a type in Omega. It decomposes into three things that already
exist, and `String`/`Bytes` are not among them:

- a **byte container** -- `&[u8]` (a view), `[u8; N]` (fixed), or `Vec<u8>`
  (owned). The only part with a layout.
- an encoding's **validity**, expressed as a *domain* over the byte container.
- an encoding's **codec** (decode/encode/boundary), expressed as
  *domain-sensitive operators*, not a predicate.

"A UTF-8 string" is therefore `[u8]::Utf8`, not a `String`:

```omega
&[u8]   in Utf8           // text view, zero-copy
Vec<u8>::Utf8             // owned text (needs the allocator)
[u8; N] in Utf8           // fixed text buffer
```

**Encodings are ordinary library code — the compiler has ZERO encoding
intrinsics (settled 2026-07-05).** `Utf8` is no more special than `Ascii`,
`Utf16`, or Shift-JIS; each is a validity *domain* over the byte container,
defined in `core`, with no compiler privilege. There is no blessed `valid_utf8`
primitive. A domain's body **is** its predicate; the compiler's only string
job is turning quoted text into bytes (copy source bytes + byte-level escapes;
no codepoint synthesis, ASCII-transparent source). Litmus: delete every encoding
from the library and the compiler must still lex and parse — it just can't
establish `in <encoding>` on anything, which is correct.

Simple, *per-element* encodings are a boolean expression directly; a *sequence*
property like UTF-8 is a pure, terminating machine over the bytes (see the
recogniser below):

```omega
domain Slice<u8>::Ascii
    requires all_below(self, 0x80);       // per-element predicate (library code)
domain Slice<u8>::NoNul
    requires no_interior_nul(self);       // per-element predicate
domain Slice<u8>::Utf8
    requires utf8_ok(self);               // sequence recogniser (below)
```

`utf8_ok` is an ordinary machine, not a builtin. Ranked recursion is legal
(settled 2026-07-18; chapter 3) and a tail-recursive spelling lowers to the
same loop — the idiomatic sequence walk remains a **state machine that narrows
the slice** — slicing over indexing, no index variable:

```omega
machine utf8_ok(b: &[u8]) -> bool {
    transition { _ -> scan(b) }
    state scan(b: &[u8]) {
        transition {
            b.len == 0          -> accept()
            b[0] < 0x80         -> scan(b[1..])                            // ASCII
            b[0] in 0xC2..=0xDF && b.len >= 2 && cont(b[1]) -> scan(b[2..]) // 2-byte
            // 3-/4-byte arms; E0/ED/F0/F4 tighten cont(b[1]) to a lead-specific
            // range to exclude overlong/surrogate encodings
            _                   -> reject()
        }
    }
    state accept() -> bool { true }
    state reject() -> bool { false }
}
machine cont(x: u8) -> bool { x in 0x80..=0xBF }
```

The bytes' bounds fall out of the arm guards (`b.len == 0` + the `b.len >= k`
checks), so the walk is memory-safe by the ordinary array-access proofs. The
same machine evaluates over a literal at compile time (to discharge an `as`) and
runs at runtime to establish membership — one definition, no separate spec.

Host and ABI boundaries then ask for the domain they actually need, with no
bespoke type per case:

- `[u8]::Utf8` for APIs that require UTF-8 text,
- `[u8]::Utf8 & NoNul` for C-style boundaries that reject interior NULs,
- `[u8]::Utf16` (a different validity + codec) for a UTF-16 boundary.

`CString`, `OsString`, `Utf16String`, `Str`/`StrView` and the like all collapse
into `[u8] in <domain>` intersections. There is no `String` type and no `Bytes`
type: each was a nominal name for either the byte container (redundant with
`[u8]`) or the *abstract codepoint text* -- and abstract text is a **quotient**
(byte-sequences modulo "decode to the same codepoints"), which has no canonical
layout and is only ever materialized as bytes-in-an-encoding, or, decoded, as a
`[u32]` of scalar values. Naming the quotient as a type is what made `String`
feel unrepresentable. Naming the container and carrying the encoding as a *fact*
removes the confusion.

### This is not "hide the byte operations"

A nominal `String` is tempting as an API-curation point -- expose only
boundary-safe operations, hide the ones that can split a codepoint. Omega does
not need it, because the fix is to *contract* the few invariant-breaking
operations, not to hide them. Reading one byte `s[5]: u8` off UTF-8 text is
always fine. The only operation that can break UTF-8 is re-slicing, so the proof
obligation lives on `slice`, in the open:

```omega
operator concat(left: Slice<u8>::Utf8, right: Slice<u8>::Utf8)
    -> Slice<u8>::Utf8 spelling +;          // concat preserves UTF-8: proven once

operator slice(s: Slice<u8>::Utf8, range: Range) -> Slice<u8>::Utf8
    requires char_boundary(s, range.start) && char_boundary(s, range.end)
    spelling [];                              // cannot cut mid-codepoint
```

"The operation exists, its contract stops misuse" replaces "the operation is
hidden" -- the proof-language form of encapsulation, and it needs no wrapper
type.

### Proving it without a byte-level tax

The hard part is not the surface idea; it is proving sequence-wide invariants
over runtime text, which is why this is staged rather than a single
byte-by-byte predicate everywhere:

- validate the encoding ONCE at the ingest boundary (running the `utf8_ok`
  recogniser), establishing `in Utf8` as a fact;
- carry that fact; never re-scan;
- prove a small set of *preservation* lemmas as operator contracts (concat
  preserves `Utf8`; boundary-`slice` preserves it) so downstream code keeps the
  fact without re-proof;
- richer sequence-wide invariants come later.

Removing the `String` type *helps* here: with no type obligated to maintain
validity through every operation, there is nothing to re-prove between the
boundary and the operators.

> Implementation note: builtin `string`/`String` and
> `PrimitiveType::String` are retired. The honest borrowed bytes/text wire field
> is `&[u8]` (the ordinary fat slice), and bounded ownership is `[u8; N]::D`.
> A byte view is variable-length, so it rides a raw-byte encoding (length varint
> plus raw bytes, like protobuf `bytes`), distinct from a `[u8; N]` repeated
> scalar field (packed per-element varints). Domains over byte views and bounded
> carriers, direct literal construction, bounded returns, wire encode/decode,
> and native/interpreter carrier lowering are built. The source corpus and
> injected build vocabulary are carrier-native; the allocator-backed growable
> surface remains ordinary future `Vec<u8>::Utf8` work rather than a reason to
> keep a compatibility primitive. Mutable boundary/operator
> statement calls already invalidate facts for their exact mutable operands and
> re-establish declared domain-membership guarantees on those caller places;
> the text canaries exercise this rule over `[u8]`, not builtin `String`.
> Ordinary state parameters follow the same place rule. An immutable data
> parameter carries the declared-domain facts of its nested fields, while a write
> through `&mut Data.field` must prove the assigned value is in that field's
> declared domain (or obtain the fact from a checked guarantee). Merely naming a
> domain on the field never blesses arbitrary bytes written through a parameter.
>
> Fixed capacities do not create distinct meanings for a domain name. It is
> therefore valid to declare the same normalized domain over several bounded
> carriers, for example `[u8; 16]::Utf8` and `[u8; 64]::Utf8`, when their
> normalized fact sets are equal. An unqualified `in Utf8` then denotes that one
> semantic domain. If repeated declarations under the same normalized name carry
> different facts, validation rejects them; declaration order never selects a
> meaning.
>
> Standard Console I/O is carrier-based: `write` and `write_line`
> borrow `&[u8]`, and the checked adapter walks that view directly. An owned
> bounded carrier projects its runtime length and inline byte address at the
> call seam, including when reached through a mutable carrier reference. A
> guard-selected literal returned as a bounded carrier is constructed in the
> result slot as `{len, inline_bytes}`, not as a borrowed descriptor.
> `read_line(&mut [u8])` accepts a concrete bounded carrier destination and
> derives its writable capacity from that call-site place before updating the
> carrier's runtime length. Growable input remains allocator-gated; fixed input
> no longer relies on builtin `String`.
> In straight-line code, bounded text writes also retain a conservative maximum
> runtime length for each place. A later `line = line + suffix` uses that
> reaching bound rather than pretending `line` is already full; overlapping
> writes invalidate the fact, and calls or opaque effects clear it. Capacity is
> therefore proved, never recovered by truncation or a runtime overflow path.
> A fixed-capacity byte destination need not claim an encoding domain when its
> consumer does not require one. Sample “press Enter” scratch fields use raw
> `[u8; 256]`: the bound is explicit and preserves the former line-read ceiling,
> while no unused `Utf8` fact is established merely because the bytes are discarded.

### Establishing the domain: construction, validation, and the wire

A byte container EARNS its encoding domain in exactly one of two ways -- and
never from the transport:

- **By construction.** A literal, or a value built from known-good bytes, is
  `[u8]::Utf8` by construction: the compiler knows the bytes, so there is no
  runtime check. The preservation operators above (`concat`, boundary-`slice`)
  carry the domain forward, so text stays text without re-validation.
  For an owned bounded carrier `[u8; N]::Utf8`, construction additionally
  proves the exact literal byte length is at most `N`. The same rule applies in
  argument and machine-result positions; there is no truncation or deferred
  capacity failure.

- **By validation -- a fallible call, riding chapter 16.** Raw bytes from an
  untrusted source are plain `[u8]` with NO encoding domain. To use them as text
  you run a `validate` operator, which is an ordinary fallible call: it returns
  the DOMAINED slice or an error, handled at a transition boundary like any other
  recoverable failure. When validation can land in more than one domain, return a
  sum whose cases each carry the ALREADY-DOMAINED slice -- the same shape as
  `Result`, and the same fact-on-a-case-payload machinery as chapter 16's errors.
  The caller matches once and gets the specific domain in each arm; it never
  re-validates.

  ```omega
  data Decoded {
      case Ascii(text: [u8]::Ascii);   // payload already carries the domain
      case Utf8(text:  [u8]::Utf8);
      case Invalid(error: DecodeError);
  }
  machine classify(bytes: &[u8]) -> Decoded { /* scans once */ }
  ```

  This is the blessed pattern, not a trick to be discovered: a multi-domain
  classification operation is a sum-returning fallible call, and the matched case payload IS
  the discharge.

- **Never from the wire.** Deserializing a wire message produces structure + raw
  bytes per the agreed SCHEMA (chapter 10) -- that schema *is* "both sides agree
  on the format in advance." A "string" field decodes to plain `[u8]`, untrusted,
  with no domain; the encoding is then earned by `validate` like any other raw
  source (a file read, a socket). The wire layer moves bytes and structure; it
  does not grant encoding facts, so decode has one failure axis (malformed
  structure), not two. A schema field MAY opt into a declared encoding domain so
  decode validates at the boundary and folds encoding-invalidity into the decode
  error -- but that is an opt-in, never the default.

## Domains On Foreign Types

A package may declare a domain over a type it does not own. This is the
analog of Rust's extension traits: downstream code names its own validity
classes over upstream data (`domain Entity::Quarantined { ... }` in a policy
package, over a core `Entity`).

Working rules:

- Visibility is import-gated. A foreign-declared domain is not in scope on
  the type unless the declaring module is imported, so unrelated packages
  cannot quietly grow a type's namespace for everyone.
- Collisions are hard errors, never priority. If the owning package and an
  extending package (or two extenders visible in the same program) declare
  the same `Type::Name` -- as a case, domain, or machine -- compilation
  fails at the second declaration. Rust resolves the analogous conflict by
  silent priority (inherent methods win over extension traits), which lets an
  upstream addition rebind downstream behavior without an error; Omega
  rejects that outright because match arms and contracts carry authority
  decisions.
- Upstream additions are therefore loud breaking changes downstream. Adding
  a case or member that collides with an extender's domain breaks the
  extender's build -- the same severity class as adding a case under an
  exhaustive match, and the same place it gets caught (whole-program
  compilation today; package-admission compatibility checking
  later).[^foreign-domains]

[^foreign-domains]: Open details: the import-gate spelling (does `use
policy::quarantine;` suffice, or does domain visibility need its own form);
whether a stricter orphan-style rule (domains only in the type's own package)
should be available per package or per type; and how foreign-declared domains
appear in authority-flow and boundary reports.

## No Hidden RTTI

Omega should not inject hidden domain tags to make classification work.

If a program wants a runtime tag, it should write one:

```omega
enum GamePhase {
    NewGame,
    Playing,
    Finished,
}
```

Then domains can classify through that field. Keeping the tag explicit makes
layout, debugging, host boundaries, and proof obligations honest.

Working interpretation:

- `domain` is a contextual keyword in declaration position.
- Domains are type-scoped named static theories over an unchanged carrier.
- A domain may contribute predicate requirements, semantic roles, establishment
  routes, and alias expansion.
- Domain predicates may not contradict the invariants of the type they classify.
- There is no separate classifier declaration; matching evaluates the domain's
  predicate requirements.
- `Type::A::B` is a sub-domain of `Type::A` — its predicate requirements
  auto-include the parent's predicate requirements (single-parent; use
  `self in X & Y` for the DAG case).
- A named predicate is a pure bool machine, called from domain `requires` for
  horizontal fact reuse (no separate `predicate` binder).
- `requires x in Type::Domain` is a caller obligation.
- `ensures x in Type::Domain` is a callee guarantee.
- `x in Type::A | Type::B` is a domain union.
- `x in Type::A & Type::B` is a domain intersection.
- `pub domain Type::Alias = Type::A & Type::B;` is a transparent predicate
  alias expanded before normalization and identity.
- `Type::Domain` in a match arm is a domain pattern for values of `Type`.
- `if x in Type::Domain` is a full executable domain check when the domain is
  runtime-checkable.
- A fixed operator spelling is declared with an optional `spelling` clause on a
  named `operator`; domain operators may carry a `spelling`.
- Static semantic roles selected at a binding site (declaration, explicit
  qualification, or `requires`) participate in operator resolution;
  flow-established knowledge never changes operator meaning.
- Different semantic roles compose; competing contributions to one role reject.
- Domain `requires` propositions conjoin. Body entries name alternative exact
  trait requirements authorized to establish provenance.
- A domain with neither predicates nor routes permits explicit qualification
  from its bare carrier.
- Qualified `as` targets preserve denotation through compiler-derived exact
  coercion; explicitly bare targets erase non-owning semantic meaning. `as`
  never invokes arbitrary user code or fabricates routed provenance.
- Multiplicity governs copy/discard and must-discharge behavior. A
  content-bearing exact qualification may separately publish one owner-unique
  core `Content<A>` conformance selecting a compiler-owned decomposition
  algebra; permissions govern operations, and carry governs mobility. None is
  inferred merely from the domain's spelling or multiplicity.
- Qualification and proof evidence erase from runtime code. Static semantic
  roles may affect later operator lowering without adding runtime metadata.
- Qualification, explicit erasure, recast, validation, and noncanonical
  conversion remain distinct according to denotation, representation, and
  runtime work.

> **Implementation gate:** the current Rust trees carry independent predicate
> bodies, closed semantic-role records, transparent aliases, and normalized
> establishment routes. Explicit `as` into an empty domain (or an alias whose
> expanded atoms are all empty) is now compiler-derived and records vacuous
> qualification evidence; the former core qualification-satisfier trait has
> been retired. The source migration to predicate `requires` and route bodies,
> exact representation/scale conversion, and per-domain erasure is not yet
> implemented. Arithmetic policies still have special lowering paths. General
> domain work must preserve every domain-theory axis independently in the IR; see
> [semantic_taxonomy_representation.md](../architecture/semantic_taxonomy_representation.md).
