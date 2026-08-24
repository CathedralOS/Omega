# Design Brief: Proof-Engine North Star — Obsoleting SPARK, Rust, and Lean

Status: **settled architectural direction; implementation is staged.** The
language guide governs landed source semantics, the terminal-Psi and proof-
kernel pages govern the artifact boundary, and `TASKS.md` owns remaining work.
This brief states only the endpoint those increments converge toward.

The endpoint is one typed framework with distinct value, proposition, and
effectful-computation judgments, explicit binding relevance, validity scope,
and trust provenance. `Prop` is the formula universe. Source automation is an
ergonomic elaborator into certificates checked by a small Psi-owned kernel;
recursive proofs certify one SCC with shared well-foundedness evidence and
per-edge descent, normalization cites exact conformance/law evidence, and the
human synopsis is derived from the accepted certificate. For Type occurrences,
`[erased]` remains orthogonal to multiplicity, validity, conservation, and
provenance; Prop terms are intrinsically erased and copyable. Relation
heterogeneity belongs to each proposition's own carrier telescopes, never to a
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

The key observation for Omega: **the entailment engine we already have is a
decision-procedure tactic** — the same category as Lean's `omega`/`ring`. The
difference from Lean is purely architectural, and that difference is the fork.

## The fork

| | Automation-as-base | Kernel + tactics |
|---|---|---|
| Examples | **Omega today**, F\*, Dafny, SPARK | Lean, Coq, Agda |
| Trusted base | the whole prover/SMT engine (large) | a tiny kernel (small) |
| Proof terms | none — the engine decides directly | every proof is a kernel-checked term |
| Ergonomics | fully automatic where it works | tactics/term-mode for the hard parts |
| Reach | only what the procedures decide — **no arbitrary quantified / higher-order math** | **all of mathematics** |
| Failure mode | engine bug = silent unsoundness | kernel is small enough to trust; tactic bugs caught by re-check |

Omega is unambiguously in the left column today, and doing it well. The left
column is *exactly* what "kill SPARK / Rust" needs. It is *fundamentally
incapable* of "kill Lean" — undecidability means no decision procedure covers
all of math, and with no kernel there is nothing to check a human-supplied proof
against.

## The synthesis to build

Neither column alone is the target. "Kill Lean inside a systems language" does
**not** mean adopting Lean's manual misery — it means building the synthesis
that neither camp ships cleanly:

1. **A tiny trusted kernel** so arbitrary proofs are *expressible and checkable*
   (this is what unlocks general math and shrinks the trusted base).
2. **Automation as the front line** so the basic 95% — bounds, overflow, ranges,
   ring identities, bounded induction — is discharged with *zero* tactics, the
   way the entailment engine already does it.
3. **An escape hatch to an explicit proof** for the genuinely-hard 5% that
   automation can't crack; the kernel checks it. An SMT-style procedure can even
   emit a kernel-checkable certificate, so automation and kernel compose.

This is strictly better than Lean (far less hand-proving) and strictly better
than pure-SMT/SPARK (it can do the hard cases at all, with a smaller trusted
base). F\* is the nearest existing point on the map (SMT automation over a
dependently-typed, kernel-checked core) — the closest prior art to study.

## One typed core, three judgments

The destination is one calculus, not several unrelated proof mechanisms:

| Judgment | Subject | Runtime meaning |
|---|---|---|
| `Type` | mathematical and runtime values | depends on relevance and layout |
| `Prop` | formulas that may hold about values | no runtime representation |
| computation | effectful machines producing values or proofs | carries effects, work, failure, suspension, and termination |

`data` constructs values in `Type`. `proposition` constructs formulas in
`Prop`. A proof is an inhabitant of a proposition. An ordinary checked machine
may construct values or establish proofs; `requires` and `ensures` elaborate to
erased proof flow at the machine entry and terminal edges. Trait laws,
termination facts, domain facts, quotient laws, and conservation equations
therefore share one proposition/proof account even when their source surfaces
remain specialized and ergonomic.

This unification preserves three distinctions:

- an object such as `Nat` is not a claim about that object;
- a proposition is not the particular evidence that establishes it; and
- a proof is not a decision procedure returning `bool`.

The `Prop` universe is internal and proof-only. Source proposition families are
not runtime values, fields, layouts, or machine result carriers. A Boolean
expression in a contract is the decidable special case: it denotes the
proposition that the expression evaluates to `true`.

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

A fact-only proposition hides all proof identity. A witness-bearing nominal
proposition publishes one opaque evidence interface. Selected carrierless
conformance is one representation of an inhabitant of that proposition, not a
parallel logical mechanism. The normalized interface is fingerprinted public
proof content; changing it is a breaking proof-API revision while the nominal
proposition symbol remains stable.

The `evidence Interface;` clause publishes the elimination contract. Named
`requires` bindings retain exact incoming evidence terms and project their
members in proof-only computation; named `ensures` bindings expose exact terms
through the erased proof-output lane. Producer conformances are selected
privately at introduction and never enter mathematical signatures. A witness that must
influence runtime computation belongs in an ordinary `Type`-level dependent
pair whose relevance is tracked explicitly; erasure alone does not authorize
eliminating a `Prop` inhabitant into runtime `Type`.

The kernel and artifact keep three identities separate: the nominal
proposition application, the retained evidence term that determines stable
opaque projections, and the derivation provenance that records trust. Two
derivations may reach the same term through different admitted premises; two
terms may inhabit the same proof-irrelevant proposition while carrying
different hidden witnesses.

Result-case guarantee groups are a contract-indexing form, not another proof
carrier. `ExactCase -> { ... }` attaches each contained named or unnamed
guarantee to one nominal result case. Matching the sum tag activates its facts;
the group creates no proposition, domain, package, or evidence identity. Named
rows retain selectable erased terms, while unnamed rows retain only their
proved proposition and provenance. Referenced borrow and revision scopes remain
part of either row's validity and are invalidated normally by intersecting
writes.

## Migration boundary

The current rule that recursive/non-layoutable mathematical data becomes
proof-only remains live until explicit relevance replaces it. During migration,
an explicit relevance annotation takes precedence; structural classification
is then legacy inference for unannotated declarations. The destination treats
recursive and other non-layoutable values as ordinary `Type` inhabitants that
may occupy erased bindings but not runtime-relevant ones; constructor tags as
well as fields contribute representation. The later effectful
computation judgment must account for Omega's states, transitions, effects,
suspension, failure, work, and multiplicity rather than pretending machines
are pure dependent functions. Neither migration blocks proposition families,
terminal-Psi proof identity, or the present certificate kernel.

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
proposition-family, explicit-relevance, and effectful-computation migrations
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

## The three kills, sequenced

- **SPARK — near, mostly hardening.** Omega is already SPARK-shaped (first-class
  contracts + automated discharge) and arguably ahead (built-in induction,
  proof-oriented from the ground up rather than bolted onto Ada). The S4
  narrowing work and domains-over-carriers are this rung: make more obligations
  discharge automatically.
- **Rust — in progress.** Ownership/borrows + logic-errors-proven-away
  (panic-as-effect) + exact-arithmetic-by-default. This is the systems-safety
  story already being executed.
- **Lean — the long pole.** Still needs quantifiers, a much broader proposition
  and proof-term vocabulary, and the automation-to-certificate bridge into the
  now-live small Psi kernel. This is most of what Lean *is* — a multi-year arc.

The friendly part of the sequencing remains: the automation-first base delivers
SPARK/Rust-class value while the initial kernel grows, and **the kernel can
become the backstop without discarding the automation** — they compose
(automation tries first; kernel-checked explicit proof catches the rest). So the
end-state is coherent and incremental, not a rewrite.

## Remaining research questions for the Lean rung

1. **Logic surface:** what fragment of quantification do we admit, and how is it
   discharged — bounded instantiation (stays automated) vs general (needs the
   kernel)?
2. **Kernel growth:** the initial terminal-Psi kernel checks typed scalar
   propositions, structural implication/conjunction proofs, and total closed
   judgments. Which additional term constructors and rules are necessary for
   quantified mathematics while keeping the trusted core small?
3. **Certificate bridge:** carry the live recursive/normalization kernel shapes
   through terminal Psi and emit them from source automation.
4. **`Real` / analysis:** the proof-side Cauchy/evidence/quotient construction
   and dedicated `proposition` surface are settled; implementing the
   proposition-family/index-telescope fragment gates its
   implementation, while the runtime approximation-policy surface remains
   open.
5. **Trust migration:** which existing automated judgments become total kernel
   primitives, which emit certificates, and which remain explicitly admitted
   while the terminal-Psi bridge is incomplete?

None of these block the near-term work; they're the gates on the long pole, and
this brief exists so they're chosen deliberately when the time comes.
