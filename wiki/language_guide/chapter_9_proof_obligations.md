# Chapter 9: Proof Obligations

An operation needs facts before it is safe to use. Checking establishes those
facts from types, live program state, contracts, and explicit proofs. It does
not normally insert a runtime check when proof search fails.

[Chapter 7](chapter_7_types_constraints_invariants.md) introduces contracts;
[Chapter 8](chapter_8_domains.md) introduces qualification. This chapter connects
them to the compiler's obligations. The reference is
[proof contracts](../spec/proofs/contracts.md).

## Vocabulary

| Term | Meaning |
| --- | --- |
| Fact | Something established for an exact subject and program point. |
| Requirement | A condition a caller or incoming edge must establish. |
| Guarantee | A conclusion available after the applicable outcome. |
| Obligation | A judgment checking must discharge. |
| Invariant | A condition restored at its required consumption boundaries. |
| Contract | The invocation's logical and operational requirements/guarantees. |
| Admission | Explicit acceptance of an exact assumption or opaque contract. |

A boundary declaration does not prove its own guarantees. Checked implementations
prove their contract; opaque ones retain the required admission. Inline assembly
likewise needs the selected instruction's checked or admitted semantics, not a
comment asserting its behavior.

Ordinary operations create obligations without a separate proof declaration:

- Indexing needs bounds and permitted access.
- Arithmetic needs its selected policy's validity conditions.
- Borrowing needs valid storage, compatible loans, and sufficient access.
- A transition establishes its target frontier and requirements.
- A return establishes its result and applicable guarantees.
- Cleanup needs a legal disposition, prerequisites, and operational contract.
- Task start needs ownership and resources, including accountable rejection.

Logical proof does not create runtime authority. Resource establishment and
conservation keep their own judgments.

## Everyday Shape

```omega
machine increment(value: u32) -> u32
requires value < 100
ensures result <= 100
{
    value + 1
}
```

The caller establishes `value < 100`. The body then proves both the Exact
addition's range and the result guarantee. The caller receives that guarantee
only for the successful returned value.

An annotation is a demand, not evidence. Giving an empty body the same
postcondition would not establish it. Stronger theorems can be supplied by
checked machines when automatic reasoning is insufficient; their exact premises
still need to hold at each use.

## Termination Claims

Termination is separate from a postcondition. `ensures` describes a reached
return, not whether a return is eventually reached.

```omega
boundary trait FiniteReader {
    machine read_all(&mut self, out: &mut Vec<u8>) -> ReadResult
        terminates;
}
```

The requirement promises a terminal outcome under its explicit premises.
A checked cyclic implementation supplies a private ranking witness, for example:

```omega
terminates by remaining -> Nat::Descending;
terminates by items -> Slice::Length;
terminates by index -> Nat::IncreasingTo(limit) in 0..=limit;
```

These are illustrative clause forms, not complete algorithms. Every relevant
cycle needs the selected well-founded decrease. A view owns its normalization;
an increasing counter needs a finite bound. Short form `terminates by n`
requires a carrier-declared canonical ranking, not the only visible measure.

Private acyclic bodies can derive a local summary. Exported or abstract calls
rely on the authored/inherited promise; an omitted public promise cannot be
inferred from today's implementation. Changing a valid private witness does not
change the public guarantee.

Runtime recursive calls are tail-only and lower to iteration. Proof/compile-time
recursion can be non-tail when admitted. Productive state loops may omit a
termination promise; that supplies neither fairness nor bounded response.

A quantitative work bound is another obligation: proving eventual termination
does not by itself calculate a fixed number of iterations. Opaque progress
premises need exact admitted provenance. Reach alone is not progress evidence.
See [termination and progress](../spec/language/termination.md) for mutual
cycles, measures, and provider premises.

## Example

```omega
data Player {
    health: i32 in 0..=100;
}

machine Player::take_damage(&mut self, damage: i32)
requires damage >= 0
requires damage <= self.health
{
    self.health -= damage;
}
```

At entry, health is established in range and the receiver supplies exclusive
access. The two requirements establish that subtraction neither overflows nor
takes health below zero. Assignment preserves the field's domain.

If a multi-write update temporarily breaks a cross-field invariant, an
[invariant window](chapter_11_invariant_windows.md) may defer that invariant's
restoration until the next required consumption point. It cannot defer an
invalid arithmetic operation, out-of-bounds access, or missing loan authority.

Failure to prove a condition is neither its negation nor permission to assume
it. Supply a proof, establish it with an explicit checked branch/validator, use
a deliberately different valid operation contract, or reject the use. A debug
check is not the language's safety proof.

The same discipline applies to floats: proving `Finite` does not authorize
reassociation, signed-zero erasure, or approximate rewrites. Optimization needs
its own observation-preservation proof.
