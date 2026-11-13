# Terminal fact validity

These rules decide when a fact may justify a later obligation in Terminal Psi.
Psi checking applies them when it discharges operation, call, crash-site and
arrival obligations; the Omega optimizer applies them when it consumes checked
facts. See the [product](product.md), [encoding](encoding.md), and
[observation](observations.md) contracts. Runtime cycles follow
[control flow and ranking](control_flow.md).

## Establishing facts on receipt

The [product admission contract](product.md#publication-and-separate-consumption)
determines whether producer-established judgments may be reused. A trusted
producer's exact artifact retains their subjects, substitutions and assumption
dependencies. Later stages preserve the validity conditions below; a theorem
name or success flag supplied by an untrusted artifact grants no premise.

Receipt requires neither rediscovery of source theorems nor replay of carried
certificates. The local kernel remains part of source proving. Canonical proof
encoding alone does not establish the encoded judgment or its correspondence to
an executable operation.

Interpreter operation checks use actual values, storage and permissions even
for trusted artifacts. Untrusted interpretation, when admitted under the
containment contract, cannot use asserted facts to skip those checks or gain
host authority. It does not establish arbitrary logical claims. Optimization
and native lowering may consume producer judgments only from trusted input.

## Premise availability

A premise must dominate its use and remain valid on every path to it. Acyclic
logical ordering alone does not establish availability.

- An acyclic join may introduce a merge token only from matching valid tokens on
  every predecessor. A fact from one arm does not survive the join.
- Cyclic reconvergence requires checked invariant establishment and preservation,
  not the acyclic merge rule.
- Scalar block invariant arrival questions come from the semantic predicate
  roster and every actual incoming edge.
  Each question is the exact simultaneous successor substitution of the scoped
  predicate. Every arrival must check before the destination predicate can
  justify downstream obligations. Integer ranges use this same predicate rule.
- Each successor applies its own argument-to-parameter substitution.
- A partial operation establishes its result equation only on its normal
  successor, after its safety obligation is discharged. That equation cannot
  prove the operation that creates it.
- A primitive read may introduce an equality between its copied SSA result and
  an exact reaching stored SSA value. Every actual predecessor must supply that
  same value; potentially aliasing writes, unknown call effects, missing
  reaching-value evidence, and unproved cyclic arrivals prevent introduction.
  The store/read types and
  dominance must check independently. Subsequent writes cannot change either
  captured SSA value. This does not introduce symbolic store summaries or a
  floating-point equality rule.

The scalar block-predicate rule generalizes range induction to matching-token
merges without introducing a union of path facts. Its prerequisites are
exact predicate formation/scope, exhaustive arrival reconstruction, checked
arrival derivations, simultaneous substitution, and the existing cyclic cuts.
No branch-local identity becomes a destination premise. This revision does not
claim a proved universal composition theorem.

Call coverage reconstructs one exact obligation for every callee `requires`
clause. Instantiation independently checks arity, binder kinds/types,
capture-free positional substitution, pre/post versions, moves and reborrows,
outcome guards, crash routes, and evidence lifetime. Missing a clause and
substituting the wrong caller term are distinct failures.

## Equality relations

[Equality bridge evidence](../proofs/contracts.md#equality-bridges-and-comparison-facts)
licenses only its checked direction for the exact selected operation, subjects
and instantiation. A true executable comparison is not generic identity.
Synthesized structural comparison evidence retains field selections and active
case guards; float leaves retain IEEE truth conditions. Raw float identity
preserves representation bits and cannot be inferred from payload-erased
meaning equality. Pure call congruence requires the same admitted operation and
unchanged argument values, including any observed referent versions. Results
from other call frames require checked exact substitution; a different value
version or selected implementation supplies no interchangeable fact.

## Borrowed read dependencies

Under `shared-observation-denotational-content`, an admitted predicate's
argument state includes the borrowed storage it actually observes, with exact
places, backing identity, value versions and applicable access/loan conditions.
A reference carrier's identity does not stand for unchanged referent contents.
Apply the same dependency rules through record fields, slice/array elements and
nested reference paths rather than classifying an entire type as forbidden
because a reference occurs inside it.

A read of slice length depends on that metadata value, not automatically on
the elements or their referents. Reading one ordinary field does not observe
an unused reference field. Following a reference depends on the selected
referent identity and the observed contents version, including intermediate
path selections. Input validity, lifetime, qualifications, custody and access
remain independently required even when a particular field is unread.

Direct and helper reads compose through sound checked summaries and exact
actual-argument substitution under the ordinary interface rules. A known
metadata-only summary may retain that precision. An unknown read footprint is
conservative, never empty; a missing summary cannot justify assuming no reads
or no dependency on possibly reachable storage. A proof requiring unavailable
stability or place correspondence has no such premise.

Call congruence and reused result facts require all relevant observed state to
remain valid. Overlapping or potentially aliasing writes invalidate facts about
current storage; proved-disjoint writes preserve them. A retained fact about
an earlier captured value does not become a fact about the current value by
reusing the same pointer or source expression. Shared loans retain their normal
scope and compatibility rules, not permanent transitive immutability.

External/device and atomic observations require their own admitted stability
contract or reasoning about an authorized captured observation. Atomicity alone
does not establish repeatability. This rule grants no reference escape,
resource duplication, new canonical generic atom or permission to evaluate a
runtime borrowed graph as a compile-time constant. See
[proof admission](../proofs/contracts.md#borrowed-proof-inputs) and
[access supply](../resources/access.md).

## Restored scalar values

Under `restored-scalar-value-facts`, a valid initialized ordinary storage place
at entry may supply its exact reaching value version; it need not have a store
inside the current body. A body read can be related to that entry version only
when the place and absence of intervening mutation are established. Missing
reaching-value evidence does not authorize inventing an entry value or zero.

For integer copies `copied` and `saved` of that same version, their equality
survives later writes to the original storage. A restoring store of `saved`
introduces an equality for the new storage version at its captured destination.
A guard on `copied` can then prove bounds on a later read of that version.
The proof retains both equalities and checks normal dominance, actual selectors,
write effects and all predecessor obligations. It does not equate entry and
current storage merely because their source paths have the same spelling.

Unknown interference or an unproved place/value correspondence prevents the
affected equality from being used. Bounded analysis may require a fresh proof;
it must not assume restoration when tracking is exhausted. Existing floating-point
observation and equality rules are unchanged.

## Mutation and current-storage facts

A call's write frame invalidates facts about overlapping current storage.
Postconditions or sound local summaries may establish facts for the successor
value version. Facts about a captured earlier value remain true of that
snapshot, not automatically of the place's current contents. Proved-disjoint
sibling places retain their facts.

Declaring a binding without `mut` is not proof that every place reachable
through it is stable: a reference carrier can be immutable while its referent
permits writes. Check the exact place, loan authority and value version.
[Writable-loan admission](../language/ownership.md#owned-local-mutability)
does not replace this analysis; a legal writable call must not leave an
old current-value fact available to prove an exit contract. Literal and
computed initializers follow the same rule.

## Entry facts and crash coverage

Entry hypotheses bind exact invocation formals or structural entry paths, with
their retained types and access. Current body values are not entry snapshots.
Under [contract inputs](../language/state_contracts.md#contract-inputs-and-working-values),
by-value names in postconditions and declared crash-route guards retain the
invocation's logical input identities even when their executable bindings are
mutable. Retain the input-to-working-value relationship at entry, and version
subsequent working-storage writes independently. Caller substitution and body
verification must refer to the same inputs; borrowed observations still use
their appropriate storage revisions, not an invented deep snapshot.
Nested-machine requirements do not become ambient invocation-entry assumptions.
A block parameter is a forwarded formal only when every incoming edge proves
the same typed origin; unknown, conflicting, or unestablished cyclic origins
supply no such equality.

Direct crash-site checking proves every asserted guard from independently
established entry and pre-terminator facts on every incoming path. A checked
contradiction may close an impossible path. Neither the site guard nor supplied
producer evidence becomes an assumption. All-crash bodies still retain their
entry contracts.

Entry-only call-ceiling coverage proves the same-cause union of published
alternatives without claiming an individual disjunct is true. Disjunction
elimination checks each branch under only its own alternative and discharges
that assumption. Semantic operation facts do not occupy requirement slots.

Boolean denotation and polarity consequences derive from the exact selected
operation, not its spelling or a desired callee requirement. Caller operands
keep caller operator meaning under substitution. Equality transport and integer
strict/inclusive conversions require their checked rules; proof traversal does
not silently reorder a canonical goal. Comparison-only normalization for crash
coverage does not change the codec's canonical ordering.

A `PredicateDenotation` inference converts one already proved proposition to
another only when the proof owner's bounded predicate-denotation normalization
gives identical results for both under their declared types. Its child is
checked under the unchanged original premises; the outer conclusion must still
equal the exact reconstructed obligation. Normalizing a premise for search
therefore requires an explicit conversion of its original citation. This rule
neither substitutes contextual SSA definitions nor grants an unproved
proposition merely because it has a normal form. Resource exhaustion rejects the
conversion.

Constructive [contradiction elimination](../proofs/classicality.md#contradiction-elimination)
may convert an already checked `Falsehood` to `Falsehood AND goal` and eliminate
the conjunction to establish a well-formed goal under those same premises.
The false premise must be proved, not inferred from search failure, and its
dependency and branch scope remain attached to the conclusion. This closes an
impossible arrival without manufacturing facts for another reachable path or
waiving operand formation and access obligations.

Besides Boolean structure, this conversion decides equality and order between
two literal fixed integers of exactly the same carrier using the primitive
judgment's signed, full-width comparison. True and false relations normalize to
`Truth` and `Falsehood`; false is not inferred from a proof-search failure.
Open integer relations, mathematical arithmetic, IEEE relations, and storage
observations remain unchanged. This is a bounded trusted conversion, not a
new assumption or a claim of a proved general normalization theorem.

`ValueEqualityTransport` additionally carries a nonempty ordered list of proved
scalar equations, each oriented from an exact scalar value identity to its
term. Every equation and the proposition being transported is checked under
the unchanged original premises. The bounded equality-transport owner applies
only those carried equations to that proposition and the desired conclusion;
their Boolean-denotation normal forms must match. Constructor tags, types and
operand order are preserved. Cyclic expansion and resource exhaustion reject.
This is explicit equality reasoning, not permission for the Terminal producer to
collapse SSA definitions or for proof search to rewrite ambient premise slots.
All cited equations remain dependencies of the resulting proof.
