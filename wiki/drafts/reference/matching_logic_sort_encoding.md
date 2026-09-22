# Typed-to-one-sorted encoding for matching logic

Status: exploratory draft, not an accepted design or implementation task.
It answers one bullet of the
[comparison checklist](matching_logic.md#possible-bounded-comparison): encode
`Nat`, `Int`, `addr`, slices, and a user sum into the one-sorted finitary basic
matching logic Chen and Rosu prove complete for, and record what the encoding
moves rather than what it discharges. No checker, translation, or axiom set in
this note is admitted authority; every encoding clause is an axiom admission
unless a checked producer can derive it. Delete this draft once the bounded
comparison rules on the encoding, or when the matching-logic route is dropped.

## Setting

The target fragment is one-sorted: there is exactly one sort, and every
element of every model inhabits it. Omega's subjects are typed; the kernel's
`mathematical_core` retains `Sort`/`Level`/`Term` per term. Encoding Omega
types therefore means replacing sort annotations with membership patterns —
`Nat(x)`, `Int(x)`, `Addr(x)`, and so on — each a predicate the subject must
provably satisfy, plus disjointness and definedness obligations that a
syntactic sort system normally states for free. Every membership predicate is
a definedness-style pattern: `x ∈ T` must be provable or provably refutable
for any reachable `x`, and operations carry definedness preconditions so that
junk (below) cannot satisfy them by accident.

## Encoding table

| Omega type | One-sorted pattern | Obligations the sort carried |
| --- | --- | --- |
| `Nat` | `Nat(x)` | Totality of constructors; closure of `+`, `*` over members |
| `Int` | `Int(x)`, disjoint from `Nat(x)` | Signed range per width; no overlap with `Nat` — `Int` is not `Nat`'s nonneg half |
| `addr` | `Addr(x)` | Interpreted under the selected target's address model; membership grants no storage or access authority |
| `u64 in Small` for a predicate domain requiring `self <= 3` | `Nat(x) ∧ 0 ≤ x ∧ x ≤ 3` | Carrier membership and the declared bound predicate; revision at each predicate change |
| `&[T]` slices | `∃b l. x = ⟨b, l⟩ ∧ Addr(b) ∧ Nat(l) ∧ ∀i < l. T(b + i)` | Bounded quantifier over the pointee region; pair constructor must be injective |
| user sum (`enum`) | `Sum(x) ⟺ ⋁_k (Tag_k(x) ∧ Payload_k(x))` | Pairwise tag disjointness, exhaustiveness, payload membership per arm |

`Int` and `Nat` share the same underlying integer elements in most intended
models, so the encoding must either place them under one membership with a
sign/range refinement or keep them disjoint — conflating them silently changes
what `x : Int` proves about `-1`. Prefer a single `Integer` membership with
per-type range refinement predicates (`Nat(x) ≡ Integer(x) ∧ 0 ≤ x`,
`Int64(x) ≡ Integer(x) ∧ -2^63 ≤ x < 2^63`); the refinement reading keeps the
junk discussion uniform.

## Sort membership

Membership is the encoding's load-bearing move: every typed assertion `x : T`
becomes the pattern `x ∈ T` (a definedness pattern over the membership
predicate). Two consequences:

- A statement that quantifies over `Nat` quantifies over a predicate, not a
  sort; the axiom set must carry closure, induction schema where used, and
  disjointness from other memberships explicitly.
- Membership in a predicate-only domain combines carrier membership with its
  declared predicates. For `domain u64::Small requires self <= 3;`, a caller
  that learns `y ∈ (u64 in Small)` must recover the carrier fact and the bound,
  here `Nat(y) ∧ 0 ≤ y ∧ y ≤ 3`. Predicate formation must be total. Ordinary
  contracts and guards can establish the same bound without an explicit domain
  qualification; they do not create a distinct scalar range-annotation type.

## Definedness

One-sorted models contain elements on which a predicate is simply false; an
operation like division or slice indexing is a partial function that the
encoding must state as a definedness precondition (`⌈i < l⌉` on `b + i`),
not as a typing rule. Missing a definedness guard does not produce a wrong
answer — it produces an unprovable or vacuously-satisfiable obligation, which
the checked comparison then mis-attributes to the proof producer. Every entry
in the table above lists its definedness preconditions beside the membership
pattern.

## Junk models

The one-sorted universe admits elements in no Omega type at all — junk. A
junk element must not satisfy a typed claim, and must not make a universal
typed claim vacuously true. Concretely: quantifiers over an Omega type
quantify over the membership predicate (`∀x. T(x) → φ`), never over the bare
universe; a comparison that reports "holds" must verify the intended model
actually contains members, because `∀x. T(x) → φ` is vacuous when `T` is
uninhabited. The intended-model check is part of the comparison's evidence,
not an axiom.

## Revisions

A revision point is anywhere a bound or membership premise changes mid-proof:
a narrowed range, a reborrowed referent, a shrunken slice window. Each
revision is a new membership assertion for the same element (`x ∈ Nat` is
retained while `x ≤ l` tightens to `x ≤ l'`), and the encoding must record
that the prior assertion is not invalidated — only refined. An imported
certificate that silently substitutes the tighter predicate without deriving
it from the weaker one is a translation admission, not a derived step.

## Borrows and multiplicity

Ownership is a type property, not a value shape: `&T`, `&mut T`, and
`&write T` are loan forms over the same carrier ([ownership and
multiplicity](../../spec/language/ownership.md)). In the one-sorted encoding a
loan is a distinct membership class `Loan_k(x)` indexed by kind `k ∈ {shared,
mut, write}`, with multiplicity expressed as disjointness axioms: an element
carrying `Loan_mut` cannot simultaneously carry another `Loan_shared` or
`Loan_mut` over the same region; shared loans may duplicate, exclusive loans
may not. Reborrows retain immediate parent lineage — encodeable as a
`ChildOf(x, y)` predicate inheriting the parent's region membership. Because
these axioms are the only thing standing between the encoding and parallel
mutable aliases, they are the encoding's highest-trust clauses and the
natural audit boundary.

## Evidence record

Per the source note: retain logical fragment (one-sorted finitary basic ML,
no fixpoint symbols unless a bounded certificate supplies them), rule and
semantics versions, exact subject, target capsule, observation profile,
bridge graph, and admissions. The completeness theorem supplies no Omega
authority by citation; encoding semantics as axioms relocates trust to the
axiom set rather than discharging it. Pinned positive and negative cases must
be identical between the current route and the encoded comparison before any
cost claim is readable.
