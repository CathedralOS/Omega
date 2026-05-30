# Chapter 9: Proof Obligations

Typed states, bounded values, borrows, transitions, effects, drops, and host
boundaries imply compiler-generated obligations.

This chapter is about the compiler's proving job, not new surface syntax.
Chapter 7 introduces contracts and flow facts. Chapter 8 introduces domains.
This chapter explains how those facts turn into obligations the compiler must
discharge.

Most programmers should see facts move through contracts, domains,
preconditions, and postconditions without needing to write mathematical proof
libraries just to get normal code accepted.

## Vocabulary

Omega's proof model should use a small set of terms consistently.

- Facts are what the compiler currently knows.
- Requirements are facts an operation needs before it may run.
- Guarantees are facts an operation promises after it runs.
- Obligations are proof or check work the compiler must discharge.
- Invariants are facts that must remain true across a boundary.
- Contracts are requirements plus guarantees.
- Boundary is an accepted authority for guarantees that Omega cannot prove from Omega code.

Short form:

```text
values carry facts
operations have contracts
contracts create obligations
the compiler proves obligations using facts
operations contribute guarantees
invariants are facts that must survive boundaries
boundary explains why unproved guarantees are accepted
```

For ordinary Omega code, users should not have to write every contract
explicitly. The compiler knows the contracts for assignment, arithmetic, field
access, transitions, borrows, cleanup, and similar language operations.

For boundary code, contracts must be explicit. Host APIs, inline assembly, target-specific primitive operations, and boundary packages sit at the edge of Omega's semantic world. The compiler cannot honestly infer their behavior unless the toolchain or author supplies a contract.

Likely obligations:

- Every assignment into a bounded location preserves the bound.
- Every terminal expression of a typed state satisfies the declared return value type.
- Every transition into a typed state provides compatible arguments.
- Every typed transition satisfies return value compatibility.
- Every transition dispatch arm establishes the assumptions needed by its target.
- Every `relax` scope re-establishes all relaxed invariants before exit.
- No transition occurs while a relax scope is active.
- Every generic invariant is instantiated with compile-time or proof-visible facts.
- Every float invariant is checked as a semantic fact, not treated as an optimization permission.
- Every owned value that dies on a transition edge is cleaned up before the
  jump, and cleanup guarantees are added to the target facts.
- Every spawned graph captures only moved, copied, or concurrency-safe values.
- Every blocking operation exposes a waitable contract or crosses an explicit
  boundary.
- Every machine that claims termination proves progress for every recursive or
  cyclic path in its reachable call/state graph.

## Everyday Shape

Most proof work should look like ordinary contracts:

```omega
machine Game::enter_combat(&mut self)
requires
    self.player in Player::Alive
ensures
    self.mode in GameMode::Combat
{
}
```

The caller must provide the requirement. The machine body must establish the
guarantee. The checker carries those facts forward.

That is the common case:

```text
preconditions + body facts -> postconditions
```

Advanced proof work and math libraries exist for cases where the compiler cannot
derive those facts automatically. They are covered in the next chapter.

## Termination Claims

Termination should be a separate proof claim, not an effect.

Working direction:

```omega
machine walk(items: &[Item])
terminates {
    decreases items -> Slice::Length;
}
{
}
```

The intended meaning is:

- `terminates` claims that the machine always completes.
- That claim is transitive through the reachable call graph and internal state
  graph.
- A terminating root such as `Main::main` should force every reachable cycle to
  justify progress, rather than requiring unrelated leaf machines to all carry
  standalone termination annotations by default.
- Progress clauses belong under `terminates` because they are consumed by the
  termination checker, not by the ordinary pre/postcondition checker.
- `decreases value -> OrderOrMeasure` means "prove recursive or cyclic
  back-edges make `value` strictly smaller under this selected ranking view."
- Plain `decreases value` should remain available when the ranking is builtin or
  otherwise unambiguous.

The important semantic piece is a well-founded ordering. A decrease metric is
accepted only when the compiler knows how to compare successive values in a
well-founded way.

Built-in ranking shapes:

- natural numbers or bounded integers (the default descending-naturals order)
- slice/view extents such as `items` under `Slice::Length`
- named domain/type-specific ranking orders such as `Card::PowerOrder`
- lexicographic tuples of decreasing metrics

`Slice::Length` is a named ranking view, not a runtime field lookup and not a
domain membership predicate by itself. It means "rank this slice by its current
length using the well-founded natural-number order." The name should be visible
through the core `Slice` surface so users can discover what the termination
checker is using.

A custom well-founded ordering is declared with a dedicated `measure` keyword as
a standalone item. A measure is **not** an abused `operator` declaration: it is a
function from the decreasing value into a well-founded domain such as `usize`.

```omega
measure Card::PowerOrder(card: Card) -> usize { card.power }
measure Quest::Difficulty lexicographic { tier, remaining_steps }
```

`lexicographic { a, b, ... }` declares an ordered tuple compared left-to-right.
Multiple named measures per type are allowed, so the same type can be ranked
different ways at different use sites.

The use site is unchanged. `terminates { decreases card -> Card::PowerOrder; }`
selects the named measure; plain `decreases value` still uses the default
descending-naturals order; built-in views such as `Slice::Length` remain
available without a `measure` declaration.

For slices, `decreases items -> Slice::Length` naturally means each back-edge
must operate on a strictly smaller remaining view, usually by carrying a
narrower slice window such as `items[1..]`.

`increases` and `decreases` are still useful as the user-facing proof words, but
the working direction is to make `->` consistently select the ranking view
rather than overloading it to mean "toward a bound."

For example:

```omega
machine weaken(card: Card)
terminates {
    decreases card -> Card::PowerOrder;
}
{
}
```

and:

```omega
machine walk(items: &[Item])
terminates {
    decreases items -> Slice::Length;
}
{
}
```

The core obligation is still a ranking proof. The surface just names the value
being tracked plus the ranking view being used.

## Example

```omega
data Player {
    health: i32;
}

machine Player::take_damage(
    &mut self,
    damage: i32
) {
    self.health -= damage;
}
```

The value `self.health` carries facts: it is an initialized machine integer, it is mutable through `self`, it follows the current arithmetic policy, and it may have obligations such as remaining in `0..=100`.

The subtraction operation has requirements: `self.health` must be mutable, `damage` must be compatible, and exact subtraction must not underflow or overflow.

The assignment back into `self.health` creates obligations: prove the arithmetic is valid and prove the resulting value satisfies the field invariant at the required boundary.

If those obligations are discharged, the operation contributes guarantees: `self.health` is initialized and has the proven resulting facts. If not, the compiler must reject the code, require a different arithmetic mode, require a `relax` scope, or require an explicit checked/boundary depending on the construct.

This maps well onto TLA+ style action checking:

- Machine fields are variables.
- State parameters are action inputs.
- Transitions are guarded next-state relations.
- Value constraints are invariants or pre/postconditions.
- Relax scopes are local invariant weakening with mandatory restoration.

Invariants are not RTTI. If proof fails, the normal result is a compiler diagnostic, not a hidden runtime tag check. Runtime validation may exist as an explicit debug or proof-emission mode, but it should not define the semantics.

Float invariants are also not fast-math flags. A proof that a value is `finite` or in `a..=b` does not automatically permit reassociation, signed-zero erasure, reciprocal transforms, or other approximate rewrites.
