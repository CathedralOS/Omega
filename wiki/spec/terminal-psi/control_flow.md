# Control flow and ranking

Terminal control is an explicit graph of typed blocks, `Jump` and `Conditional`
edges, successor arguments, and terminals. Cycles introduce no separate loop
language, implicit induction variable, or optimizer-owned progress rule.
[Calls and outcomes](calls_and_outcomes.md) defines exits and suspension;
[ownership](ownership.md) defines transfers and cleanup.

## Graph and frontier

The verifier derives each strongly connected component (SCC) from the actual
graph, including its members, entries, exits, and internal edges. Component
identity binds the owning machine and canonical topology. A producer may cite
that identity, not assert a different topology. Reducibility is an optimizer
classification, not an execution-legality condition.

Every reachable block and edge participates in dominance, successor binding,
and frontier checking, including exits downstream of cycles. Scalar arguments
match the complete target telescope; their values may differ across arrivals.
Structural arrivals agree exactly on places, claims, partial custody, and
cleanup debt. Deterministic transfer and comparison of all arrivals establish
the ownership fixed point. An acyclic scheduling remainder cannot hide an exit.

An edge performs only its authored transfers and discards. Closing a cycle
does not itself dispose anything. Cleanup belongs to a real exit or explicit
discard. Selected scalar operands materialize before structural cleanup and
successor binding commit.

## Safety and progress

A productive component may remain unranked and execute indefinitely. Safety
checking does not prove termination. All normal returns, including returns
after a cycle, must establish the published guarantees. An infinite path has
no normal return; it does not supply facts to paths that do return.

A fact from an acyclic prefix or one iteration is not a loop invariant.
Proof scheduling may cut edges only in its working graph, not executable
semantics, and must not assume the omitted incoming facts at those targets.
General invariant evidence must be checked before importing its conclusions.

A termination certificate binds a well-founded relation, block ranks, and
every derived in-component edge. Source measures normalize to this relation
and decrease evidence; the verifier checks evidence rather than searching for
a measure. For the natural-rank form:

| Edge role | Required comparison |
| --- | --- |
| Preserving implementation step | `successor_rank <= source_rank` |
| Strict descent | `successor_rank < source_rank` |

Removing strict edges must leave an acyclic graph: every cycle encounters
descent, not just the cycles selected by a traversal. Preserving implementation
steps do not relax the authored state-transition decrease requirement.

Rank substitution follows the actual selected scalar and structural successor
arguments. A length observation belongs to its descriptor binding; rebinding
the descriptor cannot reuse the previous observation as the new extent.

One grouped control certificate covers each reconstructed component, with a
shared well-foundedness citation and evidence for every internal edge. Rank
bindings and comparisons enter proof-question identity separately from topology.
Missing, surplus, reordered, or substituted evidence rejects. A rewrite changing
topology, carried state, or decrease edges invalidates the old certificate and
requires verification again. Ranking alone grants neither a quantitative
[logical-work bound](../resources/logical_work.md) nor native realization.

## Proof-only recursion

A proof-call component is not an executable control-flow SCC. Its semantic key
binds the rank relation, closed finite-inductive proof-type graph, member
contracts and rank parameters, and exact statement/expression/transition call
coordinates. A strict structural-subterm path must be nonempty, resolve every
field, and return to the component's rank type. It grants no runtime layout.

The verifier reconstructs one ranking-relation identity, one well-foundedness
question, and one decrease question per call row before checking grouped
evidence. Component acceptance remains distinct from flat contract evidence.
Source production retains the complete reachable proof-call closure and exact
hermetic declaration identities, not frontend handles. The general citation
rules belong to [mathematical proof contracts](../proofs/contracts.md#citation-and-induction).

## Correspondence

Verification establishes the stated Terminal semantics, not that a producer
compiled the intended source. Source-to-Psi correspondence and Psi-to-target
refinement are separate obligations. Producer identity substitutes for neither.
Consumer-specific implementation limits are documented beside the
[verifier](../../../omega-rust/psi/semantics/terminal-verifier/README.md#cyclic-control)
and [native entrance](../../../omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/README.md#ranked-native-admission).
