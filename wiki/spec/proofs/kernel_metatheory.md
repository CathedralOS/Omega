# Kernel metatheory

This document records the metatheory of the implemented proof kernel under
the [selected unified native inductive profile](inductive_profile.md#native-inductive-declarations).
It is a justification at the level of the reference paper's section 4 — an
argument over the rules the checker actually implements — not a mechanized
proof and not an imported acceptance verdict. Each claim names its status:
*argued* (discharged by an argument here), *witnessed* (demonstrated by the
named test), or *by construction* (the implementation cannot do otherwise).
Code anchors are `source-rust/subsystems/compiler/psi/semantics/proofs/mathematical/`
paths.

<a id="the-implemented-calculus"></a>

## Calculus and checking judgments

The checking judgments are:

| Judgment | Implementation | Meaning |
| --- | --- | --- |
| `Σ ⊢ ok` | `signature.rs::check_signature` | declaration `i` checks under the prefix `0..i` |
| `Σ; Δ; Γ ⊢ t : T` | `typing.rs::infer_type` / `check_type` | typing under signature `Σ`, level arity `Δ`, context `Γ` |
| `Σ; Δ; Γ ⊢ s ≡ t : T` | `conversion.rs::convertible` | typed conversion at the shared type `T` |
| certificate `Σ; Δ; Γ ⊢ t : T` | `certificate.rs::verify_mathematical_certificate` | signature, then context formation, then `check_type` |
| assumption closure | `signature.rs::{assumption_closure, judgment_assumption_closure}` | exact transitive assumption set over stored statements and bodies |

Terms (`term.rs::Term`): `Variable`, `Sort` (`Type u` relevant /
`Strict u` irrelevant), `Pi`, `Lambda`, `Apply`, `Sigma`, `Pair`, `Fst`,
`Snd`, `Two`, `TwoZero`, `TwoOne`, `CaseTwo`, `Id`, `Refl`, `IdElim`,
the strict layer `Empty`, `EmptyElim`, `Squash`,
`SquashIntro`, `SquashElim`, `Box`, `BoxIntro`, `BoxElim`,
`Constant{declaration, levels}`, and the arena `Dummy`
(never well-typed). Levels are `Constant`, `Parameter(i)`, `Successor`,
`Maximum` — no `imax`, matching the predicative selection. Native inductive
type formers, constructors, and recursors are ordinary `Constant` references
whose `DeclarationKind` (`Inductive`, `Constructor`, `Recursor`) is validated
as a contiguous block by `inductive.rs::check_inductive_block`.

Reduction `t ⟶ t'` is `conversion.rs::weak_head_normalize`, one budgeted
step per rule, along head spines only:

- δ: `c(ls) ⟶ body[ls]` for a definition constant `c`; an assumption,
  inductive type former, or constructor constant is neutral.
- β: `(λ.b) a ⟶ b[a]`.
- π: `fst ⟨u,v⟩ ⟶ u`; `snd ⟨u,v⟩ ⟶ v`.
- ι-Two: `caseTwo(C,d0,d1,zero) ⟶ d0`; `… one ⟶ d1`.
- ι-Id: `J(C,d,y,refl A x) ⟶ d`.
- ι-Ind: `I_m.rec(ls) params motives minors indices (C_c(ls') params fields) ⟶
  minor_c fields ihs` in one step (`inductive.rs::reduce_recursor`), where each
  recursive field `f` of type `Π(x:D). I_j params idxs` contributes the
  induction hypothesis `λ(x:D). I_j.rec(ls) params motives minors idxs (f x)`
  (or `I_j.rec(ls) params motives minors idxs f` when `r = 0`).
- ι-unsq: `unsq P f (sq A a) ⟶ f a`; `sEmpty_rect` has no constructor
  to fire on.
- ι-unbox: `unbox P f (box A a) ⟶ f a`.

The public conversion entries (`convertible` and `convertible_retaining`)
first establish that `T` is a type, then check `s : T` and `t : T`. These
premises and the comparison use one context, one inference cache and the
caller's one `Budget`. A malformed shared type reports its own error before
operand checking; an ill-typed operand is a typed error, even when reduction,
reflexivity or strict irrelevance would otherwise hide it. The internal typing
continuations retain their established premises and schedule comparisons
directly, without recursively entering the public judgment.

As for `infer_type` and `check_type`, the context and signature must already
be well-formed; the certificate route establishes that separate premise as
described under context validity below. A public conversion call does not
admit an arbitrary unchecked context.

Conversion `s ≡ t : T` is then decided by: the shared type must
itself be a type; if its sort is `Strict`, both sides convert —
definitional proof irrelevance gated on the *shared type's* sort, never the
terms' shapes. Otherwise a reflexivity shortcut, weak-head normalization of
both sides, a second reflexivity probe, then head-matched typed recursion:
componentwise for `Pi`/`Sigma` (codomain under an extended context), for
`Lambda` at a `Pi` shared type, for `Pair` at a `Sigma` shared type, for
neutral spines (`Apply`, `Fst`, `Snd`, stuck
`CaseTwo`/`IdElim`/`EmptyElim`/`SquashElim`/`BoxElim`, and stuck native
recursor applications) at inferred types, and for `Id`/`Refl`/`Squash`/
`SquashIntro`/`Box`/`BoxIntro` at the shared type's own
structure. Pair eta runs in both directions at a `Sigma` shared type; the
profile's typed function eta runs in both directions at a `Pi` shared type.
Sorts convert through `levels_equal`; assumption, inductive, and constructor
constants convert only at the same declaration with semantically equal level
instantiations.

Checking (`typing.rs::check_type`) decomposes introduction forms against
the expected head constructor rather than comparing whole inferred types:
a `Pair` against a `Sigma` checks componentwise (the second component sees
the codomain instantiated by the first), and a `Lambda` against a `Pi`
checks its annotation against the expected domain by conversion and its
body against the codomain under the extended context. The `Lambda` case
is what lets a binder's body reach a dependent `Sigma` componentwise —
without it, inference alone records a non-dependent pair type that no
conversion can equate with the tagged family.

## Structural properties

**Weakening** — if `Σ; Δ; Γ ⊢ t : T` and `Γ'` inserts one well-formed
binding into `Γ`, then `Σ; Δ; Γ' ⊢ ↑t : ↑T` under the induced shift.
*Argued:* induction on the derivation; every rule reads variables through
`Context::lookup`, which shifts the stored binding by `index + 1`
(`typing.rs`), and the signature and level arity are binder-invariant.
`Context::extend` is the one-binder case. The `shift` operation
(`substitution.rs`) is exactly this arithmetic — binders increment the
cutoff, constants and level arguments are judgment scope and never move.

**Term substitution** — if `Σ; Δ; Γ, x:A, Γ' ⊢ s : S` and `Σ; Δ; Γ ⊢ a : A`
then `Σ; Δ; Γ, Γ'[x:=a] ⊢ s[x:=a] : S[x:=a]`. *Argued:* induction on the
derivation of `s`; the variable case is `lookup` plus weakening on `a`;
every other case is congruence because `substitute_at` distributes over
every former and shifts the argument by the binder depth at each use site —
capture is impossible by de Bruijn construction. This lemma is what
discharges the β case of preservation and the dependent-codomain
instantiation in application checking (`substitute(codomain, argument)`).
*Witnessed:* `substitution_shifts_free_variables_under_binders` and the
capture-rejection controls.

**Level substitution** — a judgment checked at level arity `n` holds at
every well-formed instantiation of its parameters. *Argued:* induction on
the derivation; level instantiation (`instantiate_levels`) rewrites only
`Sort` payloads and `Constant` level arguments, which no typing rule
discriminates beyond scope-checking. This is the soundness of the
declaration rule: a declaration checked parametrically is sound at every
instantiation, which is what makes a `Constant` reference's instantiated
statement correct. *Witnessed:* `indexed_levels`' family at `[0]` and `[1]`
through one checked declaration.

**Context validity** — every binding in a checked context is a type under
its prefix. *By construction* for the certificate route:
`verify_mathematical_certificate` runs `infer_sort` on each binding under
the prefix before extending; `check_signature` checks each declaration
closed under its own level arity, so a free `Variable` is
`UnboundVariable`, not a judgment.

## Preservation

If `Σ; Δ; Γ ⊢ t : T` and `t ⟶ t'`, then `Σ; Δ; Γ ⊢ t' : T`. *Argued* by
case on the reduction:

- δ: the definition's body checked at its statement under the signature
  prefix; level substitution preserves typing of the instantiated body.
- β: inversion of the application and lambda rules plus the substitution
  lemma.
- π: inversion of componentwise pair checking.
- ι-Two: the branch was checked at `C zero` (resp. `C one`), which is the
  eliminator's inferred type when the scrutinee is literally that
  constructor.
- ι-Id: typing `J` already decided `endpoint ≡ recorded` — the recorded
  endpoint of `refl A x` is `x` — so the base `d : C x (refl A x)`
  inhabits the result `C y (refl A x)` up to conversion. Preservation is
  modulo conversion, the standard shape for a type theory with typed
  equality.
- ι-Ind: `inductive.rs::check_inductive_block` verifies that every recursor
  declaration's stored type is literally equal (by conversion) to the
  kernel-computed dependent eliminator type for its inductive block, and that
  every constructor returns the target inductive at uniform parameters. When
  `I_m.rec(ls) params motives minors indices (C_c(ls') params fields)` fires,
  `reduce_recursor` instantiates `minor_c` at `fields` and the recursively
  formed induction hypotheses, landing at `motive_m indices (C_c(ls') params fields)`
  by the substitution lemma.
- ι-unsq: the eliminator's type is `P` outright, and `f a : P` by
  application — `f`'s domain was checked convertible to the squashed
  carrier, and the introduction's `a` checked at that same carrier.
- ι-unbox: the eliminator's type is `P x` where `x` is the scrutinee;
  for `x = box A a` the body `f` checked at `Π(a:A). P (box A a)` gives
  `f a : P (box A a)` — the result up to reflexivity.

*Witnessed:* the computation tests run each ι rule on a constructor with an
arbitrary neutral child function, and the step-ceiling refusal tests pin
that reduction is a typed error, never a hang.

## Normalization

Well-typed terms strongly normalize under `⟶`; weak-head normalization —
the only phase conversion runs — terminates on them. *Argued*, at the level
of the reference paper's section 4:

- The base two-layer calculus (sorts, Π, Σ) is the predicative sMLTT
  fragment the paper's reducibility argument covers. Strict irrelevance
  lives in conversion, not reduction.
- `sEmpty`/`sEmpty_rect`, `Squash`/`sq`/`unsq`, `Box`/`box`/`unbox`:
  the strict layer's redexes are the paper's eliminator∘constructor
  iota steps — `unsq P f (sq a) → f a` and `unbox P f (box a) → f a`
  fire only on the matching introduction, so each is a standard
  projection-shaped reduction on canonical scrutinees; `sEmpty_rect`
  never fires because `sEmpty` has no constructors. The strict unit is
  derived (`sUnit := Π(_ : sEmpty). sEmpty`), so it contributes no new
  former at all.
- `Two`/`caseTwo`: the eliminator fires only on the two constructors; the
  reducibility candidate for `Two` is its two constants.
- `Id`/`J`: elimination fires only on `refl`; the candidate for
  `Id A x y` interprets a proof as `refl` when `x ≡ y` or a neutral — the
  standard treatment of intensional identity without K.
- Native inductive types (`inductive.rs`): `I_m.rec` fires only on a matching
  constructor `C_c` of its inductive block and recurses only on strict
  subterms of `C_c` (recursive fields validated strictly positive by
  `check_inductive_block`), with universe predicativity enforced by
  `level_leq`. Strong normalization follows from the standard
  reducibility-candidate construction for strictly positive predicative
  inductive families (Lean 4 / Rocq).
- Typed function eta and strict collapse are *conversion* clauses, not
  reduction rules: they cannot create nontermination. Their cost is paid
  inside the conversion termination argument, below — this is the precise
  sense in which the combined rules' metatheory reduces to the base
  calculus's.
- δ-unfolding terminates independently of normalization: signatures are
  prefix-checked, so each step strictly lowers the greatest reachable
  declaration index and recursion is impossible.

*Justification:* argued at paper level, not mechanized — matching the reference
core's own section-4 status. The checker's soundness never relies on
normalization; normalization is what makes the conversion relation
decidable in principle.

## Conversion is a decision procedure

**Termination** — on inputs both checked at `T`, `convertible` terminates
(absent the ceiling). *Argued* by the lexicographic measure (structure of
`T`'s weak-head normal form, then the compared terms): the weak-head phase
terminates by normalization; strict collapse, sorts, nullary constructors
and mismatched heads answer immediately; `Pi`/`Sigma` recursion descends to
subterms of the shared type; lambda-lambda and both eta rules at
`Π(x:A).B`/`Σ` recurse at `B` or a component — the *type* shrinks, which is
what makes function eta terminating even though `f x` is larger than `f`
(typedness is load-bearing: only the shared type authorizes the descent,
and a strict `Pi` is already collapsed); neutral-spine cases recurse on
strict subterms at inferred types. The `infer_type` calls inside spine
comparison descend the same subterms.

**Soundness** — under the well-formed signature and context premises,
`convertible` answering `true` includes both operand typing judgments and
implies `s ≡ t : T` in the declarative judgment. *Argued:* every clause either
is a declarative rule —
strict irrelevance, congruence, pair eta, typed function eta, reflexivity —
or is justified by reduction being a fragment of declarative conversion
plus transitivity. Refusals are the complementary half of correctness:
mismatched canonical heads answer `false`, which is a correctness property
of the decision, not an equality claim.

**Completeness** — on well-typed inputs, a declaratively convertible pair
is accepted. *Argued:* by orthogonality the reduction system is confluent —
every rule's left side is a distinct head-constructor pattern (`Apply`∘
`Lambda`, projection∘`Pair`, eliminator∘constructor, a definition constant)
with no critical pairs, so weak-head normalization computes a canonical
representative. The clause set is then exhaustive over normal-form pairs at
a shared type: distinct canonical heads are never declaratively convertible
(former injectivity follows from confluence plus the absence of any rule
equating formers), a neutral never converts to a canonical form, and
neutrals reduce to spine equality at inferred types, which the componentwise
clauses decide. Eta is consulted at exactly the `Pi`/`Sigma` shared types
where the declarative eta rules live.

**The ceiling is a resource bound, not a decision shortcut.** `Budget`
makes the procedure total on arbitrary input: exhaustion is
`CoreError::StepCeiling`, a typed error — never `Ok(false)` and never a
hang. The public entry charges operand formation and conversion to that same
ceiling; checking an operand never refreshes it. The honest statement is
therefore: under a well-formed signature and context, the algorithm is *sound
and bounded-incomplete* — a true conversion whose witness
exceeds the ceiling is refused, never wrongly decided. `DEFAULT_CONVERSION_STEPS`
is a policy default, not part of the calculus. *Witnessed:*
`conversion_refuses_at_the_step_ceiling` and the computation tests'
ceiling controls.

Stack depth is likewise a resource, handled outside the calculus: the
checkers and the bounded denotation recurse over term and proof-tree
structure, so `certificate.rs::run_on_verification_stack` runs the public
verification and denotation entries on a dedicated 256 MiB worker stack
(scoped thread, panics re-thrown on the caller). Depth therefore exhausts
the elaboration bound or the reservation as a typed error, never as a
process abort; the recursion itself is unchanged, so the metatheory above
still describes exactly what runs.

**Level equality is decidable** — `levels_equal` compares `max`-normal
forms (constant floor plus per-parameter successor offsets, with
constant-floor absorption); normal forms are equal exactly when the induced
functions over the naturals agree, and the algebra (`max` associative,
commutative, idempotent; `succ` distributes; the absorption law) is the
free algebra for this signature, so the decision is complete — no level
solving, and distinct parameters never convert. *By construction* plus
*witnessed*: `the_level_algebra_decides_sort_conversion`.

## Declarations and exact assumption closure

`check_signature`'s prefix discipline is the metatheoretic content of the
declaration model: a `Constant` can only name a strictly earlier
declaration, so self- and forward references reject as `UnknownDeclaration`,
no recursive definition exists, and δ-unfolding's decreasing-index measure
is what makes δ terminating. *Witnessed:*
`check_signature_rejects_self_and_forward_references`,
`a_second_declaration_references_only_the_checked_prefix`,
`a_definition_constant_unfolds_and_an_assumption_stays_neutral`.

The closure is *exact* by construction: `collect_references` traverses the
stored statement and body syntax of every reachable declaration and the
worklist returns the transitively reachable assumption indices — the
[foundation](foundation.md)'s "complete reachable checked declaration
graph". It is computed over stored terms, never over post-conversion
forms, so a theorem whose
statement references an axiom-dependent definition retains the assumption
even when the proof body never unfolds it. *Witnessed:*
`assumption_closure_reaches_through_statements_and_bodies`,
`assumption_statements_carry_the_closure_too`, and the wire-level
`an_axiom_dependent_theorem_keeps_its_assumption_through_the_wire`.
Receiver acceptance of the closure is policy, outside the kernel.

## Native inductive declarations and scheme correspondence

Under the [unified native inductive profile](inductive_profile.md#native-inductive-declarations),
plain recursive data, algebraic data types, value-indexed families, and mutually
recursive blocks are declared directly as contiguous `DeclarationKind` blocks
(`[I_0..I_{M-1}, C_0..C_{K-1}, I_0.rec..I_{M-1}.rec]`) validated by
`inductive.rs::check_inductive_block`:

- Formation: each `I_m` is checked in the signature prefix to end in a relevant
  `Sort::Type(u_m)`, with uniform parameter prefix shared across the block.
- Constructors: each `C_c` is checked with the block's inductive type formers
  in scope, enforcing universe predicativity (`level_leq` against `u_m`),
  strict positivity of every recursive occurrence, and return type targeting
  `I_m params indices`.
- Recursor: each `I_m.rec` is checked against the exact kernel-computed
  dependent eliminator type (`build_recursor_type`), with motive landing in
  `Sort::Type(w)` (relevant sort only; strict targets route through `Box`).
- Computation: `I_m.rec(ls) params motives minors indices (C_c(ls') params
  fields) ⟶ minor_c fields ihs` is *definitional* in one step via
  `reduce_recursor`, including with neutral arguments and ω-branching recursive
  fields. *Witnessed*: `inductive_profile` tests in `mathematical-kernel` and
  `indexed_vector`, `indexed_derivation`, `indexed_levels`, `indexed_mutual`,
  `indexed_nested`, and `indexed_relations` in `proof-admission`.

The [quotient](quotients.md#set-quotient-foundation) scheme keeps its
admitted/derived boundary explicit: the seven
interface assumptions (`Q`, `project`, `setQ`, `sound`, `effective`,
`elim`, `beta`) are named assumptions with exact statements; `transport`,
`idTrans`, `transportConst` and `lift` are checked definitions derived from
them. `beta` is a relevant `Id`, never a conversion rule, so a quotient's
representative stays unextractable. No other quotient declaration is
admitted: a lift whose result also depends on a premise about the class
would need a function-valued motive, and so the function extensionality the
calculus deliberately lacks, so the kernel offers no such lift — the
boundary between admitted law and derivable consequence is stated per
declaration. *Witnessed:* the
`quotient_*` scheme tests, including `quotient_beta_is_propositional_evidence_not_conversion`
and `quotient_assumption_closure_records_the_admitted_interface`.

The bounded-certificate denotation is an untrusted elaborator, not a
trusted translation: `denote_bounded_certificate`'s output is a
`MathematicalCertificate` that `verify_mathematical_certificate` re-decides
in full, so the soundness obligation reduces to the kernel's own judgment.
Meaning preservation of the denotation table (atoms as `Type 0`
assumptions, scalar `Equal` as `Id` over its denoted carrier, connectives
as `Σ`/tagged sums/`Π`, a decided non-reflexive primitive as a named
decision assumption) is argued per connective in
`mathematical_core/bounded_denotation.rs`'s module documentation. Every
certificate rule family denotes. Propositional constructions and identity
symmetry/transitivity, including `ContentConservation`, elaborate to kernel
terms. Liftable scalar integers and `IntegerMath*` share `Int` and its
identity type, so the supported `Equal`↔`IntegerMathEqual` citation crossing
does not assume a conversion. Integer order rules apply one fixed roster
of explicit laws. Value-equation transport uses identity elimination in exact
replacement contexts, including multiple equations and connective binders.
Both statements expand through the same supplied definitions; the premise
transports forward and the goal backward. Reversal reuses the original
occurrence context, so equal-looking terms elsewhere do not change. The separate
integer-order substitution rule still applies fixed endpoint-substitution laws.

Boolean values denote `Two`, with false/true as zero/one. Negation and
Boolean-valued equality use the existing `caseTwo` truth tables, keeping their
operands visible without new axioms or kernel rules. Boolean equality applies
one fixed closed lambda, storing each supplied operand once rather than
duplicating nested operands into both case branches. Proposition equality
remains `Id Two`, not that Boolean comparison. Unknown Boolean values remain
distinct constants; a Boolean-valued operation without a checked denotation
must remain opaque. If bounded normalization reduces a reflexive
result equality to `Truth`, `refl` proves the expanded result before the
stored value equations transport it back. This does not prove every Boolean
identity on neutral values.

Fixed scalar literal magnitudes have shared signed binary definitions over
`zero`, `double`, `odd` and `negate`. Discreteness derives adjacent-literal
order from five fixed arithmetic laws, then uses mixed transitivity with
the inclusive premise. Its strict-to-nonstrict form and its open
`a < b ⊢ a + 1 <= b` form are not derived; they denote rule-instance
decisions, as do the four provisional float rules (`IeeeFloatOrderChain`,
`FloatMeaningNormalization`, `FloatIntegerConversion`, `IntegerFloatExact`)
and `IntegerPowerOfTwoOrder`, which have no IEEE model or power of two in
the core to derive from. These laws remain assumptions with exact statements;
this establishes neither their consistency nor receiver approval. Larger
closed values retain opaque exact-value interning. Exact scalar subtraction
has a compositional denotation over the shared integer carrier, including
overflowing and nested expressions. Subtraction order applies fixed
subtraction-by-zero and right-antitonicity laws, then transports the endpoints.
Representable closed differences keep canonical numeral identity and derive
their order through binary comparison. Contradictory positivity premises
use an explicit strict-irreflexivity law and checked empty elimination.
The correlated unsigned subtraction witness derives its zero lower bound
from fixed self-zero and non-strict right-antitonicity laws. Open mathematical
subtraction shares the scalar operation when its children can be denoted;
if evaluating a previously skipped child would introduce a resource refusal,
the already-admitted open expression retains its prior opaque identity.
Whole closed-term resource refusals are unchanged. Open addition likewise
retains its operand applications. Correlated addition lower and upper bounds use
fixed addition monotonicity and subtraction cancellation, followed by
checked equality transport for the subtraction result and exact carrier endpoint.
Lower bounds transport the left endpoint of the order; upper bounds transport
the right. Both reuse the same fixed laws.
The retained equality evidence is oriented by its denoted identity, because
normalizing a source equality can reverse its endpoints. An open sum whose
correlated difference has collapsed to its numeral `n` cannot match the
applicative `add (sub e r) r` cancellation, so the chain substitutes the
checked numeral-operation equation `add n r = e` — an exact interned
assumption, once per evaluated operand triple — in its place.
Correlated subtraction lower and upper bounds reuse the same machinery
with two fixed `add`/`sub` adjunctions — `add a c ≤ b → a ≤ sub b c` and
`b ≤ add a c → sub b c ≤ a`: after a value root substitutes through its
cited definition, a closed sum's numeral `n` substitutes the checked
equation `add e r = n` in reverse to reach the applicative premise the
adjunction expects, and a landed endpoint substitutes through its cited
literal equality as in the addition chain. No
arithmetic law becomes definitional conversion. Other open operations
remain opaque. These fixed laws are assumptions, not an arithmetic consistency
result. Recursive evaluator preflights may revisit prefixes;
shallow term storage does not establish linear checking cost.

The exact-add definition bound cites a semantic `out = l + r` equality
beside the two operand bounds. A fixed two-sided monotonicity law
`a ≤ b → c ≤ d → add a c ≤ add b d` combines the operand evidence into a
bound on the applicative sum, the cited definition transports that bound
onto `out`, and a checked numeral-operation equation `add lb rb = k`
lands the conclusion's literal endpoint. Each operand's endpoint re-shapes
its evidence to the required direction: an oriented `≤` stands, an
operand-to-literal `Equal` transports through `eq_le` in either citation
orientation, and a literal addend is its own endpoint through `refl`.
A `Truth` bound over an open addend contributes its carrier endpoint
through an interned membership assumption `IntLe min' op'` or
`IntLe op' max'` — the exact fixed-carrier fact, interned once per
operand and direction; quantifying it as a law would range over every
`Int`, which membership does not survive. A bound literal the carrier
cannot represent keeps the explicit instance fallback below. The direct
`IntegerAffineBound` add form skips the cited definition: its premise is
the conjunction of the two operand bounds, whose denoted `Σ` pair
projects each conjunct onto the same endpoint machinery, and its
conclusion names the `add` application itself, so two-sided monotonicity
and the checked `add lb rb = k` equation land the bound on the
application directly. An endpoint sum outside the representable numeral
range keeps the instance fallback. The direct subtract form shares the
machinery with subtraction's antitone twist: the right operand's
endpoint re-shapes to the direction opposite the conclusion's, a fixed
`a ≤ b → d ≤ c → sub a c ≤ sub b d` law combines the operands, and the
checked `sub lb rb = k` numeral equation lands the literal — including
the `(Exact, Carrier)` boundary cases the checker admits and negative
bounds over unsigned carriers.

Remaining families, including other bound and correlated-root
witnesses, transport through opaque operations, reversed identities nested
inside connectives and further Boolean normalization, denote
a *rule-instance decision*: an assumption constant of type
`Π(_ : ⟦premise₁⟧). … . ⟦conclusion⟧` whose premise/conclusion relation
is re-decided during denotation by the same shared function the bounded
checker runs, applied to the denoted premise evidence. The judgment's
assumption closure then names the instance's arithmetic or conversion
content exactly. Source invalidity, unsupported valid encodings and producer
defects remain separate outcomes. Exhausting the optional transport producer's
construction allowance declines to that explicit assumption-bearing route;
it does not report an unchecked derivation or reject previously supported
bounded evidence solely for the extra construction cost.
*Witnessed:* the bounded-denotation unit
tests inspect exact law closure and identity compositions;
`compiler/tests/kernel_discreteness.rs` independently checks a source-produced
certificate, its mathematical wire roundtrip and its invalid control.
`compiler/tests/kernel_subtract_order.rs` checks the complete source-produced
ranked-loop edge and its subtraction operation against independently
reconstructed obligations, exact assumption closure through the mathematical
wire, and rejection of a non-decreasing rank. A guarded decrement-two operation
checks the same lower-bound rule and rejects an insufficient guard.
`compiler/tests/kernel_add_bound.rs` uses the unchanged guarded-addition source
customers, independently reconstructs their operation questions and checks the
correlated lower- and upper-bound certificates and mathematical wire. A changed
addend, reversed guard, missing premises or malformed kernel evidence rejects.
`compiler/tests/kernel_equality_transport.rs` compiles a nested Boolean result
contract and drops its source before reconstructing its obligation and operation
equations from Terminal bytes. Its mathematical proof uses identity elimination
instead of a conclusion-specific assumption and survives canonical wire replay.
Removing any required equation, changing an executable operand or the decoded
J endpoint, and negating the source result all reject.
Each receipt records the exact closure, term storage, normalization budget and wire
size; these are measurements of this certificate, not a general speed claim.
The `terminal-codec` bounded-certificate tests exercise retained declarations,
exact shared-integer closure, universally quantified order laws and
fixed-to-mathematical equality citations. Changed endpoints, forged evidence,
mismatched conclusions and malformed wire data reject.

### Assumed laws of the integer model

The integer model's laws are assumptions with exact statements: believed,
not derived. A proof that uses one lists it in its assumption closure, and
`source_safety_roster` in `bounded_denotation.rs` returns the whole set the
integer model declared. The bounded checker's certificates and the source
integer search, which proves goals about data indexed by values, share
them. The [rule for admitting an arithmetic rule](foundation.md#admitting-an-arithmetic-rule)
governs adding one. Every variable ranges over `Int`. In the division rows
`/` is the exact truncating quotient of the named fixed carrier, and each
such law is declared once per carrier.

| Operation | Law (code name) | Statement |
| --- | --- | --- |
| Order | `EqualityToLessOrEqual`, `LessThanToLessOrEqual` | `x = y → x ≤ y`; `x < y → x ≤ y` |
| Order | the four transitivity laws | `x ≤ y → y ≤ z → x ≤ z`, and the same with `<` in either or both premises (a strict premise gives a strict conclusion) |
| Order | the four substitution laws | an equality replaces either side of a `<` or `≤` |
| Order | `LessOrEqualAntisymmetry` | `a ≤ b → b ≤ a → a = b` |
| Order | `LessThanIrreflexive` | `x < x → ⊥` |
| Numerals | `DoubleZero`, `NegateZero` | `double(0) = 0`; `negate(0) = 0` |
| Numerals | `EvenBeforeOdd`, `OddBeforeNextEven`, `NegateOrder` | `double(x) < odd(x)`; `x < y → odd(x) < double(y)`; `x < y → negate(y) < negate(x)` |
| Addition | `Monotone`, `MonotoneBoth` | `a ≤ b → a + c ≤ b + c`; `a ≤ b → c ≤ d → a + c ≤ b + d` |
| Addition | `CancelSubtract` | `(b − c) + c = b` |
| Addition | `ZeroRight`, `ZeroLeft` | `a + 0 = a`; `0 + a = a` |
| Subtraction | `Zero`, `SelfZero` | `x − 0 = x`; `x − x = 0` |
| Subtraction | `Antitone`, `LessOrEqualAntitone` | `a < b → x − b < x − a`; `a ≤ b → x − b ≤ x − a` |
| Subtraction | `MonotoneAntitone` | `a ≤ b → d ≤ c → a − c ≤ b − d` |
| Subtraction | `CancelAddLeft`, `CancelAddRight` | `a + c ≤ b → a ≤ b − c`; `b ≤ a + c → b − c ≤ a` |
| Subtraction | `AddRightInverse`, `SubtractRightInverse` | `a + k = m → a = m − k`; `a − k = m → a = m + k` |
| Multiplication | `DistributeAdd` | `s · (a + b) = s · a + s · b` |
| Multiplication | `MonotonePositive`, `AntitoneNegative` | `a ≤ b → 1 ≤ r → a · r ≤ b · r`; `a ≤ b → r ≤ −1 → b · r ≤ a · r` |
| Division | `DivideUpperPositive`, `DivideLowerPositive` | `0 ≤ n → 1 ≤ d → (n / d) · d ≤ n`; `n ≤ 0 → 1 ≤ d → n ≤ (n / d) · d` |
| Division | `DivideUpperNegative`, `DivideLowerNegative` | `0 ≤ n → d ≤ −1 → (n / d) · d ≤ n`; `n ≤ 0 → d ≤ −1 → n ≤ (n / d) · d` |
| Division | `QuotientUpperPositive`, `QuotientLowerPositive` | `1 ≤ k → a · k ≤ m → a ≤ m / k`; `1 ≤ k → m ≤ a · k → m / k ≤ a` |
| Division | `QuotientUpperNegative`, `QuotientLowerNegative` | `k ≤ −1 → m ≤ a · k → a ≤ m / k`; `k ≤ −1 → a · k ≤ m → m / k ≤ a` |
| Division | `DivisorCaseSplit` | every `d` satisfies one of `d ≤ −2`, `d = −1`, `d = 0`, `1 ≤ d` |
| Casts | `CastIdentity` | `cast(x) = x`, declared once per widening or exact cast between two named fixed carriers |

Three of these were added for data indexed by values: `DistributeAdd` in
pull request #5554, and `ZeroRight` and `ZeroLeft` in pull request #5587.
Each names the program that needs it (`vector_bound`'s induction step
rewrites `10 * (previous + 1)` to `10 * previous + 10`, and a tree's rank
check bounds `left + right + 1` below by `left + 1`). They landed with the
change that needed them, not one per pull request as the admission rule
asks.

Every law in this table is tested against computed arithmetic by
`bounded_denotation/law_arithmetic.rs`. The test declares each law through
the same builder the proofs use and evaluates the declared kernel statement,
not the formula text, in a model that gives every integer declaration its
computed meaning: the integers with their own order, `+`, `−` and `·`,
`double(x) = 2x`, `odd(x) = 2x + 1`, `negate(x) = −x`, the truncating
quotient for `/` and the identity for a cast. Every law holds at every
assignment of its variables in −8..=8. Each division law also holds at every
value of the 4-bit carriers, and the two-variable ones at every value of the
8-bit carriers, with the quotient computed by the carrier's own exact
division; each cast law holds at every value of its source carrier, with the
cast computed by the carrier. A `CoreLaw` variant the test does not list
does not compile.

Each law also has a wrong twin the model must refute. For a law with
premises, each premise in turn is replaced by one that always holds; the
weakened statement then fails, or reaches a division by zero, at some
assignment, so every stated side condition is needed. For a law without
premises, a near miss of the same shape (for example `a + 1 = a` beside
`ZeroRight`, or `DivisorCaseSplit` without its `d = 0` case) fails. This
evidence shows that one ordinary model, the integers, satisfies every law at
every tested value. It is not a derivation: each row stays provisional until
the core checks a proof of it.

What the kernel does *not* discharge: faithfulness of the encoding to a
*source* declaration — positivity, nominal identity and statement fidelity
are elaboration-side obligations owned by `PROOF-CONTRACT-MIGRATION`. The
kernel's guarantee ends at the checked declaration graph.

## Reference rule correspondence

Each row compares one rule of the reference selection, Gilbert et al.,
POPL 2019, with the kernel rule that implements it. Rules come from Fig. 1,
p. 1:9 (MLTT) and sections 3.2–3.6, pp. 1:9–1:12 (the sMLTT additions).
*Matches* means every reference derivation of that rule is a kernel
derivation and conversely. *Restriction* means every kernel derivation is a
reference derivation, but not every reference derivation is available.
*Extension* means the kernel accepts derivations the printed rules do not;
an extension needs its own justification and is not part of the selection
merely because the kernel implements it. Kernel anchors are `typing.rs`,
`typing/judgments.rs` and `conversion.rs`.

| Reference rule | Kernel rule | Correspondence |
| --- | --- | --- |
| Empty context; context extension by a type; variable; weakening (Fig. 1) | de Bruijn `Context::extend` with a sort-checked binding; `Variable(i)` looks up and shifts its binding | Matches. Weakening is admissible through shifting rather than a primitive rule. |
| `Type_i : Type_{i+1}` (Fig. 1); `sProp_i : Type_{i+1}` (§3.2) | `Sort::Type(u)` and `Sort::Strict(u)` both infer `Type (u+1)` | Matches, with universe-polymorphic levels and no `imax`. |
| Π formation `Type_max(i,j)` with relevant codomain (Fig. 1, §3.2) | `Formation::Pi` with a `Type` codomain sort forms `Type (max)` for either domain sort | Matches. |
| Π formation into `sProp_max(i,j)` when the codomain is strict (§3.2) | `Formation::Pi` with a `Strict` codomain sort forms `Strict (max)` | Matches the predicative variant; the impredicative variant is not selected. |
| Abstraction and application (Fig. 1) | `Lambda` checks its body against the codomain; `Apply` substitutes the argument | Matches. |
| Relevant Σ formation over two relevant components (Fig. 1) | `Formation::Sigma` forms `Type (max)` | Matches. |
| Strict `Σs` over two strict components (§3.6) | `Formation::Sigma` with both component sorts `Strict` forms `Strict (max)` | Matches. The paper's note that projections and surjective pairing need no added rules holds here: `Fst`/`Snd` and pair eta are the relevant ones. |
| Σ with one strict and one relevant component | `Formation::Sigma` forms `Type (max)` (`dependent_pair_type_tracks_component_sorts`, `a_sigma_is_strict_only_when_both_components_are_strict`) | Extension: neither Fig. 1 (both `Type`) nor §3.6 (both `sProp`) prints this rule. |
| Pair introduction (Fig. 1) | `Pair` against `Sigma` checks the second component at the codomain instantiated by the first | Matches the intended rule. The printed premise `Γ, x:A ⊢ M : B` is a typo for `Γ ⊢ N : B{x := M}`. |
| First and second projection (Fig. 1) | `Fst`, `Snd` | Matches. |
| Conversion rule (Fig. 1) | `check_type` compares inferred and expected types with `convertible` | Matches. |
| β, projection computation, surjective pairing (Fig. 1) | β and π reductions; pair eta in both directions at a `Sigma` shared type | Matches. |
| Congruence for every constructor (§3.1 text: "congruence rules for every constructor"; Fig. 1 prints "(congruence rules omitted)") | Head-matched typed recursion for every former, introduction and stuck eliminator | Matches. |
| Definitional proof irrelevance at a strict type (§3.2) | `convertible` accepts any two terms whose shared type has a `Strict` sort | Matches; the shared type decides, never the terms' shapes. |
| `sEmpty : sProp_i` at every level, no premise (§3.3) | `Empty` infers `Strict 0` only (`empty_proposition_is_typed_only_at_strict_level_zero`) | Restriction: only the `i = 0` instance. Without cumulativity, `sEmpty` is not a `Strict 1` proposition. |
| `sEmpty_rect A e : A` into any `Univ_i` (§3.3) | `EmptyElim` forms a target of either sort and checks the scrutinee against `Empty` | Matches. |
| Derived `sUnit := sEmpty → sEmpty` (§3.3) | Derived, with no former | Matches. |
| Squash formation from `Type_i` into `sProp_i`; `sq` (§3.4) | `Squash` requires a `Type` domain and forms `Strict` at its level; `SquashIntro` | Matches the primitive rule. The predicative encoding's `j < i` side condition belongs to the encoding, not to this primitive. |
| `unsq P f x : P` with `P : sProp_j` (§3.4) | `SquashElim` requires a strict target and `f : A → P` | Matches; ι-unsq is the computation the irrelevance makes harmless. |
| Box formation from `sProp_i` into `Type_i`; `box` (§3.5; printed `Prop_i`, `sProp_i` meant) | `Box` requires a `Strict` domain and forms `Type` at its level; `BoxIntro` | Matches. |
| `unbox` with a motive into `Type_j` or `sProp_j`; `unbox P f (box a) ≡ f a` (§3.5) | `BoxElim` accepts a motive into either sort; ι-unbox | Matches. |

Beyond this selection the kernel adds the
implemented profile's typed function eta, relevant identity, `Two` rules
and native inductive blocks. These are extensions with
their own arguments above, not rows of the reference figure. Section 4's
consistency argument translates sMLTT into extensional type theory (Fig. 3,
Lemma 4.1, Theorem 4.2). It covers the matching and restricted rows. It does
not cover the mixed-sort Σ extension until that rule is selected and given its
own argument. Section 4.4's UIP extension remains excluded.

<a id="evidence-ledger"></a>

## Evidence requirements

Checked examples and rejection controls witness their particular judgments;
they do not establish consistency or the absence of arbitrary derivations.
Kernel-built encodings do not establish faithful source elaboration. Source
carrier, binder and capture identities, positivity, recursive termination and
exact assumption dependencies must be checked independently and preserved
through receiving and execution. The [foundation's acceptance examples](foundation.md#migration-acceptance)
require source-level witnesses as well as kernel checks.

## Trust boundary and explicit non-claims

Trusted code is the kernel proper: `term.rs`, `substitution.rs`,
`typing.rs`, `conversion.rs`, `signature.rs`, `certificate.rs` and
`certificate/batch.rs`. The batch importer, exact syntax interner and reuse of
successfully checked complete ordered signatures are trusted checking code.
Batch checking imports into one owned arena and rechecks every context and
judgment under one total budget; numeric handles never establish cross-arena
identity. The schemes, theorems and bounded denotation are *data* the kernel
re-decides; a defect in them surfaces as an ordinary rejection, not a false
judgment.

The reference core's strict layer comprises `sEmpty` with ex
falso into either sort, `Squash`/`sq`/`unsq` (existence without witness
extraction; the dependent eliminator is derived through irrelevance,
not primitive), `Box`/`box`/`unbox` (the converse embedding, whose
strict-motive non-dependent instance is the plain projection
`Box A → A`), and the derived strict unit `sUnit := Π(_ : sEmpty).
sEmpty`. Definitional proof irrelevance is governed by the shared
type's sort, so `Box`'s relevant payload is precisely where a neutral
proof stays distinct from a canonical `box`. `Squash` from
[mathematical bindings](mathematical_bindings.md) has this kernel
counterpart.

Not claimed: K/UIP, identity eta, `Two` eta, eta for inductive types, function
extensionality, equality reflection, cumulativity, `imax`, impredicativity.
Conversion decides definitional equality only; under a well-formed signature
and context it is sound and bounded-incomplete under the ceiling. Normalization
is argued at the
reference paper's section-4 level, not mechanized. Exact assumption closure
is computed over the stored signature; receiver acceptance of that closure
is policy outside the kernel.
