# Chapter 8: Domains

A domain is a zero-cost semantic theory attached to a value's unchanged
carrier.

> **Two FACETS (settled 2026-07-18, frozen decision 19; record:
> [domain_facets_and_qualification.md](../design_briefs/domain_facets_and_qualification.md)).**
> A domain has two independently governed facets:
>
> - a **predicate facet** — propositions about a value, resource, or current
>   program state, established by *proof* (flow-establishable,
>   lattice-composing, freely droppable, fully erased);
> - a **semantic facet** — an explicitly selected interpretation and operator
>   meaning, introduced by *authorized commitment* (declaration, mint, or
>   signature only; never flow-acquired, never silently dropped; reaches
>   codegen through operator selection).
>
> A domain may carry either facet or both: `Utf8` is predicate-only,
> `Wrapping` is semantic-only, `Degrees` is both. The governing law:
> **flow inference may change what is known; only declarations, mints, and
> signatures change what operations mean.** Prover growth turns rejections
> into acceptances — it never reinterprets a valid program.

Domains are not runtime tags, wrapper types, hidden storage, or a second
object model. Attaching, proving, selecting, or forgetting a domain never
changes representation or adds runtime metadata; validation and conversion
remain ordinary operations and may perform runtime work.

> **Every data type has a DEFAULT DOMAIN (settled 2026-07-05).** The invariants a
> `data` type "always has" are its default domain — the one domain that is always in
> scope for the value and travels with it everywhere, so it need not be named or
> tracked. Named domains like `Player::New` or `Quantity::Additive` are **subdomains**
> refining that base with tighter invariants, operators, or facts, selected at a mint
> point (`as`) when provable. This is why a per-field constraint and a domain are the
> *same* mechanism (see [Chapter 7](chapter_7_types_constraints_invariants.md)):
> single-field constraints are standing invariants of the default domain; cross-field
> invariants live there too, with stores the checker cannot prove domain-preserving
> carried as [invariant windows](chapter_11_invariant_windows.md) until the next
> consumption point (settled 2026-07-17). The first implementation slices now
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
honestly absorb gates carried only by later cases. Explicit `[zero_init]` is a
stronger promise that zeroed bytes are already an established value, so it is
rejected when the default domain or any zero-reachable field excludes zero.

```omega
data Player {
    health: i32;
    in_cutscene: bool;
}

domain Player::Valid {
    self.health >= 0;
    self.health <= 100;
}

domain Player::Dead {
    self in Player::Valid;
    self.health <= 0;
    self.in_cutscene == false;
}

domain Player::Alive {
    self in Player::Valid;
    self.health > 0;
}
```

`self` is the value being classified. Domain bodies are proof facts. They do
not create fields and they do not run unless the program explicitly asks for a
runtime diagnostic/checking build.

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
    winner: Option<PlayerId>;
}

domain Game::NewGame {
    self.phase == GamePhase.NewGame;
    self.turns == 0;
    self.board.empty;
    self.winner == None;
}

domain Game::Playing {
    self.phase == GamePhase.Playing;
    self.winner == None;
}

machine Game::start_game(&mut self)
    requires self in Game::NewGame
    ensures self in Game::Playing
{
}
```

## Establishing A Domain — `as`, With All Facts Proven

A value is *in* a domain only where the compiler can prove the domain's
predicate holds. Membership is a discharged proof obligation, never a runtime
tag or an unbacked claim. A value comes to carry a domain in exactly three
ways, and no others:

- **Constant.** A compile-time-known value whose facts the compiler checks
  directly: a literal `"hello"` is provably `Utf8`; `0` is provably `[0..=100]`.
- **Flow.** A dominating guard narrows a value: in the `true` arm of
  `transition level <= 100`, `level` carries `[0..=100]` (Chapter 9). The guard
  *is* the proof. **Flow establishes predicate facets only** — a semantic
  facet is never acquired through control flow.
- **`as`.** The explicit minter. For a predicate facet, `value as T in D` is
  licensed **only when the prover discharges every invariant of `D` at that
  exact point**. If it cannot, it is a compile error — you restructure (add
  the guards that establish the fact) until the proof exists. For a semantic
  facet, `as` makes an explicit, authorized commitment (see Introduction
  Authority below). Minting into a hybrid domain does both at once.

There is no unsafe cast and no "assert, on me" escape. **`as` proves facts
and declares commitments — it never asserts a fact unproven, and never
invents a commitment unstated.** A predicate is a fact about the value,
provable; a semantic qualification is a commitment by the author, not
falsifiable from the bits (no checker can determine that a raw `1.0` "really
came from" a kilometre measurement). Diagnostics keep the two failure classes
separate: *"predicate obligation not discharged"* (a proof is owed) versus
*"introduction authority unavailable"* (a permission is owed).

### The Five Transitions

| Operation | Representation | Effect | Runtime cost |
|---|---|---|---|
| Refinement mint (`as`) | unchanged | certifies already-proved facts | none |
| Semantic qualification (`as`) | unchanged | makes an explicit, authorized commitment | none |
| Forgetting | unchanged | discards facts (free) or meaning (per weakening rules) | none |
| Conversion | may change | preserves denotation across representations | ordinary contracted call |
| Validation | unchanged | performs work whose postcondition establishes facts | ordinary contracted call |

Forgetting and conversion are different operations: forgetting `raw 1 in Km`
yields `raw 1` (denotation deliberately discarded); *converting* it yields
the canonical `1000` in metres. Only conversion and validation may cost at
runtime, and both are ordinary contracted calls — never hidden inside `as`.

### Introduction Authority

Semantic introduction is **sealed by default**: only the owning package, or
holders of an exported, attenuable `MintAuthority<D>` (contract-visible,
proof-erased), may qualify a value into the domain. Open introduction is a
one-line opt-in at the declaration site — the right posture for units, where
qualifying your own measurement is an ordinary authorial commitment:

```omega
domain f64::Km { introduction open; }
domain Quantity::Torque { introduction sealed; }
```

A forgotten annotation must never become an ambient authority leak, so the
dangerous case is the default and the harmless case requests openness once.

Predicate facets need no introduction policy — facts are proved, not
authorized. But provability is scoped by **body visibility**: a predicate
whose body (or named-predicate machines) is package-private cannot be
unfolded by outsiders' flow or `as`; outsiders establish or propagate it only
through owner-exported evidence — a transformer's postcondition
(`sanitize_sql -> Bytes in SanitizedForSQL`) or an exported decision
procedure's true-arm. The owner chooses the evidence surface.

### Weakening

A semantic domain weakens implicitly to its carrier only if (1) the identity
representation map **preserves denotation** and (2) every default operation
**agrees with the qualified operation** throughout the default's accepted
region. Certified arithmetic policies pass both; units fail (1) even where
raw arithmetic coincides. Each semantic domain declares its denotation map,
so the criterion is checked, not intuited: **mechanically decidable for
recognized schemas** (rationally scaled units, blessed policies), **otherwise
proof-obligated via an explicit `weakens_to` certificate — never guessed.**
Once a certificate is accepted, the certified operator theory is **sealed**:
overlapping later extensions must re-prove the agreement law or be rejected.
Units and kinds therefore never weaken silently; certified policies weaken
implicitly — sound because the exact-loud default reinstates obligations on
the far side.

Establishing a domain over *runtime* data is therefore ordinary code, not a
compiler builtin. To turn untrusted bytes into `&[u8] in Utf8` you write a
machine that reads the bytes, guards that each unit is valid, and casts in the
arm where the whole sequence is proven:

```omega
data Utf8Scan {
    case NotText;
    case Text(view: &[u8] in Utf8);
}

machine Scanner::scan(&mut self, bytes: &[u8]) -> Utf8Scan {
    self.i = 0;
    transition { _ -> step(bytes) }

    state step(&mut self, bytes: &[u8]) -> Utf8Scan {
        transition self.i < bytes.len {
            true -> check(bytes)
            _    -> (Utf8Scan::Text { view: bytes as &[u8] in Utf8 })  // all units proven
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
> The explicit `as`-mint to an **arithmetic** domain works today
> (`x as u8 in Saturating`; see `expressions/arithmetic_domain_cast_exit`). The
> `as`-mint to a **reference/encoding/layout** domain shown here
> (`bytes as &[u8] in Utf8`) is the recast surface that is **not yet
> implemented** — it is the pending work in the mint arc (`as` + the
> invariant-prover's reach). The shape above is the intended spelling, not
> currently compilable.

The compiler generates none of this — and generates nothing at all for domain
membership. Its only job is to accept or reject the `as`, by asking whether the
domain's invariants are proven at that point. The reach of that prover is the
ceiling on what can be minted: a fact the prover cannot yet discharge (a
whole-buffer property established across a loop) simply cannot be cast until the
prover grows to reach it. This is the anti-serde principle at its end — the only
path from raw bytes to a trusted fact is a proof the machine actually checked.

If you want a `Valid | Invalid`-style result, you declare that sum type yourself
(`Utf8Scan` above); there is no built-in verdict type and no generated codec.

### Declarations And The Zero Value

The one place a domain appears without a written `as` is a declaration —
`x: T in D`, or a `data` field `f: T in D`. The cast is implicit there, but it
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

domain Player::Valid {
    self.health >= 0;
}

domain Player::Alive {
    self in Player::Valid;
    self.health > 0;
}

domain Player::Dead {
    self in Player::Valid;
    self.health == 0;
}
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
domain Player::Dead {
    self.health == 0;
}
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

A domain is *only* its invariant facts — there is no separate classifier clause
(there is no `when` keyword). A domain that participates in matching just gets
tested by evaluating its body; when the body's leading fact is a cheap field
compare, that test *is* cheap, with nothing extra to declare.

Refinement is expressed structurally, by nesting the name: `A::B::C` is a
**sub-domain** of `A::B`, and its body auto-includes the parent's facts.

```omega
domain Game::Playing             { self.phase == GamePhase.Playing; self.winner == None; }
domain Game::Playing::RoundStart { self.turn == 1; }
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
parents still writes the explicit intersection in its body (`self in X & Y`) —
the name path is for the common refinement chain, `&` for the DAG cases.

A domain pattern is executable when its body's facts are pure, finite, and
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

domain Game::Playing { self.phase == GamePhase.Playing; in_span(self); }
domain Game::Sudden  { self.phase == GamePhase.Playing; in_span(self); self.turn == 9; }
```

The distinction: a **sub-domain** is a named membership set with identity (you
`match` / `as` / `require` it); a **named predicate** is an anonymous reusable
condition with no identity (a helper like `in_bounds`). Use the first for a
meaningful state, the second for a shared fact-bundle. They compose — a
sub-domain body may call named predicates, and a predicate may reference
membership.

## Overlap And Intersections

Domains may overlap when they are just proof facts.

```omega
domain Password::Valid {
    self.len >= 12;
    self.has_symbol;
}

domain Password::Secure {
    self.entropy_bits >= 80;
}
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

Domains are primarily proof facts about values. Omega also allows proven
domains to participate in operator resolution when the meaning is unique.

The intuition is that operators are shorthand for resolved semantic
operations. If a value's *declared* semantic facet supplies a `+`, `-`, or
similar operator meaning, the compiler resolves the operator through that
domain's operation contract. **Activation is a property of bindings, not of
values and not of proof state**: a binding declared, minted, or
`requires`-qualified into `Degrees` resolves `+` through Degrees within its
scope; a plain `i32` binding never does, regardless of what has been proven
about the value it holds.

```omega
domain i32::Degrees {
    // semantic facts about cyclic degree values
}

machine rotate(value: i32, delta: i32) -> i32
requires value in i32::Degrees
{
    value + delta
}
```

In that shape, `+` is not mystical. The `requires` clause is a signature-site
selection: `value` is *bound* into `i32::Degrees` for this machine's scope,
so `value + delta` resolves through the `Degrees`-specific addition meaning
if that meaning is unique.

This stays strict:

- Only binding-site selections (declaration, mint, `requires`) participate in
  operator resolution; flow-established membership never does.
  `if x in Degrees { x + delta }` proves the range fact — the `+` stays
  ordinary exact addition.
- Resolution reads the complete static operand-domain tuple and must be
  unambiguous; competing meanings are compile errors, never ranked.
- Adding proof knowledge can move a program from compiling to rejected
  (a new ambiguity is a loud error), never from meaning-A to meaning-B.
- No hidden runtime tag is introduced for dispatch.

This is especially attractive for semantic abstractions such as strings and
quantities. For example, `String::Utf8` and `String::NoNul` may want the `+`
spelling to resolve through concatenation while preserving whichever domains
the operation can soundly guarantee.

## Operator Definitions And Domain Contexts

Operator overloading is trait-like in spirit but proof-aware in
resolution.

Rust maps a fixed operator spelling such as `+` or `[]` to a trait method such
as `Add::add` or `Index::index`. Omega has a similar semantic home for
operators, but with one extra axis: the current proof context may determine
which operator meaning is available.

A fixed operator spelling is declared with an optional `spelling` clause on a
named `operator`. The named operator carries the full signature and proof
contract; the `spelling` clause only binds the surface symbol that resolves to
it.

```omega
domain Quantity {
    // semantic facts about quantity values
}

operator add(left: Quantity, right: Quantity) -> Quantity spelling +;
```

Domain operators declared inside a `domain` block may carry a `spelling`.
Domain-sensitive resolution then selects among spelled candidates by
receiver/operand type plus proven domain context. Competing domain meanings for
the same spelling are a compile error.

Decided model:

- Fixed operator spellings are declared with an optional `spelling` clause on a
  named `operator`.
- Core types such as `Slice`, `Array`, `Vec`, and `String` can expose
  operator definitions whose implementations are bound to boundary primitive
  compiler/runtime operations below the public core surface.
- User/library types can expose ordinary operator definitions when the language
  supports that surface.
- Domains may provide or select operator meanings when the value's *binding*
  is declared, minted, or `requires`-qualified into that domain — never from
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

For domain-sensitive quantities, the same spelling can resolve through a domain
context:

```omega
machine add_degrees(value: i32, delta: i32) -> i32
requires value in i32::Degrees
{
    value + delta
}
```

Here the ordinary integer `+` is not automatically replaced. The `requires`
clause binds `value` into `i32::Degrees` for this scope, and that binding
participates in resolution only if it exposes a unique operator meaning for
this expression.

Ambiguity is an error:

```omega
requires value in Angle::Degrees & Angle::Turns
```

If both domains expose different `+` meanings for the same expression, the
program must choose a clearer operation or narrow the proof context before using
operator syntax.

Not every domain needs this power — a predicate-only domain supplies no
operator meanings at all. The earlier open question of whether arithmetic
policies belong in a separate evaluation-mode concept is resolved by the
facet model (frozen decision 19): `Wrapping`, `Saturating`, and `Trapping`
are semantic-only domains — the compiler-blessed closed subset, special only
because primitive arithmetic needs direct lowering. Decision 17 is unchanged
and conforms. The important point stands: domain-sensitive operators are
resolved from static binding-site selections, not from runtime type mutation
and not from the flow-fact environment.

**Normalization is not entailment.** A small deterministic, confluent,
terminating normalizer owns what a domain expression *is* (canonical
dimension vectors, scale products, kind tags); type identity, semantic
interface identity, and monomorphization keys depend only on it. The
entailment engine proves propositions *about* expressions and can never
redefine canonical identity. Physical ABI remains the **carrier's** ABI
(representation erasure holds); semantic interface identity includes
normalized domains. Units and the full quantity model (dimension x kind x
scale x presentation) are specified in the design brief.

## Domains On Strings And Encodings

Text is not a type in Omega. It decomposes into three things that already
exist, and `String`/`Bytes` are not among them:

- a **byte container** -- `&[u8]` (a view), `[u8; N]` (fixed), or `Vec<u8>`
  (owned). The only part with a layout.
- an encoding's **validity**, expressed as a *domain* over the byte container.
- an encoding's **codec** (decode/encode/boundary), expressed as
  *domain-sensitive operators*, not a predicate.

"A UTF-8 string" is therefore `[u8] in Utf8`, not a `String`:

```omega
&[u8]   in Utf8           // text view, zero-copy
Vec<u8> in Utf8           // owned text (needs the allocator)
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
domain Slice<u8>::Ascii { all_below(self, 0x80) }   // per-element predicate (library code)
domain Slice<u8>::NoNul { no_interior_nul(self) }   // per-element predicate
domain Slice<u8>::Utf8  { utf8_ok(self) }           // sequence recogniser (below)
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

- `[u8] in Utf8` for APIs that require UTF-8 text,
- `[u8] in Utf8 & NoNul` for C-style boundaries that reject interior NULs,
- `[u8] in Utf16` (a different validity + codec) for a UTF-16 boundary.

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
operator concat(left: Slice<u8> in Utf8, right: Slice<u8> in Utf8)
    -> Slice<u8> in Utf8 spelling +;          // concat preserves UTF-8: proven once

operator slice(s: Slice<u8> in Utf8, range: Range) -> Slice<u8> in Utf8
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

> Implementation note: the compiler today still carries `string` (an unsized
> text view) and `String` (`PrimitiveType::String`) as builtin types. A wire
> `&string` field was prototyped (zero-copy interpreter decode + native encode)
> and then REMOVED as a vestige once this model settled: the honest borrowed
> bytes/text wire field is `&[u8]` (a fat slice, which is already the native
> representation), not a `&string`. The byte view is variable-length, so it
> rides a RAW-byte encoding (length varint + raw bytes, like protobuf `bytes`),
> distinct from a `[u8; N]` repeated field (packed per-element varints). Wiring
> a `&[u8]` bytes field through the wire layer, plus replacing the builtin
> Domains over byte views and bounded carriers, direct literal construction,
> bounded return values, and native/interpreter carrier lowering are now built.
> Wholesale `string`/`String` removal waits on the remaining corpus migration
> and the allocator-backed growable carrier surface. Mutable boundary/operator
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
> Standard Console output is already carrier-based: `write` and `write_line`
> borrow `&[u8]`, and the checked adapter walks that view directly. An owned
> bounded carrier projects its runtime length and inline byte address at the
> call seam, including when reached through a mutable carrier reference. A
> guard-selected literal returned as a bounded carrier is constructed in the
> result slot as `{len, inline_bytes}`, not as a borrowed descriptor. The mutable
> `read_line` compatibility signature still names
> `String` pending the allocator/bounded-destination surface; this is an
> implementation fence, not the target text model.

### Establishing the domain: construction, validation, and the wire

A byte container EARNS its encoding domain in exactly one of two ways -- and
never from the transport:

- **By construction.** A literal, or a value built from known-good bytes, is
  `[u8] in Utf8` by construction: the compiler knows the bytes, so there is no
  runtime check. The preservation operators above (`concat`, boundary-`slice`)
  carry the domain forward, so text stays text without re-validation.
  For an owned bounded carrier `[u8; N] in Utf8`, construction additionally
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
      case Ascii(text: [u8] in Ascii);   // payload already carries the domain
      case Utf8(text:  [u8] in Utf8);
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
- Domains are type-scoped named proof predicates.
- Domains classify values that satisfy the type's data and field invariants.
- A domain body may not contradict the invariants of the type it classifies.
- There is no `when` classifier keyword; a domain is only its invariant facts.
- `Type::A::B` is a sub-domain of `Type::A` — its body auto-includes the parent's
  facts (single-parent; use `self in X & Y` for the DAG case).
- A named predicate is a pure bool machine, called from domain bodies for
  horizontal fact reuse (no separate `predicate` binder).
- `requires x in Type::Domain` is a caller obligation.
- `ensures x in Type::Domain` is a callee guarantee.
- `x in Type::A | Type::B` is a domain union.
- `x in Type::A & Type::B` is a domain intersection.
- `Type::Domain` in a match arm is a domain pattern for values of `Type`.
- `if x in Type::Domain` is a full executable domain check when the domain is
  runtime-checkable.
- A fixed operator spelling is declared with an optional `spelling` clause on a
  named `operator`; domain operators may carry a `spelling`.
- Semantic facets selected at a binding site (declaration, mint, or
  `requires`) participate in operator resolution; flow-established membership
  never does. Competing meanings for the same spelling are a compile error,
  never ranked.
- Semantic introduction is sealed by default; `introduction open` is the
  declaration-site opt-in; exported `MintAuthority<D>` delegates sealed
  introduction.
- Predicate facets erase completely from ordinary runtime code unless a
  diagnostic build explicitly asks for checks. Semantic facets add no runtime
  metadata but reach codegen through operator selection.
- The five transitions are distinct: refinement mint and semantic
  qualification (representation-identity, free), forgetting
  (representation-identity, free, explicit for meaning), conversion
  (denotation-preserving, may change representation, ordinary contracted
  call), validation (fact-establishing work, ordinary contracted call).

> **Implementation gate:** the current Rust trees still store facts and
> operators in one undifferentiated domain record. General domain work must
> first preserve the two facets and normalized semantic qualification in the
> IR; see
> [semantic_taxonomy_representation.md](../architecture/semantic_taxonomy_representation.md).
