# Terminal fact validity

These rules decide when a fact may justify a later obligation in Terminal Psi.
Psi checking applies them when it discharges operation, call, crash-site and
arrival obligations; the Omega optimizer applies them when it consumes checked
facts. See the [product](product.md), [encoding](encoding.md), and
[observation](observations.md) contracts. Runtime cycles follow
[control flow and ranking](control_flow.md).

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
  same value; potentially aliasing writes, unknown call effects, missing stores,
  and unproved cyclic arrivals prevent introduction. The store/read types and
  dominance must check independently. Subsequent writes cannot change either
  captured SSA value. This does not introduce symbolic store summaries or a
  floating-point equality rule.

The scalar block-predicate rule generalizes range induction to matching-token
merges without introducing a union of path facts. Its prerequisites are
exact predicate formation/scope, exhaustive arrival reconstruction, checked
arrival derivations, simultaneous substitution, and the existing cyclic cuts.
No branch-local identity becomes a destination premise. This revision does not
claim a proved universal composition theorem.

**Implementation status (re-verified `52d95a9d76f`, linux x86-64):** the cyclic
controls above have checked equivalents on the source path.
`typed-trees-to-checked-trees`'s `checks/ranges/incoming_guards.rs` meets
multi-predecessor joins by intersection — a fact holds at a join's entry only
when every incoming edge carries it — and declines loop-header bounds to the
invariant pass. `checks/ranges/loop_invariants.rs` (`collect`/`seed` entry
points) and `checks/contracts/exits/cyclic_headers.rs` implement the cyclic
reconvergence rule: a re-entered state's header conjunct is proposed, proved at
every arrival (entry arrival, induction-step backedge under simultaneous
argument substitution, and each base edge on ambient entry facts), and a failed
arrival discards every proposal — so a uniformly forwarded parameter keeps its
exact typed origin and an unestablished cyclic origin supplies no equality. What
remains open is recorded on GENERAL-CYCLIC-EXECUTION: composed cyclic Unit plans
and the per-callee call/return composition the admission roster cannot yet
express.

Call coverage reconstructs one exact obligation for every callee `requires`
clause. Instantiation independently checks arity, binder kinds/types,
capture-free positional substitution, pre/post versions, moves and reborrows,
outcome guards, crash routes, and evidence lifetime. Missing a clause and
substituting the wrong caller term are distinct failures.

## Entry facts and crash coverage

Entry hypotheses bind exact invocation formals or structural entry paths, with
their retained types and access. Current body values are not entry snapshots.
Named-state requirements do not become ambient invocation-entry assumptions.
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
