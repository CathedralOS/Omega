# Chapter 9: Proof Obligations

Typed states and bounded values imply compiler-generated obligations.

## Vocabulary

Omega's proof model should use a small set of terms consistently.

- Facts are what the compiler currently knows.
- Requirements are facts an operation needs before it may run.
- Guarantees are facts an operation promises after it runs.
- Obligations are proof or check work the compiler must discharge.
- Invariants are facts that must remain true across a boundary.
- Contracts are requirements plus guarantees.
- Trust is an accepted authority for guarantees that Omega cannot prove from Omega code.

Short form:

```text
values carry facts
operations have contracts
contracts create obligations
the compiler proves obligations using facts
operations contribute guarantees
invariants are facts that must survive boundaries
trust explains why unproved guarantees are accepted
```

For ordinary Omega code, users should not have to write every contract explicitly. The compiler knows the contracts for assignment, arithmetic, field access, transitions, borrows, and similar language operations.

For boundary code, contracts must be explicit. Host APIs, inline assembly, target intrinsics, and trusted packages sit at the edge of Omega's semantic world. The compiler cannot honestly infer their behavior unless the toolchain or author supplies a contract.

Likely obligations:

- Every assignment into a bounded location preserves the bound.
- Every terminal expression of a typed state satisfies the declared return value type.
- Every transition into a typed state provides compatible arguments.
- Every typed transition satisfies return value compatibility.
- Every transition dispatch arm establishes the assumptions needed by its target.
- Every `relax` scope re-establishes all relaxed invariants before exit.
- Every transition leaving a `relax` scope either re-establishes the invariant or carries an explicit proof obligation into a compatible target.
- Every generic invariant is instantiated with compile-time or proof-visible facts.
- Every float invariant is checked as a semantic fact, not treated as an optimization permission.

## Example

```omega
data Player {
    health: i32[exact, range<0, 100>];
}

machine Player::take_damage(
    &mut self,
    damage: i32[range<0, 100>]
) {
    self.health -= damage;
}
```

The value `self.health` carries facts: it is an initialized machine integer, it is mutable through `self`, it uses exact arithmetic, and it must remain in `range<0, 100>`.

The subtraction operation has requirements: `self.health` must be mutable, `damage` must be compatible, and exact subtraction must not underflow or overflow.

The assignment back into `self.health` creates obligations: prove the arithmetic is valid and prove the resulting value satisfies the field invariant at the required boundary.

If those obligations are discharged, the operation contributes guarantees: `self.health` is initialized and has the proven resulting facts. If not, the compiler must reject the code, require a different arithmetic mode, require a `relax` scope, or require an explicit checked/trusted boundary depending on the construct.

This maps well onto TLA+ style action checking:

- Machine fields are variables.
- State parameters are action inputs.
- Transitions are guarded next-state relations.
- Value constraints are invariants or pre/postconditions.
- Relax scopes are local invariant weakening with mandatory restoration.

Invariants are not RTTI. If proof fails, the normal result is a compiler diagnostic, not a hidden runtime tag check. Runtime validation may exist as an explicit debug or proof-emission mode, but it should not define the semantics.

Float invariants are also not fast-math flags. A proof that a value is `finite` or in `range<a, b>` does not automatically permit reassociation, signed-zero erasure, reciprocal transforms, or other approximate rewrites.
