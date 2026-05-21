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

domain Dead for Player {
    self.health <= 0;
    self.in_cutscene == false;
}

domain Alive for Player {
    self.health > 0;
}
```

`self` is the value being classified. Domain bodies are proof facts. They do
not create fields and they do not run unless the program explicitly asks for a
runtime diagnostic/checking build.

## State Contracts

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

domain NewGame for Game
    when self.phase == GamePhase.NewGame
{
    self.turns == 0;
    self.board.empty;
    self.winner == None;
}

domain Playing for Game
    when self.phase == GamePhase.Playing
{
    self.winner == None;
}

machine Game::start_game(&mut self)
    requires self in Game::NewGame
    ensures self in Game::Playing
{
}
```

Domains may also be declared inside a `data` block as sugar when that is easier
to read:

```omega
data Player {
    health: i32;
    in_cutscene: bool;

    domain Dead {
        health <= 0;
        in_cutscene == false;
    }
}
```

That desugars to `domain Dead for Player { self.health <= 0; ... }`.

## Domains And Invariants

Domains classify values that are valid for their type.

```omega
data Player {
    health: i32[non_negative];
}

domain Alive for Player {
    self.health > 0;
}

domain Dead for Player {
    self.health == 0;
}
```

The field constraint defines ordinary `Player` validity. The domains name
semantic subsets inside that valid space.

A domain may not specify facts that violate the data or field invariants of the
type it classifies.

```omega
data Player {
    health: i32[positive];
}

// Invalid: `health == 0` contradicts `health: i32[positive]`.
domain Dead for Player {
    self.health == 0;
}
```

Relax scopes may temporarily suspend an invariant inside a machine body, but
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
domain Playing for Game
    when self.phase == GamePhase.Playing
{
    self.winner == None;
}

domain Finished for Game
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
domain Valid for Password {
    self.len >= 12;
    self.has_symbol;
}

domain Secure for Password {
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
- Domain facts erase from ordinary runtime code unless a diagnostic build
  explicitly asks for checks.
