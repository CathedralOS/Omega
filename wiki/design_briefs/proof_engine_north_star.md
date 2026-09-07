# Design Brief: Proof-Engine North Star

Status: **settled architectural direction; implementation is staged.** The
language guide governs landed source semantics, the terminal-Psi and proof-
kernel pages govern the artifact boundary, and `TASKS.md` owns remaining work.
This brief states the endpoint those increments converge toward. The
[contract-first mathematical proof direction](mathematical_proofs.md) governs
the source model: ordinary proof machines and trait bundles, general logic in
contracts, and explicitly selectable axioms. Its implementation remains open.

The endpoint is one typed framework with distinct value, proposition, and
effectful-computation judgments, explicit binding relevance, validity scope,
and trust provenance. `Prop` is the formula universe. Source automation is an
ergonomic elaborator into certificates checked by a small Psi-owned kernel;
recursive proofs certify one SCC with shared well-foundedness evidence and
per-edge descent, normalization cites exact conformance/law evidence, and the
human synopsis is derived from the accepted certificate. For Type occurrences,
`[erased]` remains orthogonal to multiplicity, validity, conservation, and
provenance; Prop terms are intrinsically erased and copyable. Relation
heterogeneity belongs to each relation's own carrier telescopes, never to a
global carrier-role convention.

## The ambition

Omega should make whole *classes* of existing tools redundant inside one
systems language:

- **Rust** — memory/concurrency safety, but by *proof* (ownership + borrows +
  logic errors proven away) rather than by a borrow-checker's conservative
  approximation.
- **SPARK** (Ada + `gnatprove`) — contract-checked systems code, discharged
  automatically by an SMT backend.
- **Lean / Coq / academic proof languages** — serious mathematics: "at some
  point we want to port a huge swath of mathematical proofs to Omega."

The first two are *near*; the third is the long pole. The point of this brief is
that they are not three separate projects — they are three rungs on **one**
proof engine, and the rung you can reach is set by a single architectural
choice.

## Primer: what a proof actually is

By Curry–Howard, **a proof of a proposition `P` is a value whose type is `P`** —
a *term*. A small **trusted kernel** does exactly one job: type-check that term
against the proposition. For derivation validity, that kernel is the trusted
base: a fixed set of inference rules plus explicit **axioms**. End-to-end PCC
also needs an authoritative translation from canonical artifact bytes to the
right proposition and a soundness bridge from that proposition to the artifact's
operational semantics. Those dependencies are explicit, versioned trust-graph
nodes rather than authority silently granted to proof automation.

**Tactics** exist because writing those terms by hand is brutal. A tactic is an
*untrusted metaprogram* that inspects the current goal and emits term fragments:

- structural — `intro`, `apply`, `induction` (one inference step);
- rewriting — `simp`, `rw` (apply known lemmas / equations — "shortcuts");
- **decision procedures** — `omega` (linear integer arithmetic), `ring`
  (commutative-ring identities), `linarith`. These discharge a whole goal
  automatically.

The load-bearing property: **tactics are untrusted.** A buggy tactic cannot
prove a falsehood, because the kernel re-checks the term it produced. So you can
pile on arbitrarily clever automation without growing the trusted base. (Axioms
are the opposite — they're what you *don't* prove; minimize them.)

Omega's source entailment engine provides automation for a bounded contract
fragment. Extending that automation and specifying a general mathematical
calculus are different obligations; neither substitutes for the other.

## Automation and checked derivations

Automation searches for proofs; the kernel checks explicit derivations in a
specified calculus. No terminating decision procedure decides all mathematical
truths. This does not prevent automation from supporting quantified statements
or from producing certificates for a useful fragment. A small checker is a
trust-minimization goal, not by itself a proof of mathematical expressivity.

The target combines automation for common contracts with explicit proof-machine
bodies for harder theorems. Both must produce checkable evidence with retained
assumptions. Lean-competitive mathematical capability is a long-term requirement,
not something established by changing declaration syntax or by one proof corpus.

## One typed core, three judgments

The destination is one explicit account of checking and trust, with these
judgments. This table does not settle a universe hierarchy or prohibit separately
identified mathematical foundations:

| Judgment | Subject | Runtime meaning |
|---|---|---|
| `Type` | mathematical and runtime values | depends on relevance and layout |
| `Prop` | formulas that may hold about values | no runtime representation |
| computation | effectful machines producing values or proofs | carries effects, work, failure, suspension, and termination |

`data` constructs values in `Type`. Contracts state formulas, and ordinary
checked machines establish their conclusions from premises. Traits and named
conformances group witness operations with their laws. Termination facts,
domain facts, quotient laws, and conservation equations share a checked
logical account without requiring a separate source declaration for formulas.

An object, a statement about it, a proof of that statement, and a decision
procedure are distinct. Internal `Prop` notation does not require a source
result type or another executable machine category. General logical expressions
and arbitrary mathematical function/predicate binders remain necessary even
where no decision procedure exists.

Selectable axioms must survive as exact transitive dependencies of checked
theorems. Changing axioms within a fixed calculus is not the same as changing
its universes, equality, computation, or proof-irrelevance rules. The latter
requires explicit foundation design and compatibility, not guessed reuse of
identically printed statements. The checker establishes derivability, not
consistency of arbitrary admitted axioms.

## Evidence dimensions

Proof erasure is not the whole evidence model. Every retained fact is described
along independent dimensions:

- **relevance:** whether it contributes runtime representation or behavior;
- **validity scope:** timeless, borrow-scoped, entry-scoped, state-versioned
  and invalidated by intersecting writes, or lease-scoped; and
- **provenance:** derived, certified, admitted, and the exact authority/evidence
  chain that supplied it.

`Prop` terms are unrestricted and copyable. Consumable authority belongs to an
affine or linear `Type` carrier, which may have zero runtime layout, and follows
Omega's existing `[copy]`/affine/`[linear]` rules. Erased relevance remains a
separate judgment for Type occurrences: an explicitly erased Type ghost may
still be linear or scoped even though it has no runtime representation.
Provenance attaches to the evidence, not to proposition identity, and composes
through every proof so a deployment profile can reject a proof closure
containing unacceptable admissions.

### Borrow compatibility over existing authority

Borrow reasoning applies the universe split directly. A proposition may prove
a relationship over already-established, versioned values, places, and
authority occurrences. It may not itself create, amplify, transfer, extend,
return, consume, or duplicate authority. The latter operations remain in the
Type/resource ledger because erased copyable evidence has no custody
disposition.

The relational obligation family is semantic; individual solvers are not new
obligation kinds. Structural projection, literal and symbolic interval
normalization, domain facts, arithmetic entailment, and explicit theorem
citation may all prove the same spatial-disjointness, spatial-containment, or
non-interference goal. They consume one shared path- and version-valid proof
context rather than run as a borrow pass followed by a proof fallback.

Loan formation freezes the exact place occurrences selected by its evaluated
arguments. Premises licensing compatibility must dominate that event and be
valid at the captured value/place versions. The derived conclusion is scoped
to the resulting loan occurrences, not to the later values of expressions
that selected them. Proof justification must be acyclic: a fact established by
a loan or its effects cannot authorize formation of that same loan.

The proof certificate and resource ledger meet but do not subsume one another.
The former records the normalized relation, exact premise tokens, derivation,
captured place occurrences, and authorized formation event. The latter records
owner lineage, access polarity, temporal containment, and restoration. Even a
loan over an empty place footprint retains its complete resource obligation.

The source relevance marker attaches to a binding occurrence:

```omega
data Certified<T> {
    value: T;
    proof [erased]: Valid<T>;
}
```

The erased binding remains in typed terms, semantic identity, validity and
obligation tracking, but contributes no runtime field, address, read, or
cleanup. It may be consumed by proof computation or statically authorize an
effectful call; it may not determine runtime data or control. Erasure never
discharges Type custody, content conservation, validity scope, or provenance.
A structurally zero-layout Type value is not implicitly erased: it remains an
ordinary value whose ownership and multiplicity are checked normally.
Only reliance on runtime representation or runtime cleanup is forbidden.

Layout and ABI use the erased-stripped form, while nominal identity and
semantic fingerprints retain erased bindings. Construction supplies an erased
term unless a visible accessible nullary constructor determines it
structurally; no general inhabitance judgment or implicit zero/default
construction is introduced.

Relevance does not assign relational roles to carrier parameters. Heterogeneous
indices are named by the proposition's independent left/right telescopes, and
the same carrier may support another relation with one shared telescope.
Erased proofs are irrelevant only after their proposition applications agree;
proof irrelevance never identifies evidence for distinct propositions.

## Witnesses and elimination

Use ordinary trait/conformance bundles for mathematical witnesses and their
laws. Repeated projection of one bundle preserves witness identity; distinct
bundles may supply different witnesses for the same statement. Statement
equality does not authorize equating witness values.

Existence and executable construction are distinct. Constructive proofs may
supply a witness bundle. Nonconstructive proofs may reason about existence under
explicit assumptions, including choice when accepted; resulting noncomputable
mathematical values remain proof-only. Erasure never licenses runtime extraction.

The source syntax and typing rules for general quantification, predicate
abstraction, and noncomputable values remain open implementation/design work.
Universe levels, equality, induction, and quotients must be specified before
claiming general mathematical coverage. Neither the current specialized
entailment engine nor static machine-symbol parameters already supplies that
whole foundation.

## Migration boundary

`PROOF-CONTRACT-MIGRATION` in [TASKS.md](../../TASKS.md) owns source replacement,
core examples, and evidence preservation through Terminal Psi and replay.
The proof cases in [Mathematical Proofs](mathematical_proofs.md#migration-acceptance)
must demonstrate witness composition, higher-order reasoning, nonconstructive
existence, axiom policy, and quotient laws. Optional naming syntax is not a
dependency of that work. Explicit relevance and effectful-computation work
remain separate requirements; bundling does not settle their semantics.

## Certified elaboration and review

Omega source presents a proof strategy, not every primitive inference. Local
computation, constructor reasoning, branch facts, contract extraction, and
licensed decision procedures may remain implicit at the source surface. That
compression does not grant authority: the elaborator must materialize a
certificate for every accepted conclusion, and the kernel checks the
certificate under explicit premises.

Two independent tests govern that split:

- **source visibility:** theorem, conformance, boundary, and other provenance-
  bearing dependencies remain explicit even when their resolution is total;
- **certificate strategy:** a total deterministic procedure may be replayed by
  the checker, while partial or heuristic proof search must emit evidence the
  checker can validate without repeating the search.

Totality alone never establishes soundness. A replayed normalizer is trusted
checker logic unless it emits a lower-level certificate. In either form, each
normalization node cites the exact selected conformance and law terms it used.
The conclusion inherits their complete trust closure: normalizing under one
admitted law makes the result admission-dependent rather than fully derived.

Recursive contracts need a distinct certificate rule. For each strongly
connected proof-call component, the certificate records the selected measure,
ranking relation, and one proof that the relation is well-founded. Every
intra-component application separately proves that its callee measure is
strictly below its caller measure. Only then may the callee contract enter the
local context as an inductive hypothesis. Calls outside the component use
ordinary contract application. This covers self recursion and mutual induction
without treating a circular contract citation as an ordinary call.

The primary review synopsis is derived deterministically from that checked
certificate. It reports the certificate fingerprint, recursive components,
implicit closure rules, exact cited laws, and trust closure. Source spans are
attribution metadata attached to certificate nodes; no second analysis may
reconstruct what probably happened. The source remains readable as ordinary
control flow, while the synopsis warns reviewers about logical work hidden by
that presentation and the certificate remains the complete authority.

## Current boundary

The source entailment engine already handles canonical polynomial, order,
range, congruence, and measured-recursion obligations. Terminal Psi has a small
structural certificate kernel, sealed admissions, exact accepted-premise trust
records, deterministic proof-bundle synopsis rendering, and kernel checkers for
recursive-component and law-normalization certificate shapes.

The source engine does not yet emit those terminal certificates. Quantifiers
remain unsupported; proof views do not yet have semantics; and the broader
contract/bundle, explicit-relevance, and effectful-computation migrations
remain open. `TASKS.md` is the authoritative queue for that gap.

Terminal-Psi artifact verification has a settled endpoint beyond that source
bridge. A total low-rung semantic-ledger definition consumes canonical bytes,
directly denotes each primitive operation, emits exhaustive canonical goals and
validity-scoped local premises, and performs no multi-node algebraic reduction.
Rust reduction remains untrusted only when it proves those goals with checked
certificates. Separate safety/partial-correctness and progress/termination
composition theorems connect accepted ledgers to the exact pinned operational
semantics. Until those theorems and the low generator land, current verifier,
reduction, and denotation dependencies remain explicit trusted-judgment nodes;
no clean artifact report may hide them beneath kernel acceptance.

## Implementation priorities

Current systems customers need sound contract checking, ownership, and
artifact replay. General mathematics additionally needs higher-order logical
terms, quantifiers, dependent mathematical values, and explicit foundation and
axiom rules. Neither a small kernel nor an ergonomic source syntax proves those
capabilities complete. Automation remains useful through this expansion, but a
failed search must not become acceptance of an unproved conclusion.

## Remaining research questions for the Lean rung

1. **Logic surface:** specify general quantification and arbitrary mathematical
   function/predicate abstraction, including nonconstructive existence.
   Bounded automatic instantiation is a proof-search strategy, not the limit
   on statements users may express.
2. **Kernel growth:** the initial terminal-Psi kernel checks typed scalar
   propositions, structural implication/conjunction proofs, and total closed
   judgments. Which additional term constructors and rules are necessary for
   quantified mathematics while keeping the trusted core small?
3. **Certificate bridge:** carry the live recursive/normalization kernel shapes
   through terminal Psi and emit them from source automation.
4. **`Real` / analysis:** general relation expressions, mathematical-function
   binders, and witness bundles must support the Cauchy/quotient construction.
   Demonstrate those proofs before claiming the replacement complete; runtime
   approximation policy remains open.
5. **Trust migration:** which existing automated judgments become total kernel
   primitives, which emit certificates, and which remain explicitly admitted
   while the terminal-Psi bridge is incomplete?

Universe/equality rules and foundation compatibility also require explicit
design. Selectable axioms are required independently of that work.

None of these block the near-term work; they're the gates on the long pole, and
this brief exists so they're chosen deliberately when the time comes.
