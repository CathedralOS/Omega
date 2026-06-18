# Chapter 8: Domains

Domains are named proof predicates over existing values.

They are not runtime tags, wrapper types, hidden storage, or a second object
model. A domain names a meaningful semantic state that the compiler can prove
for a value.

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
    turns: usize;
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

Relax scopes may temporarily suspend a required fact inside a machine body, but
that does not make an invalid value a member of a domain. Domain membership is a
fact about values that satisfy the type's ordinary validity rules.

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

## Classifiers

Domains that participate in domain patterns should provide a cheap classifier
with `when` when possible.

```omega
domain Game::Playing
    when self.phase == GamePhase.Playing
{
    self.winner == None;
}

domain Game::Finished
    when self.phase == GamePhase.Finished
{
    self.winner != None || self.turns == 9;
}
```

The `when` clause is the classifier. The domain body is the full set of proof
facts for that domain.
For a domain pattern such as `Game::Playing`, the compiler may lower the match
through the classifier, such as `game.phase`, instead of rechecking every body
fact.

If a domain has no classifier, a domain pattern may still be executable when
all of the domain body's facts are pure, finite, and runtime-checkable:

```omega
if player in Player::Dead {
    respawn(player)
}
```

This lowers to the domain body's comparisons and updates the true branch with
`player in Player::Dead`. Domains with quantifiers, opaque proof calls, or
non-executable facts cannot be used as runtime checks unless they expose an
explicit executable classifier or checker.

For classified domains, the compiler checks classifier facts:

- A non-wildcard match over a known domain union must be exhaustive.
- Classifiers should be mutually exclusive when the program relies on
  unordered domain-union reasoning.
- Each arm receives the facts from the selected domain.
- Each transition target must accept the facts established by its arm.

The compiler may infer simple classifiers later, but explicit `when` clauses
are the reliable source-level mechanism.

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
the domains must still be distinguishable by mutually exclusive classifiers.

## Domain-Sensitive Operators

Domains are primarily proof facts about values. Omega also allows proven
domains to participate in operator resolution when the meaning is unique.

The intuition is that operators are shorthand for resolved semantic
operations. If a domain supplies the only valid `+`, `-`, or similar operator
meaning for a value in the current proof context, the compiler may resolve the
operator through that domain's operation contract.

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

In that shape, `+` is not mystical. The machine body already knows that
`value` is in `i32::Degrees`, so the compiler can resolve `value + delta`
through the `Degrees`-specific addition meaning if that meaning is unique.

This should stay strict:

- Only proven domains may participate in operator resolution.
- Resolution must be unambiguous.
- Competing domain-provided meanings are compile errors.
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
- Domains may provide or select operator meanings when the value is proven to
  be in that domain.
- Resolution must be static and unambiguous.
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

Here the ordinary integer `+` is not automatically replaced. The domain fact
`value in i32::Degrees` participates in resolution only if it exposes a unique
operator meaning for this expression.

Ambiguity is an error:

```omega
requires value in Angle::Degrees & Angle::Turns
```

If both domains expose different `+` meanings for the same expression, the
program must choose a clearer operation or narrow the proof context before using
operator syntax.

Not every domain needs this power. Some ideas, especially arithmetic policy
concepts such as `wrapping` or `checked`, may turn out to fit better as a
separate evaluation-mode concept than as ordinary value domains. The important
point is that domain-sensitive operators are resolved from compile-time proof
knowledge, not from runtime type mutation.

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

// Encodings are validity domains over the byte container. (`[u8]` is the
// surface spelling of `Slice<u8>`, which is the nominal carrier the domain
// binds to -- the same move as `domain i32::Degrees`, one generic level up.)
domain Slice<u8>::Utf8  when valid_utf8(self)      { /* facts decode relies on */ }
domain Slice<u8>::Ascii when all_below(self, 0x80) { self in Slice<u8>::Utf8; }
domain Slice<u8>::NoNul when no_interior_nul(self) { }
```

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

- validate the encoding ONCE at the ingest boundary (the `when valid_utf8`
  classifier/checker), establishing `in Utf8` as a fact;
- carry that fact; never re-scan;
- prove a small set of *preservation* lemmas as operator contracts (concat
  preserves `Utf8`; boundary-`slice` preserves it) so downstream code keeps the
  fact without re-proof;
- richer sequence-wide invariants come later.

Removing the `String` type *helps* here: with no type obligated to maintain
validity through every operation, there is nothing to re-prove between the
boundary and the operators.

> Implementation note: the compiler today still carries `string` (a `&[u8]`
> view) and `String` (`PrimitiveType::String`) as builtin types, and the wire
> path already lowers `&string` to `{ptr, len}` over `u8` -- i.e. the
> representation is *already* a byte view with the encoding left implicit. This
> section is the decided target model; replacing the builtin `string`/`String`
> with `[u8] in Utf8` waits on domains over `Slice<u8>`, the codec operators
> above, and a corpus migration.

## Domains On Foreign Types

A package may declare a domain over a type it does not own. This is the
analog of Rust's extension traits: downstream code names its own validity
classes over upstream data (`domain Entity::Quarantined when ...` in a policy
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
- `when` is a cheap, pure classifier, not the whole invariant.
- `requires x in Type::Domain` is a caller obligation.
- `ensures x in Type::Domain` is a callee guarantee.
- `x in Type::A | Type::B` is a domain union.
- `x in Type::A & Type::B` is a domain intersection.
- `Type::Domain` in a match arm is a domain pattern for values of `Type`.
- `if x in Type::Domain` is a full executable domain check when the domain is
  runtime-checkable.
- A fixed operator spelling is declared with an optional `spelling` clause on a
  named `operator`; domain operators may carry a `spelling`.
- Proven domains may participate in operator resolution when the applicable
  operator meaning is unique; competing domain meanings for the same spelling
  are a compile error.
- Domain facts erase from ordinary runtime code unless a diagnostic build
  explicitly asks for checks.
