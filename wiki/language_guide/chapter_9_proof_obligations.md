# Chapter 9: Proof Obligations

Typed states, bounded values, borrows, transitions, service reach, drops, and host
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
- Every invariant window closes — its suspended facts re-proven — at the next
  consumption point (read, borrow, call, transition, return, boundary).
- No window spans a transition edge: transitions are consumption points.
- Every generic invariant is instantiated with compile-time or proof-visible facts.
- Every float invariant is checked as a semantic fact, not treated as an optimization permission.
- Every owned value that dies on a transition edge is cleaned up before the
  jump, and cleanup guarantees are added to the target facts.
- Every concurrently activated machine receives only moved, copied, or
  concurrency-safe values, and a rejected start returns all moved ownership.
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

Termination is a machine-level progress guarantee, not an effect and not a
postcondition over a returned value. Omega uses one source family:

```omega
boundary trait FiniteReader {
    machine read_all(&mut self, out: &mut Vec<u8>) -> ReadResult
        terminates;
}

machine walk(items: &[Item])
terminates by items -> Slice::Length;
{
}
```

These spell two semantic fields:

- bare `terminates;` authors the public guarantee that an invocation reaches a
  terminal outcome, conditional on its explicit requirements and pinned
  progress premises;
- `terminates by subject -> View` supplies private ranking evidence for every
  cycle in a checked implementation.

Checked acyclic bodies derive the guarantee and write nothing. A cyclic body
must author a ranking witness because the compiler never invents subjects or
heuristically selects noncanonical ranking theories. A body satisfying a
terminating requirement inherits the guarantee; its `terminates by` text only
supplies the witness and does not redefine the interface.

Derived summaries remain local. An exported machine that omits bare
`terminates;` publishes no termination guarantee, even if its current body is
acyclic. Trait/import calls use the pinned authored or inherited guarantee;
direct local calls may use the tighter checked summary.

Every cyclic edge must make the selected well-founded rank strictly smaller.
Useful views include:

- `Nat::Descending` for a descending unsigned/natural value;
- `Nat::IncreasingTo(limit)` for a value climbing toward a finite bound;
- `Slice::Length` for a narrowing slice/view;
- named structural views such as `Tree::ProperSubtree`; and
- lexicographic rankings for multi-part progress.

For example:

```omega
machine walk_up(limit: u64, index: u64)
terminates by index -> Nat::IncreasingTo(limit) in 0..=limit;
{
}
```

Authors do not manufacture `limit - index`: the view owns the deterministic
rank normalization. The optional range constrains the rank produced by the
view, establishes its well-founded floor, and allocates nothing.

The short form `terminates by n` is available only when the carrier declares a
stable canonical default ranking. It normalizes immediately to the explicit
view. A declared custom measure is never inferred merely because it is the
only visible candidate; adding another measure must not reinterpret a checked
program.

Custom ranking views remain declared through the dedicated `measure` surface,
not as operators. Multiple named measures per carrier are legal. Mutual cycles
share a joint ranking and every cyclic edge must decrease it; the exact source
spelling for differently shaped participants remains deferred.

Runtime recursive calls remain tail-only for constant-stack lowering;
proof-time and compile-time machines use the same clause without the runtime
tail restriction. Explicit state loops use the same ranking law when they
promise termination. Productive loops may omit the promise and run forever.

`ensures` remains partial correctness: it states what is true **if** a return
edge is reached. Result domains therefore cannot replace termination.
`reaches` remains a service-reach ceiling and cannot replace
termination either. The normalized artifact derives its terminal-outcome
classification from the termination guarantee, reachable outcomes, and
explicit premises without adding phantom `Completes<...>` surface syntax.

Pinned operation contracts, not reach rows or mere parameter mentions,
identify positive progress premises. A profile is an owner-classified atomic
domain declared with `satisfies ProgressProfile` and an `established by`
boundary route. It is opaque, predicate-free, sealed against downstream
classification or route extension, and established only with the exact
admitted grant/receipt. Public termination contracts author premise schemas
through ordinary requirements such as `requires scheduler in WeakFair`;
checked calls instantiate those schemas by exact argument substitution.
Derived instances must match a published schema, an exact local receipt, or a
build-bound provider premise in the component manifest. Provider schemas and
component demands retain the profile owner's exact closed `established by`
requirement routes so admission can check which receipt issuers are authorized;
the route catalog is never itself a receipt. The profiles do not entail proof
facts, and general trace logic and profile entailment remain deferred.

The ranking witness is excluded from published contract identity. Swapping one
valid view for another revalidates the implementation and proof cache only;
callers and external requirement bindings continue to see the same guarantee. The complete
ruling and acceptance register are frozen in
[termination_ranking_and_progress.md](../design_briefs/termination_ranking_and_progress.md).

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

If those obligations are discharged, the operation contributes guarantees: `self.health` is initialized and has the proven resulting facts. If not, the compiler carries the debt as an invariant window closed at the next consumption point, requires a different arithmetic mode, or requires an explicit checked/boundary construct — and rejects the code when the window cannot close.

This maps directly onto transition-system induction:

- Machine fields are variables.
- State parameters are action inputs.
- Transitions are guarded next-state relations.
- Value constraints are invariants or pre/postconditions.
- Invariant windows are local invariant weakening with mandatory restoration
  at consumption points.

Invariants are not RTTI. If proof fails, the normal result is a compiler diagnostic, not a hidden runtime tag check. Runtime validation may exist as an explicit debug or proof-emission mode, but it should not define the semantics.

Float invariants are also not fast-math flags. A proof that a value is `finite` or in `a..=b` does not automatically permit reassociation, signed-zero erasure, reciprocal transforms, or other approximate rewrites.
