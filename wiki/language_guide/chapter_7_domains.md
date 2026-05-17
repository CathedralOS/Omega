# Chapter 7: Domains

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

States and functions can require or guarantee domains.

```omega
state respawn(player: &mut Player)
    requires player in Dead
    ensures player in Alive
{
    player.health = 100;
}
```

This is shorthand for a named bundle of proof obligations. The caller must
prove `player in Dead` before entering `respawn`. The state must prove
`player in Alive` before returning or transitioning to a target that requires
that domain.

Machine-owned state uses the same model:

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

state start_game(&mut self)
    requires self in NewGame
    ensures self in Playing
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

## Domain Unions

Some operations naturally produce one of several semantic states.

```omega
fn apply_move(game: &mut Game, pos: BoardPos) -> MoveResult
    requires game in Playing
    ensures game in Playing | Finished
{
}
```

`game in Playing | Finished` means the compiler knows `game` is in one of
those domains after the function, but not which one until control flow splits
again.

Callers can split a known domain union with `domain_of`:

```omega
match domain_of(game) {
    Playing -> continue_game()
    Finished -> show_result()
}
```

This does not mean "check every invariant at runtime." It means the compiler
uses an existing classifier to choose the arm and then adds that domain's facts
to the arm's proof context.

## Classifiers

Domains that participate in `domain_of` should provide a cheap classifier with
`when`.

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

The `when` clause is the classifier. The domain body is the full invariant.
For `domain_of(game)`, the compiler should lower the match through the
classifier, such as `game.phase`, not by rechecking every body fact.

The compiler checks classifier facts:

- Classifiers for a classified union must be mutually exclusive.
- The known union must be exhaustive at the match site.
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
requires password in Valid & Secure
```

But overlapping domains cannot be used as a `domain_of` dispatch unless their
current classified set has mutually exclusive classifiers. If two possible
domains have no disambiguating feature, `domain_of` is a compile error.

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
- `when` is a cheap, pure classifier, not the whole invariant.
- `requires x in Domain` is a caller obligation.
- `ensures x in Domain` is a callee guarantee.
- `x in A | B` is a domain union.
- `x in A & B` is a domain intersection.
- `domain_of(x)` is only legal for a known, exhaustive, disjoint classified
  domain union.
- Domain facts erase from ordinary runtime code unless a diagnostic build
  explicitly asks for checks.
