# Proofs, evidence and quotients lane

The owner paused this lane on 2026-11-11 after an outside review and lifted
the pause on 2026-11-12 (see [check on the lowered form](check_on_lowered_form.md)).
Slice 1 is built; nothing else here is started.

Staging plan, 2026-11-11, for one of the five lanes of the first parallel
group in [check on the lowered form](check_on_lowered_form.md). The owner
authorized starting it ahead of route one's gate. It is scoped the way the
[places foundation lane](places_foundation_lane.md) is.

The new checker refuses every program that writes a proof. An `[erased]`
binding, a theorem cited for its conclusion, a named proposition, an identity
claim or a quotient is `Unsupported` today, so the whole program falls back to
the current checker. This lane lets the checker on the lowered form hold those
constructs: each lowers by structure, and each obligation it creates is
decided by the one prover with a certificate the proof kernel checks. The
lane adds no second prover and no assumed fact.

Most of the lane's programs cannot enter the subset by this lane alone. The
probe below shows why: a proof fixture usually has no program entry, or
prints, borrows or uses generic data. So the lane is measured the way the
places lane was: by the refusals it removes, by agreement on the programs
that do enter, and by tests on closed bodies for the rest.

## Scope

### In, by slice

Each slice lands on its own with its own measured agreement. The order is
by dependency: a later slice uses what an earlier one built.

| Slice | Source construct | Lowered form | Specification |
| --- | --- | --- | --- |
| 1 | `[erased]` scalar formals and locals, and the term a call supplies for an erased formal | `MachineContract::erased_scalar_formals`; the call's `erased_arguments` as `ScalarTerm`; no operation | [proof contracts](../../spec/proofs/contracts.md#explicit-erased-bindings): "The binding remains in checking and semantic identity but contributes no runtime field, address, read, or cleanup." |
| 2 | A runtime expression that reads an erased binding, rejected by this route | A refusal that is a verdict about the program, at the read | the same section: "it cannot determine runtime data or control." |
| 3 | A theorem cited as a statement call: a resultless machine whose `ensures` the caller uses, with its `requires` proved at the citation | `CallUnit` while the callee has a runtime body; the callee's `ensures` as `CalleeGuarantee` knowledge | [citation and induction](../../spec/proofs/contracts.md#citation-and-induction): "Establish its `requires` under exact operand substitution before importing its `ensures`; erase the call if fact-only." |
| 4 | Erased terms that compute (`keep(n, a + 1)`), with their formation conditions as obligations | A term in `erased_arguments`; one obligation per partial operation in it | [total arithmetic](../../spec/proofs/contracts.md#total-arithmetic): "Exact operations discharge representability before formation." |
| 5 | Erased record formals, erased data fields, and literals that supply them | `erased_proof_formals`, `ProofTerm::Construction`; an erased field outside the structural type graph | proof contracts: "Erased fields need not enter the executable structural-type graph; layout skips them before ABI classification. Semantic fingerprints retain them." |
| 6 | Named mathematical definitions and propositions in clauses (`let greater_than(limit, value): core::Strict<0> = value > limit;`), unfolded | The clause's proposition after unfolding; no operation | [mathematical bindings](../../spec/proofs/mathematical_bindings.md#named-mathematical-definitions): "It introduces a transparent checked term definition. Naming a proposition does not prove it". |
| 7 | Structural `==` between records and sums in clauses and bodies, `core::Identity<T>` | Conjunction of leaf equalities over values; `StructuralCaseMembership` values for tags | proof contracts: "a true record comparison yields each selected field comparison; sum comparisons retain tag agreement and the active payload case." |
| 8 | A payload named where the other cases are refuted, and a dead arm discharged by proof | The places lane's payload judgment, with refutation as a kernel-checked goal | [structural access](../../spec/terminal-psi/structural_access.md#store-vocabulary): "Otherwise the case holds when every other declared case of that sum is refuted." |
| 9 | Boundary machines and `boundary let` as assumptions, with their closure reported | Knowledge justified by the callee's contract, marked as resting on an assumption | [axioms and receiving policy](../../spec/proofs/contracts.md#axioms-and-receiving-policy): "A bodyless boundary machine's `ensures` is an axiom claim until admitted, not a checked theorem." |
| 10 | Calls as denotational terms in clauses (`ensures minus(a, c) == minus(b, c)`), and call congruence | A term form for an application; congruence as a kernel rule or its absence reported | proof contracts: "Repeated admitted pure comparisons with the same selected operation and exact unchanged arguments have the same Boolean denotation." |
| 11 | Quotients: `as Quotient`, `Quotient::lift` and `Quotient::define` with their congruence and transport theorems | Ordinary checked bodies; the correspondence as a checking judgment only | [quotients](../../spec/proofs/quotients.md#quotient-correspondence-judgment): "The correspondence is a checking judgment only. Terminal Psi does not retain it, and it grants no executable operation". |

Slices 1 to 5 need nothing the kernel lacks: their goals are the integer
orders and equalities the route-one prover already certifies. Slices 6 to 8
add proposition forms and may need the prover to split an equality by field.
Slices 9 to 11 need term forms and rules the kernel may not have. Each of
those slices starts by checking that, and a missing kernel rule is reported
in its pull request, in plain words, before anything is built on it.

### Out

| Construct | Owner |
| --- | --- |
| Proof-only recursion, ranking and induction (`Nat`, `Ordinal`) | loops, recursion and termination |
| Trait laws, conformance evidence, generic theorems, universe levels | generics and traits |
| Float meaning, `embed`, policy operators in terms | integers, floats, casts and domains |
| Domain predicates and named domains as clause facts | integers, floats, casts and domains |
| Witness bundles and squash over generic data | generics and traits, after slice 5 |
| A reference to an erased binding, erased array extents | references and loans; slices, collections, indices and text |

Everything out is refused as `Unsupported` with a detail that names the
construct. Nothing is skipped.

## How the lane keeps one route

- **No table and no justification is added by a slice.** An erased actual is
  a term, which `Actual` cannot hold. Slice 1 reads it from the call in the
  lowered form, where Terminal Psi already keeps it, and records the gap in
  the check-tables README. A slice that needs a new `Justification` (slice 9,
  for an assumption) stops and proposes it in a pull request of its own.
- **One prover.** Every goal goes to `Prover::prove` and
  `verify_obligation`. Unfolding a definition, substituting an erased term
  and expanding a structural equality all happen before the prover starts,
  so it sees values and propositions.
- **Values only in knowledge.** The places lane's choice holds here: a
  structural identity is stated as equalities between the values its leaves
  hold, never as a `Proposition` that names a place.
- **Identity is fixed upstream.** A definition, proposition, theorem or
  quotient operation is named by its symbol. Where the typed trees hold one
  only as a spelling, the slice repairs the stage that lost it or refuses.

## The corpus slice

**The probe was stopped part-way when the lane was paused on 2026-11-11;
these counts are partial.** It ran `structure_elaboration_corpus`, built at `main` `2df885a2a`
with `CARGO_PROFILE_DEV_OPT_LEVEL=1`, on a cloud Linux x86_64 VM with 4 CPUs,
three at a time. The selection is the 375 `pass/` and `run/` fixtures outside
`facts/` that use one of the lane's constructs (`[erased]`, `core::Identity`,
a quotient, a `proposition` or a parameterized top-level `let`) or sit in
`proofs/`, `relevance/` or `math/`. 289 of the 375 completed, in name order;
the last 86 were not run. The probe reports only the first refusal of each
closure.

| Probe stage | Fixtures |
| --- | ---: |
| refused | 162 |
| no program entry | 79 |
| current checker failed (so no typed trees) | 37 |
| ran (already inside the subset) | 11 |

First refusals, top six:

| Refusal | Fixtures | In this lane |
| --- | ---: | --- |
| `machine service reach` | 65 | no |
| `type data with custody` | 27 | no (ownership) |
| `type constrained` | 23 | no (domains) |
| `type addr` | 8 | no |
| `type fixed array` | 7 | no (collections) |
| `contract clause that is not a comparison or conjunction` | 6 | yes (slices 6, 7 and 10) |

Only 3 first refusals are the ones slice 1 removes (`parameter erased
parameter` 2, `statement erased local` 1). How many programs enter was not
estimated: the screen by reading that the places plan did has not been done
here. What the counts already say is that 79 of 289 bind no entry and 65 are
stopped at service reach, so few of the lane's programs can enter until
those two are served by their owners.

## Measures and acceptance

Each slice reports, as route one's measures do:

1. **Agreement.** Every corpus program the slice brings into the subset is
   checked by both routes, and each disagreement is classified. Target: no
   program the specification accepts is rejected, and nothing false is
   accepted.
2. **Kernel-checked evidence** per obligation kind, with its denominator.
3. **Decisions keyed on text: 0. Silent drops: 0. Routes per obligation
   kind: 1.**
4. **Lines added**, route production code and tests apart.
5. **A must-reject twin** for each construct, differing only in the
   construct's own input.
6. **The error the programmer sees.** For every `fail/` fixture the slice
   handles: how many the new route rejects at all; of those, how many
   messages contain the fixture's pinned `expected.txt` fragment or name the
   same obligation; and of those, how many point at the source construct the
   current checker's diagnostic names, at another site, or are `Unpositioned`
   with a reason. Each miss is listed and explained. Where a construct's
   lowered form does not carry enough to name its source, the slice says so.

A proof found once may be carried forward, and every consumer checks it
again against its own goal (`specialization-certificate-reuse`;
`lowered_discharge::reverify`, `#2309`). The lane never treats a pass from an
earlier compile as a check, and searching again is always allowed.

## Dependencies

| PR | What the lane needs from it | If it does not land |
| --- | --- | --- |
| `#2317` join predicates, crash coverage, `result` renamed at a call | Slices 3 and later use callee guarantees that name `result`; it also changes `substitute`, which slice 1 extends | Slice 1 merges with it by hand; slice 3 waits |
| `#2309` saved-proof re-verification | Not needed; it splits `discharge`'s per-machine steps, which the lane's edits follow | None |
| `#2362` data `requires` on the lowered form | Slice 5 touches the same data declarations | Slice 5 follows it |
| A per-machine elaboration root for programs with no entry (`#2312` leaves it open) | Most proof fixtures bind no entry | The lane measures those constructs on closed bodies only |
