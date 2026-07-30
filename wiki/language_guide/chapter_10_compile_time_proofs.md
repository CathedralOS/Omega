# Chapter 10: Compile-Time Proofs

Compile-time proofs are not a second programming language.

They are ordinary machines whose contracts are checked as evidence. If a
machine is used only to establish facts, it emits no runtime code.

The basic shape is:

```text
requires + body facts -> ensures
```

If the checker can prove that implication, the machine is a proof artifact. If
it cannot, the contract is only an unchecked promise and must be rejected or
treated as an explicit boundary.

## Machines As Proofs

This machine proves a simple ordering fact:

```omega
machine distinct_indices(
    i: u64,
    j: u64
)
requires
    i < j
ensures
    i != j
{
}
```

The empty body is valid only if the checker can prove the guarantee from the
requirement and built-in arithmetic/order rules.

This machine proves a closed arithmetic fact:

```omega
machine pythagorean_3_4_5()
ensures
    3nat * 3nat + 4nat * 4nat == 5nat * 5nat
{
}
```

The checker reduces both sides to the same `Nat` value, then closes the equality
by reflexivity. The body does not need to simulate computation.

## Typed Facts

Proof facts must be typed.

```omega
3nat * 3nat
```

is math over `Nat`.

```omega
3i32 * 3i32
```

is machine arithmetic and carries machine obligations such as width and
overflow behavior.

The same operator spelling can exist in both worlds. The operand types decide
which proof rules apply.

## Proof-Only Data

`Nat` and its kin are proof-only types: unbounded, with no machine layout, no
ZII obligation, and no runtime existence. Nothing declares this — the
structure does. Recursive data is legal, and recursion is what makes a type
proof-only:

```omega
data Nat {
    case Zero;
    case Succ(n: Nat);   // recursive: no layout is derivable — proof-only
}
```

Working rules:

- **Proof-only is computed, never spelled.** A type is proof-only when it is
  recursive (directly or mutually) or any field's type is proof-only. There
  is no marker; writing recursive data is the opt-in, and diagnostics name
  the classification ("`Nat` is proof-only: recursive data has no layout").
- A proof-only value may appear **only in fact positions** — `requires`,
  `ensures`, `where` clauses, domain bodies — and in proof-stratum machine
  bodies. It never has a size, an address, or a zero value.
- A machine whose signature mentions a proof-only type is itself proof-only:
  it is evaluated by the checker, never lowered.
- **The checker computes where values exist and rearranges where they do
  not.** `Nat`/`Int`/`Rat` facts evaluate with exact unbounded arithmetic
  (`3nat * 3nat` reduces to `9nat`); facts over axiomatized carriers such as
  `Real` normalize symbolically under the carrier's declared algebra. The
  operand type picks the mode.
- A pure, total, measured machine over ordinary machine types is **dual-use**:
  it runs at runtime *and* serves as a fact atom the engine reasons about.
  Most theorems about `u64` code cite dual-use machines directly and never
  need `Nat` at all; `Nat` appears when a claim is genuinely about unbounded
  mathematics.

Core ships the roster: `Nat`, `Seq<T>`, `Bag<T>`, and `Rat`. Every finite
nonzero float embeds into signed `Rat` exactly (binary values are dyadic
rationals), while signed zero, infinity, and NaN inhabit the separate
proof-level `FloatMeaning` cases. Float verification invokes executable
`FloatSemantics` functions whose finite branches are exact Rat arithmetic plus
one format rounding step. Its `FiniteNonZero` payload is `Rat in NonZero`, so
the proof carrier has no overlapping zero representation. `Int` follows when
subtraction-closed reasoning wants it, with one rule stated at introduction:
`Int`'s order has no floor, so ranking views over it must produce a
well-founded `Nat` rank or carry a proven floor.

Core's `Rat` stores a signed `IntPair` numerator and a positive `Nat`
denominator; `mk_signed_rat` cancels the pair's shared offset and reduces the
remaining magnitude with the denominator. Its Cauchy-facing metric still
avoids division. `rat_gap(p, q)` is the nonnegative absolute cross-product
numerator gap, and
`rat_close(p, q, precision) == Nat::Zero` states
`|p-q| <= 1/precision` by comparing `precision * gap` with the common
denominator in Nat's monus order. Its reflexive and symmetric laws are ordinary
checked machines; they are the metric substrate for the constructed `Real`
corpus, not compiler-known arithmetic.

The supporting natural metric is ordinary core code as well. `nat_gap(a, b)`
computes symmetric absolute difference from the two monus directions, and
`nat_gap_triangle(a, b, c)` proves
`nat_gap(a, c) <= nat_gap(a, b) + nat_gap(b, c)` in the settled
`sub(left, right) == Nat::Zero` order spelling. Its proof uses nested structural
case states; every value leaf is checked, and recursion remains admissible only
when strict-subterm provenance survives every state-parameter forwarding edge.
Proof citations are statement-ordered: an earlier checked citation can
establish a later citation's `requires`, but a later statement can never justify
an earlier call. No Nat metric law is built into the checker.

Rational triangle is likewise division-free. `rat_gap_triangle_scaled(p,q,r)`
lifts all three gaps to the shared denominator and proves
`q.den * gap(p,r) <= r.den * gap(p,q) + p.den * gap(q,r)`. It is an ordinary
composition of Nat gap homogeneity, commutative-semiring factor rearrangement,
and `nat_gap_triangle`. Citing it substitutes symbolic member places into the
consumer's frame (`p.den` becomes the actual argument's `.den`); the names in a
theorem declaration are never observable at a citation site.

The order layer used above is checked core code as well.
`mul_le_mul_right(a,b,k)` transports `a <= b` through a common multiplier;
`mul_le_cancel_right(a,b,k)` reflects the order when `k` is positive. The
first proof is requires-bearing induction: its induction hypothesis is visible
to an authored per-arm citation only when every premise instantiated at the
smaller self-call is already established at that statement boundary. Earlier
citations may make the conditional hypothesis available to later citations;
an unproved or membership-shaped premise contributes no hypothesis.

`rat_close_triangle_split(p,q,r,e)` is the reciprocal-precision triangle:
closeness of `p,q` and `q,r` at `e+e`, plus positivity of `q.den`, proves
closeness of `p,r` at `e`. It scales the denominator-shared gap triangle,
combines both premise bounds, cancels `q.den`, and then cancels the concrete
factor two. No division or hidden ordered-ring tactic enters the proof.

The first sequence-facing atoms are ordinary generic machines too.
`cauchy_at<Sequence, Modulus>(precision, i, j) == Nat::Zero` states the
same-generator point obligation after `i` and `j` have reached the static
modulus. `converges_together_at<Left, Right, Modulus>` states its
heterogeneous two-generator twin. Their arbitrary precision and index inputs
are the universal variables; positive precision and both modulus bounds are
ordinary `requires`. There is no hidden quantifier, runtime callable, or
compiler-known notion of convergence in this surface. Their same-generator
reflexivity and heterogeneous symmetry facts are checked generic theorem
machines and remain citable at concrete generator/modulus selections.

`converges_together_at_triangle_split<Left,Middle,Right,Modulus>` lifts the
doubled-precision Rat theorem to one shared middle index. Both precision levels,
all modulus thresholds used by the premises and conclusion, and the actual
`Middle(index).den` positivity fact remain explicit requirements. Static-machine
application member places preserve the selected generator during citation
substitution; a positivity fact about another generator does not alias it.

The pointwise corpus supplies the mathematical kernel for the quotient below.
The remaining language layer packages an existential modulus plus its universal
law as carrierless proof evidence. A convergence proposition opens that
evidence to one stable opaque modulus symbol characterized by its law; it does
not run a convergence decider or expose the selected conformance in runtime
layout. The same evidence term opens to the same symbol, while distinct
evidence terms may carry distinct witnesses without changing proposition or
quotient identity.

The ordered implementation dependency is explicit: proof-side proposition
families and typed index telescopes land before evidence-bearing quotient
formation. See
[Law-Bearing Relations, Evidence, And Quotients](../design_briefs/law_bearing_relations_and_quotients.md).

A quotient coarsens a type: sort its values into buckets of things a
proven equivalence calls interchangeable, and the buckets become the
values — read `%` as it already reads everywhere else, modulo. Wrapping
arithmetic is the familiar instance: `u32` addition is integer addition
with numbers differing by 2^32 counted the same.

```omega
data Real = CauchySeq % ConvergesTogether;
```

This is the bodyless `data` declaration (the `const X = ...;` shape): the
right side is a type expression, and `%` is its one new form. `CauchySeq` is a
proof carrier family whose typed index telescope contains its generator
machine. `ConvergesTogether(a, b)` is a proposition over representative
values. Its representatives may be `CauchySeq<A>` and `CauchySeq<B>` with
different generator indices while sharing the same family identity. Rat is the
same model with an empty index telescope. Quotient carrier matching never
admits an instance of a different family.

The proposition's evidence is a selected conformance projected entirely into
the proof stratum:

```text
ConvergenceEvidence<A, B>
|- modulus(precision: Nat) -> Nat       opaque proof symbol
`- close_after(...)                    checked universal law
```

The mathematical name `ConvergesTogether(a, b)` is a transparent proposition
alias; ordinary signatures do not expose the underlying carrierless `dyn`.
Because the entire dynamic value has no runtime carrier, this exact by-value
owned-`dyn` case needs no storage owner, table, allocation, or cleanup. Merely
having no runtime table slots would not suffice for an ordinary runtime
instance.

Relation properties are ordinary explicit conformances. `Reflexive`,
`Symmetric`, and `Transitive` are independent requirements;
`Equivalence<C, R>` composes all three and redeclares none. Preorders and
partial orders reuse the same component properties. The compiler does not
discover free proof machines by `_reflexive`, `_symmetric`, or `_transitive`
suffix. The current N6 canaries exercise that implemented legacy convention
and are migration inputs, not the final law.

`%` consumes the carrier family, proposition relation, and a selected
`Equivalence` conformance. A unique home satisfier is inferred; ambiguity uses
the ordinary named-conformance selection. Quotient formation remains
carrier-only (`seq as Real`; `42 as Real` does not compile — that road runs
through `Rat` and a constant stream). Proven `ConvergesTogether(a, b)` makes
`(a as Real) == (b as Real)` a fact. Equality on the quotient means "same
bucket," never "same representative".

Equivalence licenses the quotient type, not operations on it. A machine lifts
only through a selected `Respects` conformance. Parameters, including an
attached receiver, normalize into one argument record. Given an argument
relation `RA`, result relation `RR`, and semantic precondition `P`,
`Respects` proves both:

```text
RA(x, y) -> (P(x) <-> P(y))
RA(x, y) && P(x) -> RR(f(x), f(y))
```

The first clause makes partiality representative-independent; it is trivial
for a total machine. Fixed ambient facts, authority, and resource requirements
do not vary by representative and remain ordinary contract obligations.
Binary addition uses a fieldwise relation over both operands. Division must
additionally prove that equivalent denominators agree about being zero.
Comparison uses equality as its result relation. Normalizing arguments as one
record avoids separate respect traits for every arity.

An attached carrier operation used this way has a by-value receiver and is
proof-side only: it does not install a method or reify a representative on the
quotient. A borrowed or mutable receiver is still a forbidden runtime use of
proof-only data. Operations attached to runtime data remain runtime operations
and cannot accept proof-only values.

A boundary axiom may be cited as an environmental assumption elsewhere, but
cannot admit either an equivalence or respect conformance for a checked
quotient. Both require checked proof machines. A false quotient equality
propagates by substitution without the containment boundary available to an
admitted resource claim.

## Proof Views

Runtime data often needs a mathematical view before it can be reasoned about.

For slices, useful proof views include:

```text
Seq(items)    ordered finite sequence view
Bag(items)    finite multiset/counting view
Range(len)    finite index space
```

These are ordinary proof-only types from core — recursive data plus
extraction lemmas, not compiler-known forms. They do not allocate at runtime;
they let contracts talk about math without pretending that proof binders are
runtime loops.

`Sorted` is an ordinary domain defined by a predicate machine (see Quantified
Facts below); the views exist so contracts can talk about order and counting
without inventing runtime loops. Sorting is naturally expressed as:

```omega
machine Sort::bubble_sort_preserving(
    before: &[Nat],
    items: &mut [Nat]
)
requires
    Bag(items) == Bag(before)
ensures
    Seq(items) in Sorted
    Bag(items) == Bag(before)
{
}
```

The `before` value is explicit. There is no implicit `old` keyword here. A
caller that wants to prove preservation can make or carry a snapshot itself.

## Helper Machines

Large proofs should be decomposed through helper machines with small contracts.

```omega
machine Sort::compare_swap(
    before: &[Nat],
    items: &mut [Nat],
    index: u64
)
requires
    index + 1 < items.len
    Bag(items) == Bag(before)
ensures
    items[index] <= items[index + 1]
    Bag(items) == Bag(before)
{
}
```

The preservation fact is explicit. If a caller needs a before-state, it passes
one in. Nothing in this chapter relies on an implicit snapshot keyword.

A sorting proof is built from smaller facts:

```text
compare/swap orders one adjacent pair
compare/swap preserves Bag(items)
one pass moves the largest remaining item to the end
repeated passes establish Seq(items) in Sorted
Bag(items) stays equal to the explicit before value
```

## Quantified Facts

> **Settled 2026-07-18: quantifiers are not keywords.** Universal claims over
> all values are machine parameters (a theorem over `(n: u64)` is checked
> symbolically once). Element-wise facts are element types and window facts
> (chapter 7). Relational facts over sequences are **predicate machines** plus
> one extraction lemma each. Existentials are witness-carrying out-params.
> `forall`/`exists` remain parse errors; the quantified shape lives in the
> engine, not the surface.

A relational property is defined by an ordinary measured machine:

```omega
machine sorted(items: &[i32]) -> bool
terminates by items -> Slice::Length;
{
    transition items.len <= 1 {
        true  -> true
        false -> items[0] <= items[1] && sorted(items[1..])
    }
}

domain [i32]::SortedAscending { sorted(self); }
```

The definition also specifies the decider: a checked validator runs it (or a
loop the checker proves refines it), and the successful path uses `as` only
after the predicate is established.

Consuming the fact at an arbitrary index needs one **extraction lemma** per
predicate — an induction, written once by the predicate's author:

```omega
machine sorted_extracts(items: &[i32], i: u64, j: u64)
requires sorted(items) == true && i < j && j < items.len
ensures items[i] <= items[j]
terminates by i -> Nat::IncreasingTo(j);
{ ... }
```

After that, the engine holds the quantified fact-shape natively and every use
is mechanical, under two closed rules:

- **Instantiation** happens only at index atoms in scope at the obligation —
  deterministic, budgeted, never searched. A missing instance is a normal
  "cannot prove" naming the index it needed.
- **The delta rule**: extending a quantified fact by one element (a
  validator's loop step, a table's append) costs one definitional unfold.
  Loop invariants over sequences ride state arrival contracts (chapter 11).

Instances injected by the lemma are ordinary atom-facts, so the
difference-bound engine composes them — transitivity, everything-left-of-mid,
min-at-ends are downstream chains, not further lemmas.

## Induction Is Ranked Recursion

A proof-stratum machine recurses under the same rule as every machine: a
`terminates by` ranking, checked at every cycle (chapter 3). Read as a proof, the
machine *is* the induction: transition dispatch is the case analysis
(exhaustiveness enforced — no missed constructor), the measured cycle is the
appeal to the induction hypothesis, and a state's arrival contract (parameter
facts plus state `requires`, chapter 11) is the hypothesis itself, proven at
every in-edge. Nothing was added to the language to express induction; the
state machine was already its shape.

Induction may also be indexed by a finite unsigned count while its theorem is
about proof-only data. On an arm guarded by `n > 0` (or `n >= 1`), a recursive
argument `n - 1` is the checked predecessor. The structural checker treats that
argument as an opaque index, imports the recursive contract there, and can then
unfold or cite `Nat` lemmas around the recursive result. This is a bridge at the
recursive edge, not an implicit conversion between `u64` and structural `Nat`.

## Termination Proofs

Termination is a proof over every cycle in the reachable machine/state graph,
not an `ensures` proposition evaluated after a return and not a reach-row
member.

```omega
machine walk(items: &[Nat])
terminates by items -> Slice::Length;
{
}
```

The ranking argument is ordinary proof vocabulary:

- choose explicit subjects;
- select a well-founded ranking view; and
- prove every cyclic edge makes the produced rank strictly smaller.

Direction belongs to the view rather than a blessed `decreases` or `increases`
keyword. `Nat::Descending`, `Nat::IncreasingTo(limit)`,
`Tree::ProperSubtree`, and lexicographic views all satisfy the same checker
role. A standalone `measure` declaration supplies a named custom view and
multiple measures per carrier are legal.

Proof-stratum machines use exactly the same `terminates by` source and checking
rule as runtime machines. Their eligibility differs only at lowering: measured
non-tail recursion is legal when evaluation remains in the proof/compile-time
stratum and is rejected if runtime lowering is requested.

The normalized artifact separates the public termination guarantee from the
private ranking witness. A witness change invalidates its provider proof cache,
not caller or external requirement-binding identity. See chapter 9 and
[Termination, Ranking, And Progress](../design_briefs/termination_ranking_and_progress.md).

## Citing Proofs

A fact the engine cannot derive may be discharged by citing a proof machine's
contract, instantiated at the operands. This is the only connection between
proof-stratum theorems and runtime code, and it has no syntax of its own — a
cited theorem is a fact like any other:

```omega
machine Walker::step(&mut self)
requires self.n >= 1 && self.n <= 6148914691236517205
ensures self.n == collatz_step(n0)    // refinement: the u64 op IS the ideal op
stores self.n
{ ... }
```

Working rules:

- A theorem over parameters applies at any operands satisfying its
  `requires` — instantiation is machine application, not search.
- An `ensures` may equate a runtime place with a pure machine's result (a
  *refinement* fact): the runtime operation provably computes the
  mathematical function on the domain where its witnesses fit. Prove once
  over the ideal type; embed per width by supplying each width's bound.
- Runtime code that cites no proofs pays nothing and sees nothing.

Carrying a theorem to a site is an ordinary statement call — a fact-only
machine invoked for its `ensures`, which enters the flow facts and erases at
codegen:

```omega
mask_is_mod(self.head, self.cap);            // erased; its ensures now in scope
self.slots[self.head & (self.cap - 1)] = x;  // proves against those facts
```

This explicit form is the default (settled 2026-07-18): the proof structure
stays visible in the text. When an obligation fails for want of a known
lemma, the diagnostic names it by shape match. A rewrite extension —
proven equations joining the engine's term reading — is parked in the
design brief, to be revisited only if ergonomics demand it.

## Evidence And Trust

Facts are proven, computed, deferred, or accepted — and each tier is a
distinct compiler behavior, never a label:

- **Proven** (the engine, a derivation, a cited theorem): no declaration
  exists. Most facts live here invisibly.
- **Evaluated**: the compiler runs an ordinarily terminating machine in the
  hermetic target-semantic evaluator. Deterministic work is metered for live
  progress, warnings, and any root-selected ceiling; long or unlimited
  evaluation remains legal when root policy permits it. Results and canonical
  usage records are cached separately.
- **Deferred** ("prove later", written by tooling): a waiver of exactly one
  compiler-derived obligation — nothing new becomes citable. Warns on every
  build; fatal at **package release** (publishing with an open deferral is
  the hard error — "release" is a package-manager moment, not a build
  configuration; debt never crosses a package boundary). Hash-pinned to the
  code under it: edits kill the deferral and it must be re-taken.
- **Accepted**: a `boundary machine` — a contract with no body, the proof
  system's face of the boundary culture (chapter 19): trusted, audited,
  reported.

```omega
boundary machine collatz_cert_checked()
ensures check_collatz_cert(cert_blob_b41c) == true
```

Working rules:

- **The statement carries all specificity.** Trust the narrowest thing — an
  execution claim ("this checker accepts this certificate", the
  certificate's identity inside the statement) rather than the theorem it
  implies; a userspace proof machine lifts the narrow claim to the broad
  one. The trust report cannot be vaguer than the claim, because it *is*
  the claim.
- **There is no inline `assume`.** Boundary machines are the only home for
  unproven facts. Grant locality: **own-package boundary machines are active
  in dev builds**, carrying a standing warning until granted; boundary
  machines arriving **from packages are inert until granted** — a library's
  boundary machines surface as requests when the package is added, and a
  package can never self-grant.
- **Grants flow from the root.** The final build's build.omg accepts each
  request by symbol — `b.accept_boundary<walker_lib::collatz_cert_checked>();`
  (a compile-time machine parameter, chapter 13). The build lockfile — the
  same machine-written lockfile that pins package resolution; one receipt
  file, not two — records the statement hash automatically; a statement
  that drifts under a grant fails the build until re-approved. No hash is
  ever hand-written; build.omg stays the only file a human authors.
- **The engine can veto.** A boundary statement the engine can refute — one
  contradicting declared ranges, domains, or another accepted statement —
  is a compile error, grants notwithstanding.
- **Blast radius is reported.** The trust report names which conclusions
  rest on which boundary machines; facts derived without touching one stay
  in the unconditional tier, visibly. Export status is irrelevant — the
  report sees every grant, private or public.
- **The grant row is the language's `unsafe`.** A granted false statement
  can corrupt anything proofs protect — bounds, domains, and through
  corrupted memory, everything downstream. Reach restrictions cannot be waived by
  facts (they ride the call graph, and a boundary machine has no body),
  but a false range fact reaches the same place dynamically. Omega has no
  `unsafe` keyword because this is the one unsafe door: root-only, pinned,
  reported, tripwired.
- **Runtime-decidable boundary claims get oracle tripwires** in proof
  builds: a test run that witnesses a violation traps naming the machine
  that lied.

Certificates need no construct of their own: a certificate is wire data,
its checker is a measured machine, its soundness is a theorem
(`check(c) == true` implies the claim), and establishment is the
`evaluated` tier — or a proved `as` qualification through a certificate domain
(`domain [u8]::ValidCert { check(self); }`), the validated-decode pattern
of chapter 8 applied to proofs. A build that can afford the check *proves*
the claim outright; one that cannot accepts the narrow execution claim
above and lifts it by theorem.

Trust has a data face too. `boundary data` declares a type whose source
representation is externally admitted rather than structurally defined. It
does not mean “imported layout” or “exported layout,” and the keyword does not
encode traffic direction. A `boundary machine` is likewise classified by its
supply mode—checked body, trait requirement, selected provider, or accepted
declaration—rather than by an inbound/outbound reading of `boundary`.

The N5
`omega::language::core::real` package is an ordinary core declaration built
from this surface; its relevant contents are:

```omega
boundary data Real;                                    // opaque proof-only carrier
boundary machine Real::add(a: Real, b: Real) -> Real;  // no ensures: a symbol — claims nothing
boundary machine real_add_commutative(a: Real, b: Real)
ensures Real::add(a, b) == Real::add(b, a);            // an axiom: one trust row
```

The carrier is proof-only (nothing without a definition can have a layout);
its meaning is exactly its axiom machines; the package rides the same
grant/lockfile/report machinery. An ensures-less declaration claims nothing
and needs no grant; each axiom is one accepted-tier row. Axioms retire by
the standard upgrade: ship the constructed type with its proven theorems,
and consumers swap grant for import.

Core ships classical logic itself this way: excluded middle is a boundary
machine, granted like anything else — nothing is granted by default, not
even logic (project templates carry the line). A build that never grants it
is constructive, and its trust report says so.

## Automation And Boundary

The checker should automatically solve common cases:

- arithmetic normalization,
- equality reflexivity,
- range implications,
- branch facts,
- disjoint field facts,
- simple generic const facts.

When automation fails, library authors can provide helper machines. When a fact
cannot be proven from machine code, contracts, or boundary foundations, it must
cross an explicit boundary.
