# Kernel metatheory and implementation evidence

This document carries the combined rule/encoding justification the
`PROOF-KERNEL-CORE` board item and the
[inductive profile](inductive_profile.md#typed-function-eta) require before a
verified profile can be claimed: substitution, preservation, normalization
and decidable conversion for the rules the reference checker implements, the
derived-encoding correspondence, and the implementation evidence each
obligation already has.

It is a justification at the level of the reference paper's section 4 — an
argument over the rules the checker actually implements — not a mechanized
proof and not an imported acceptance verdict. Each claim names its status:
*argued* (discharged by an argument here), *witnessed* (demonstrated by the
named test), or *by construction* (the implementation cannot do otherwise).
Code anchors are `omega-rust/psi/semantics/proof-admission/src/` paths.

## The implemented calculus

The judgments, as implemented, are:

| Judgment | Implementation | Meaning |
| --- | --- | --- |
| `Σ ⊢ ok` | `signature.rs::check_signature` | declaration `i` checks under the prefix `0..i` |
| `Σ; Δ; Γ ⊢ t : T` | `typing.rs::infer_type` / `check_type` | typing under signature `Σ`, level arity `Δ`, context `Γ` |
| `Σ; Δ; Γ ⊢ s ≡ t : T` | `conversion.rs::convertible` | typed conversion at the shared type `T` |
| certificate `Σ; Δ; Γ ⊢ t : T` | `certificate.rs::verify_mathematical_certificate` | signature, then context formation, then `check_type` |
| assumption closure | `signature.rs::{assumption_closure, judgment_assumption_closure}` | exact transitive assumption set over stored statements and bodies |

Terms (`term.rs::Term`): `Variable`, `Sort` (`Type u` relevant /
`Strict u` irrelevant), `Pi`, `Lambda`, `Apply`, `Sigma`, `Pair`, `Fst`,
`Snd`, `Two`, `TwoZero`, `TwoOne`, `CaseTwo`, `Id`, `Refl`, `IdElim`, `W`,
`Sup`, `IndW`, the strict layer `Empty`, `EmptyElim`, `Squash`,
`SquashIntro`, `SquashElim`, `Box`, `BoxIntro`, `BoxElim`,
`Constant{declaration, levels}`, and the arena `Dummy`
(never well-typed). Levels are `Constant`, `Parameter(i)`, `Successor`,
`Maximum` — no `imax`, matching the predicative selection.

Reduction `t ⟶ t'` is `conversion.rs::weak_head_normalize`, one budgeted
step per rule, along head spines only:

- δ: `c(ls) ⟶ body[ls]` for a definition constant `c`; an assumption
  constant is neutral.
- β: `(λ.b) a ⟶ b[a]`.
- π: `fst ⟨u,v⟩ ⟶ u`; `snd ⟨u,v⟩ ⟶ v`.
- ι-Two: `caseTwo(C,d0,d1,zero) ⟶ d0`; `… one ⟶ d1`.
- ι-Id: `J(C,d,y,refl A x) ⟶ d`.
- ι-W: `indW(P,s,sup A B a k) ⟶ s a k (λ(b:B a). indW(P,s,k b))` — the
  hypothesis domain `B a` is rebuilt from the `sup`'s checked annotations,
  so the step fires even when the ambient `W` type is neutral.
- ι-unsq: `unsq P f (sq A a) ⟶ f a`; `sEmpty_rect` has no constructor
  to fire on.
- ι-unbox: `unbox P f (box A a) ⟶ f a`.

Conversion `s ≡ t : T` (`convertible`) is decided by: the shared type must
itself be a type (`infer_sort(T)` runs first, so a malformed shared type
reports its own error); if its sort is `Strict`, both sides convert —
definitional proof irrelevance gated on the *shared type's* sort, never the
terms' shapes. Otherwise a reflexivity shortcut, weak-head normalization of
both sides, a second reflexivity probe, then head-matched typed recursion:
componentwise for `Pi`/`Sigma` (codomain under an extended context), for
`Lambda` at a `Pi` shared type, for `Pair` at a `Sigma` shared type, for
neutral spines (`Apply`, `Fst`, `Snd`, stuck
`CaseTwo`/`IdElim`/`IndW`/`EmptyElim`/`SquashElim`/`BoxElim`) at
inferred types, and for `Id`/`Refl`/`Sup`/`W`/`Squash`/`SquashIntro`/
`Box`/`BoxIntro` at the shared type's own
structure. Pair eta runs in both directions at a `Sigma` shared type; the
profile's typed function eta runs in both directions at a `Pi` shared type.
Sorts convert through `levels_equal`; assumption constants convert only at
the same declaration with semantically equal level instantiations.

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
- ι-W: the step's checked type `Π(a:A). Π(k:Π(b:B a). W A B).
  Π(_:Π(b:B a). P(k b)). P (sup A B a k)` applied to `a`, `k`, and the
  hypothesis `λ(b:B a). indW(P,s,k b)` lands at `P (sup A B a k)` — the
  eliminator's result. The hypothesis lambda's domain is rebuilt from the
  `sup`'s own checked `carrier`/`children` annotations; this is why the
  annotations are checked at typing and ignored at conversion.
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
- `W`/`indW`: `indW` fires only on `sup` and recurses on `k b` — children
  of the tree — so the candidate for `W A B` is defined by well-founded
  recursion on tree structure, the standard strong-normalization argument
  for W-types. This is the recursion the whole encoding stands on.
- Typed function eta and strict collapse are *conversion* clauses, not
  reduction rules: they cannot create nontermination. Their cost is paid
  inside the conversion termination argument, below — this is the precise
  sense in which the combined rules' metatheory reduces to the base
  calculus's.
- δ-unfolding terminates independently of normalization: signatures are
  prefix-checked, so each step strictly lowers the greatest reachable
  declaration index and recursion is impossible.

*Status:* argued at paper level, not mechanized — matching the reference
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

**Soundness** — `convertible` answering `true` implies `s ≡ t : T` in the
declarative judgment. *Argued:* every clause either is a declarative rule —
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
hang. The honest statement is therefore: the algorithm is *sound
unconditionally and bounded-incomplete* — a true conversion whose witness
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

## Encoding-scheme correspondence

The profile's derived indexed family is discharged *inside* the kernel:
the five `indexed_scheme` declarations are ordinary universe-polymorphic
definitions `check_signature` re-decides, so the correspondence obligations
are checks the kernel ran, not assertions about it.

- Formation: `IW i : Type max(l,u,v)` is `INDEXED_W`'s stored type —
  checked.
- Constructor: `isup : Π(a:A). Π(g:Π(b:B a). IW (next a b)). IW (out a)`
  is `INDEXED_SUP`'s stored type — checked.
- Induction: `iindW`'s stored type is the dependent indexed eliminator's —
  checked.
- Computation: `iindW Q s i (isup a g) ≡ s a g (b ↦ iindW Q s (next a b)
  (g b))` is *definitional* — `convertible` decides it, including with a
  neutral child function `g`, where the closing step is pair eta then the
  typed function eta the profile names as the dependency. *Witnessed*:
  `isup_and_iindw_check_and_compute_with_a_neutral_child_function` and the
  per-family computation tests (`induction_computes_on_modus_ponens_with_a_neutral_child_function`,
  `induction_computes_on_cons_with_a_neutral_tail`,
  `induction_computes_on_nil_with_a_vacuous_child_function`,
  `induction_computes_on_rnode_with_neutral_payload_and_children`, the
  mutual family's node/forest cases).
- Encoding soundness: `indexed_correctness` proves
  `∀(i:I). ∀(t:IW i). Id I (out (rootOf t)) i` inside the kernel by
  `iindW` itself — the encoding's defining property (a tree's recorded
  index is its constructor's `out`) is a checked theorem, not an axiom.

The [quotient](quotients.md#set-quotient-foundation) scheme keeps its
admitted/derived boundary explicit: the seven
interface assumptions (`Q`, `project`, `setQ`, `sound`, `effective`,
`elim`, `beta`) are named assumptions with exact statements; `transport`,
`idTrans`, `transportConst` and `lift` are checked definitions derived from
them. `beta` is a relevant `Id`, never a conversion rule, so a quotient's
representative stays unextractable; `liftPre` is admitted because a
function-valued motive would need the function extensionality the calculus
deliberately lacks — the boundary between admitted law and derivable
consequence is stated per declaration. *Witnessed:* the
`quotient_*` scheme tests, including `quotient_beta_is_propositional_evidence_not_conversion`
and `quotient_assumption_closure_records_the_admitted_interface`.

The bounded-certificate denotation is an untrusted elaborator, not a
trusted translation: `denote_bounded_certificate`'s output is a
`MathematicalCertificate` that `verify_mathematical_certificate` re-decides
in full, so the soundness obligation reduces to the kernel's own judgment.
Meaning preservation of the denotation table (atoms as `Type 0`
assumptions, scalar `Equal` as `Id` over a carrier assumption, connectives
as `Σ`/tagged sums/`Π`, a decided non-reflexive primitive as a named
decision assumption) is argued per connective in
`mathematical_core/bounded_denotation.rs`'s module documentation. Every
certificate rule family denotes. Propositional constructions and identity
symmetry/transitivity, including `ContentConservation`, elaborate to kernel
terms. Liftable scalar integers and `IntegerMath*` share `Int` and its
identity type, so the supported `Equal`↔`IntegerMathEqual` citation crossing
does not assume a conversion. Integer order rules apply one fixed roster
of explicit laws. Single-equation transport between identities uses identity
elimination; transport of integer order applies fixed endpoint-substitution
laws.

Fixed scalar literal magnitudes have shared signed binary definitions over
`zero`, `double`, `odd` and `negate`. Discreteness derives adjacent-literal
order from five fixed arithmetic laws, then uses mixed transitivity with
the inclusive premise. These laws remain assumptions with exact statements;
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
Whole closed-term resource refusals are unchanged. Other open arithmetic
remains opaque; these fixed laws are assumptions, not an arithmetic
consistency result. Recursive evaluator preflights may revisit prefixes;
shallow term storage does not establish linear checking cost.

Remaining families, including other bound and correlated-root
witnesses, multiple-equation or nested transport and transports outside the
supported integer vocabulary, denote
a *rule-instance decision*: an assumption constant of type
`Π(_ : ⟦premise₁⟧). … . ⟦conclusion⟧` whose premise/conclusion relation
is re-decided during denotation by the same shared function the bounded
checker runs, applied to the denoted premise evidence. The judgment's
assumption closure then names the instance's arithmetic or conversion
content exactly. Source invalidity, unsupported valid encodings and producer
defects remain separate outcomes. *Witnessed:* the bounded-denotation unit
tests inspect exact law closure and identity compositions;
`compiler/tests/kernel_discreteness.rs` independently checks a source-produced
certificate, its mathematical wire roundtrip and its invalid control.
`compiler/tests/kernel_subtract_order.rs` checks the complete source-produced
ranked-loop edge and its subtraction operation against independently
reconstructed obligations, exact assumption closure through the mathematical
wire, and rejection of a non-decreasing rank. A guarded decrement-two operation
checks the same lower-bound rule and rejects an insufficient guard. The
`terminal-codec` bounded-certificate tests exercise retained declarations,
exact shared-integer closure, universally quantified order laws and
fixed-to-mathematical equality citations. Changed endpoints, forged evidence,
mismatched conclusions and malformed wire data reject.

What the kernel does *not* discharge: faithfulness of the encoding to a
*source* declaration — positivity, nominal identity and statement fidelity
are elaboration-side obligations owned by `PROOF-CONTRACT-MIGRATION`. The
kernel's guarantee ends at the checked declaration graph.

## Evidence ledger

| Obligation | Evidence |
| --- | --- |
| Formation/typing rules, sorts, pairs, eta, ceiling, storage receipt | `type_checking.rs` (24 tests) — including the 51-slot polymorphic-identity receipt and `checking_the_polymorphic_identity_retains_bounded_storage` |
| `Id`/`Two`/`W` formation, dependent elimination, computation, neutral controls | `identity_and_w_types.rs` (15 tests) — including `identity_proofs_stay_relevant_without_uip` and `pointwise_two_agreement_grants_no_function_equality` |
| Strict layer: `sEmpty` ex falso, squash formation/introduction/elimination/computation, box formation/introduction/elimination/computation, irrelevance boundaries, derived `sUnit`, declaration checking | `strict_layer.rs` (14 tests) |
| Level scope, parametric checking, declaration discipline, closure | `levels_and_declarations.rs` (17 tests) |
| Indexed + quotient schemes checked as signatures, applications, boundaries | `schemes_and_quotients/` |
| Acceptance families: vector length, mutual, nested, derivation context/conclusion, level polymorphism | `tests/indexed_{vector,mutual,nested,derivation,levels}.rs` — each pins constructor computation on neutral children, rejection controls, exact closures and measured receipts |
| Bounded rule families | `tests/{equality_symmetry,predicate_conversion,predicate_denotation,value_equality_transport,integer_order_weakening}.rs` |
| Canonical wire: byte-identical re-encode, independent re-verification, closure through the wire | `terminal-codec` tests `mathematical_certificate`, `theorem_certificate`, `bounded_certificate` |
| Measured cost examples | derivation `next` family checking: 28_946 budgeted steps; a derivation certificate: 4_794 steps with 278_515 retained arena slots (`indexed_derivation.rs`) |

## Trust boundary and explicit non-claims

Trusted code is the kernel proper: `term.rs`, `substitution.rs`,
`typing.rs`, `conversion.rs`, `signature.rs`, `certificate.rs`. The schemes,
theorems and bounded denotation are *data* the kernel re-decides; a defect
in them surfaces as an ordinary rejection, not a false judgment.

The reference core's strict layer is implemented: `sEmpty` with ex
falso into either sort, `Squash`/`sq`/`unsq` (existence without witness
extraction; the dependent eliminator is derived through irrelevance,
not primitive), `Box`/`box`/`unbox` (the converse embedding, whose
strict-motive non-dependent instance is the plain projection
`Box A → A`), and the derived strict unit `sUnit := Π(_ : sEmpty).
sEmpty`. Definitional proof irrelevance is governed by the shared
type's sort, so `Box`'s relevant payload is precisely where a neutral
proof stays distinct from a canonical `box`. `core::Squash` from
[mathematical bindings](mathematical_bindings.md) now has a kernel
counterpart; what still gates the squash cases of
`PROOF-CONTRACT-MIGRATION` is the source-side migration, which that
item owns.

Not claimed: K/UIP, identity eta, `Two` eta, W eta, function
extensionality, equality reflection, cumulativity, `imax`, impredicativity.
Conversion decides definitional equality only; it is sound unconditionally
and bounded-incomplete under the ceiling. Normalization is argued at the
reference paper's section-4 level, not mechanized. Exact assumption closure
is computed over the stored signature; receiver acceptance of that closure
is policy outside the kernel.
