# Proof kernel core audit

Omega cannot compile a program whose proofs need a mathematical declaration
beyond a closed scalar predicate. PR #2823 deleted the code that turned
definitions, universe levels, `data` families, theorem machines,
`core\Identity` and quotients into terms the proof kernel checks. The kernel
and its tests survived, but no source stage calls it for those declarations.
This note says what was lost, which outcomes the spec still requires, how
stage 05 turns a source clause into a proof term today, and what to build
first for **PROOF-KERNEL-CORE** in [TASKS.md](../../../TASKS.md).
Delete it when that task closes or its plan carries these findings.

## Summary

1. **Lost.** #2823 deleted the source-side elaborator
   (`mathematical_declarations.rs`, 12,717 lines) and the quotient checks in
   `validation/src/quotients.rs` (8,175 lines). The kernel and its tests
   survived intact, but no stage now gives it definitions with binders,
   universe levels, `data` families, theorem machines, Identity over
   non-scalar types or quotients.
2. **Still needed.** Three general rules, not a port: (R1) every
   mathematical declaration becomes a kernel signature entry with its levels,
   and Identity works for every type; (R2) a `data` family becomes one W
   description, and each construction carries its evidence on the value;
   (R3) the lowered-form checker kernel-checks that signature and takes
   induction from the kernel. Separately, the checker misses two outcomes the
   automatic proof floor requires: divisors known only by an interval, and a
   contradiction between two open order facts. Everything else in the old
   `checker.rs` and `obligations.rs` is covered, subsumed or obsolete.
3. **Core's `no_wrap` already works.** It is a closed scalar predicate, which
   stage 05 serves today; a stage 06 test accepts it and refuses the call
   without the bound. What core lacks is Identity over data and generic types
   (`algebra.omg`, `cauchy.omg`, `nat.omg`, `rat.omg`, `extent.omg:219`).
4. **No sample reaches the kernel gap yet.** Of 176 samples run, 0 pass and
   2 cannot run on `linux_arm64`; every failure stops earlier. The largest
   blockers are domain rules the samples predate (53), value-indexed type
   arguments that stage 03 cannot evaluate (48), the level binder `u` read
   as a builtin type (18) and an unresolved `core::Access` import (18).
   This audit has no run of these samples from before #2823, so it measures
   the loss by reading code, not by programs that stopped compiling.
5. **The per-placement pattern moved into stage 05.** Stage 06 gives every
   value the same treatment, but the code that reads a source clause does
   not. Thirteen functions in four files read an expression into a proof
   term, each accepting its own subset of expressions for its own clause
   position. `Proposition` has five equality forms, one per kind of operand,
   and none that takes a type. That is why Identity serves two types and a
   call inside a clause serves one shape of machine. See
   [how a clause becomes a proof term today](#how-a-clause-becomes-a-proof-term-today).
6. **Do first.** Build toward the core library, not the samples: it needs no
   level binder, no `core\Squash` and no `data` family. Give stage 05 one
   term reader, add the two term forms `Proposition` lacks (application of a
   declaration, and `Id` at a type), and take `core/algebra.omg` as the first
   customer. Families, induction and quotients follow when a library
   declaration or a repaired sample needs them. See
   [plan order](#plan-order).

## How to read this audit

The yardstick is the spec's required outcomes and the programs that need them,
never agreement with the old checker.
[Check on the lowered form](check_on_lowered_form.md) records why: a checker
that rebuilds execution from source trees makes "each source arrangement ...
another opportunity to lose a fact", while explicit operations and exact
value/place identities give "one obligation ... one meaning wherever its
operand came from". Its decision item 1 forbids elaboration from using checker
facts to choose program meaning, and its lessons say to represent
relationships instead of adding a case per source placement. The old checker
did the opposite: one collector, one checker and one diagnostic per placement.

So each old capability gets one of four labels:

| Label | Meaning |
| --- | --- |
| **covered** | Today's single mechanism already gives the outcome generally. |
| **subsumed** | The old rule existed for one construct or placement; the value/place model makes it unnecessary, even with no exact equivalent. |
| **needed** | The spec requires the outcome and nothing on main routes to it. The row names the one general rule to add and the stage that owns it, not a list of cases. |
| **obsolete** | The spec no longer requires the outcome, or the old code was dead. |

[Stage trust](stage_trust.md) and the
[stage 06 README](../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/README.md)
define "what exists now". Do not port old functions; the old line ranges are
cited only so a reader can see what an outcome used to cost.

**Subsumed** is a claim about stage 06, argued from its design and not tested
here. It does not say stage 05 reads the construct: where stage 05 still
refuses a placement the old collector served, the row says so.

## Sources

| Side | Revision | How it was read |
| --- | --- | --- |
| Old code | `cea9cdcde460^` (parent of #2823's merge `cea9cdcde460`) | `git show cea9cdcde460^:<path>`; line numbers below are in those files |
| New code | `origin/main` at `6bab34687545` | worktree checkout |
| Sample run | `6bab34687545` on `omega-linux-arm64` | one `mbx xtask sample-gate` batch, [below](#who-needs-it-today) |

Old files, all under `omega-rust/psi/compilation/`:

| File | Lines | Deleted by |
| --- | --- | --- |
| `proof/src/mathematical_declarations.rs` | 12,717 | #2823 |
| `proof/src/checker.rs` | 6,649 | #2823 |
| `proof/src/obligations.rs` | 4,107 | #2823 |
| `proof/src/derivation_store.rs` | 299 | #2823 |
| `validation/src/proof_contracts_core.rs` | 11,591 | #2823 |
| `validation/src/quotients.rs` | 8,175 | #2823 |
| `validation/src/float_and_mathematical_contracts.rs` | 3,771 | #2823 |
| `proof/src/lemmas.rs`, `boundary.rs`, rest of `lib.rs` | 377, 213, 815 | #2911 (`c3dd18abe600`); they had no callers at `cea9cdcde460^` |

`proof/admission/` (the kernel) was not touched by #2823; see
[what else was dropped](#what-else-was-dropped).

## The deleted elaborator, by capability

`mathematical_declarations.rs` read typed trees through the deleted
`validation` crate (69 `validation::` references) and wrote
`checked_trees::CheckedMathematical{Declaration,Signature}`
(`:11847-11855`, `:12683-12684`). Its entry was
`check_mathematical_declarations` (`:12688-12703`), called from old stage 04
`preparation.rs:2051,2110` and old stage 05 `checking.rs:1025,1047`. It never
read `derivation_store` or the old facts. About 43% of it (≈5,460 lines) is
general elaboration logic, 45% (≈5,740) is per-case code and plumbing that fed
the old checker, and 11% (1,442) is tests in four `#[cfg(test)]` modules
(`:6048-6168`, `:7088-7161`, `:9080-9842`, `:9847-10330`).

The pattern of one piece of code per placement explains the deletion of the
old range checker, but only about half of this file. The general half went
because it depended on crates #2823 removed, and that dependency was narrow:
of its 69 `validation::` references, 17 name the typed-tree view
`CheckingInput`, 29 name one helper module, `mathematical_applications`, and
23 name other helpers. #2823 chose to move no old checking code, and its
`TASKS.md` change left **PROOF-KERNEL-CORE** as it was, so this audit is the
first record of the loss. The file is still a reference and not a source to
restore: its output type `CheckedMathematicalSignature` is gone and its
per-placement half must not return. Its tests are the best record of what
used to work; port their cases as expectations for the new code.

Paths: S03 = `source-to-psi/pipeline/03_symbol-resolved-trees-to-typed-trees/src/`,
S05 = `source-to-psi/pipeline/05_prepared-typed-trees-to-lowered-program/src/`,
K = `psi/semantics/proofs/mathematical/`, all under
`source-rust/subsystems/compiler/`. Stage 05 prints a refusal as
"not supported yet: <construct> <detail>".

| # | Capability and example | Old code | Spec that requires it | Main today | Label and general rule |
| --- | --- | --- | --- | --- | --- |
| C1 | **A named definition becomes one kernel declaration** (Π over the telescope, λ body, constants applied by prefix; `boundary let` becomes an assumption). `public let no_wrap(base: addr, length: u64): core\Strict<0> = embed(base) + embed(length) <= addr::Bound;` | `elaborate_declaration` `:12028-12113`, binders `:12115-12157`, body `:12180-12353`, scope `:3715-3922`, callee instantiation `:473-647`, dependencies `:1448-1552`; ≈1,050 | [mathematical_bindings.md#named-mathematical-definitions](../../spec/proofs/mathematical_bindings.md#named-mathematical-definitions), [#assumptions-and-executable-demand](../../spec/proofs/mathematical_bindings.md#assumptions-and-executable-demand), [foundation.md#assumptions-and-calculus-identity](../../spec/proofs/foundation.md#assumptions-and-calculus-identity) | Stages 01-03 parse, resolve and type it (S03 `declarations.rs:1447-1538`). S05 `read/subjects.rs:566-578` sends every subject definition to `mathematical_predicate_definition` (S05 `read/mathematics.rs:251`), which serves only a closed definition with scalar formals and a `core\Strict<0>` result, as a `Proposition`, not a kernel term. It refuses binders ("open mathematical predicate declaration", `:275`), other results (`:282-286`), `boundary let` ("uninterpreted mathematical premise theory", `:287-289`), recursion (`:290-292`) and open applications (`:144-149`). No source stage calls `check_signature` (K `signature.rs:80,89`); its only non-test callers are the kernel's own `bounded_denotation.rs:12390` and `certificate.rs:35,665`, and the Psi wire decoder. | **needed** (R1). Elaboration in stage 05 turns every typed declaration into a closed kernel `Declaration` and publishes one signature table; stage 06 checks it once with the kernel. The old topological-order refusal (`:509-513`) is incidental. |
| C2 | **Universe levels**: `core\Level` binders, `Type<u>`/`Strict<u>`, generalization, call-site level inference. `let same_result<u: core\Level, A: core\Type<u>>(left: A, right: A): core\Type<u> = core\Identity::<A>(left, right);` | `elaborate_level` `:2345-2399`, `generalized_level` `:2683-2706`, level slots `:520-647`, call inference `:676-893`, `source_levels` `:11537-11632`; ≈420 | [mathematical_bindings.md#universes-and-mathematical-function-types](../../spec/proofs/mathematical_bindings.md#universes-and-mathematical-function-types), [foundation.md#universes-functions-and-recursion](../../spec/proofs/foundation.md#universes-functions-and-recursion) | S03 checks arity only (`type_reference.rs:891-901`). Nothing classifies `core\Level`; the stage 02 comment hands it to "the typed-tree elaboration leg" that no longer exists (`02_.../lowering/mathematical.rs:9-11`). Any level binder is refused by C1. | **needed**, part of R1: one level telescope per declaration, generalized from the interface. |
| C3 | **Mathematical type formers**: arrows as Π, `core\Squash<T>`, sorts, generic `data` application as a family. `present: core\Squash<Witness<u,v,A,P>>` | `elaborate_mathematical_type` `:2401-2439`, `elaborate_type_reference` `:2441-2552`, `named_type` `:2610-2663`; per-scalar carriers `:896-1385` (490) | [mathematical_bindings.md#universes-and-mathematical-function-types](../../spec/proofs/mathematical_bindings.md#universes-and-mathematical-function-types), [#witnesses-existence-and-proof-construction](../../spec/proofs/mathematical_bindings.md#witnesses-existence-and-proof-construction) | S03 lowers the type forms structurally. S05 refuses arrow inputs ("higher-order mathematical premise input", `read/mathematics.rs:196-199`); nothing handles Squash after stage 03. | Π/Squash/sort mapping **needed**, part of R1. The per-scalar carrier assumptions are **subsumed**: scalars take the bounded denotation's carrier theory. |
| C4 | **Boolean and arithmetic bodies as propositions**: comparisons, connectives, `embed`, integer `+`/`-`. `let within(limit: u64, value: u64): core\Strict<0> = !(value > limit);` | `propositions` `:3323-3714`, `operand_terms` `:2806-3322`; ≈900 | [mathematical_bindings.md#assumptions-and-executable-demand](../../spec/proofs/mathematical_bindings.md#assumptions-and-executable-demand) (total Boolean computation as proposition), [contracts.md#mathematical-values-and-quantification](../../spec/proofs/contracts.md#mathematical-values-and-quantification) | S05 reads the body as `Proposition`/`IntegerMathTerm` (`read/mathematics.rs:389-473`) and unfolds it at use (`mathematical_clause`, `:220-250`); stage 06 proves it through `proof_admission`. | **covered** for closed scalar first-order bodies. The old per-operator kernel constants are **obsolete**. Under R1 the same body must also become a kernel term once binders appear. |
| C5 | **`core\Identity::<T>(l, r)` as `Id` formation** at T's level, for every T. `ensures core\Identity::<T>(add(a, b), add(b, a));` (`core/algebra.omg:29`) | `elaborate_identity` `:1580-1636`, `identity_declaration` `:1638-1685`; ≈106 general. `:1686-2331` (≈670) only fed old-checker rows | [mathematical_bindings.md#scope](../../spec/proofs/mathematical_bindings.md#scope), [contracts.md#data-identity-in-facts](../../spec/proofs/contracts.md#data-identity-in-facts), [inductive_profile.md#primitive-rules](../../spec/proofs/inductive_profile.md#primitive-rules) | S05 serves two subject types: integers become scalar equality, `FloatMeaning` goes through `float_meaning_identity` (`read/contracts.rs:915-920`). Every other T is refused ("Identity of an unserved type", `:921-927`), and so is any use outside a machine `requires`/`ensures` ("Identity outside a machine entry or return clause", `:750-754`), which covers domain predicates and definition bodies. `TerminalIdentityFormations` (`psi/proofs/theorems.rs:11-26`) has a wire reader and no source writer. | **needed** as one rule, part of R1: Identity forms `Id T l r` for every T. The two-type special case is incidental. |
| C6 | **W-based indexed families from `data`**: uniform parameters vs indices, payload and child fields, case constraints, `U(A)` payload lifting. `data Vector<Length: u64, Total: u64> requires ... { case Nil; case Cons(..., tail [erased]: Vector<previous, previous_total>); }` | `source_family` `:10808-10869`, `build_source_family` `:10871-11242`, template core `:10655-11536`, description builder `:7731-10331` (≈1,350 without tests); demand gating `:10332-10654` (323) incidental | [inductive_profile.md#derived-indexed-families](../../spec/proofs/inductive_profile.md#derived-indexed-families), [#declaration-correspondence-and-strict-logic](../../spec/proofs/inductive_profile.md#declaration-correspondence-and-strict-logic) | Nothing builds a W description. An erased formal of data type is refused ("parameter erased parameter of a data type", S05 `read/machine.rs:4057-4062`); open domain-family application is refused (`read/types.rs:176-184`). Kernel `indexed_scheme` / `IndexedFamily` (K `indexed.rs:429-523`) have only test callers. | **needed** (R2): one encoding from a typed `data` declaration to an indexed W description, owned by elaboration in stage 05, checked by the kernel. |
| C7 | **Constructor applications as `sup`/`pack` evidence**. `let next [erased]: Vector<NextLength, NextTotal> = Vector::Cons { previous: Length, ..., tail: tail };` | `source_family_constructor_term` `:6506-7026`, `:6478-6504`; per-placement chasing `retain_source_constructor_schema` `:6202-6434` and `:6517-6560` | [inductive_profile.md#derived-indexed-families](../../spec/proofs/inductive_profile.md#derived-indexed-families), [#declaration-correspondence-and-strict-logic](../../spec/proofs/inductive_profile.md#declaration-correspondence-and-strict-logic) | No producer; refused by the C6 refusals and "expression erased term other than a literal or a binding" (S05 `lower/operations.rs:114-121`). | **needed** (R2), placement-free: the construction operation carries constructor evidence on the value it produces. The old chase through locals, parameters and states is **subsumed** by the value/place model. |
| C8 | **Theorem machines, citation and source induction**: a resultless machine with erased family evidence becomes a Π statement proved by `indW`. `machine vector_bound<Length: u64, Total: u64>(evidence [erased]: Vector<Length, Total>) terminates by Length ensures embed(Total) <= 10 * embed(Length)` | `retain_indexed_contract_theorems` `:4942-5306` (a shape filter that silently skips non-matching machines), `source_family_induction` `:7223-7606`, `terms::induction` `:8919-9078`, `parameter_closure` `:5586-6047` | [mathematical_bindings.md#logical-hypotheses-and-machine-use](../../spec/proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use), [contracts.md#citation-and-induction](../../spec/proofs/contracts.md#citation-and-induction), [inductive_profile.md#primitive-rules](../../spec/proofs/inductive_profile.md#primitive-rules) | Citation of a theorem is an ordinary call: `CallRequirement` (S05 `lower/calls.rs:1022`) plus `CalleeGuarantee` (stage 06 `lib.rs:1307`). The `TerminalIndexedTheorem` slot (`psi/proofs/theorems.rs:29-36`) has no writer; the program is refused at the erased data formal (C6). | Citation **covered**. Theorem statement and induction **needed** (R3): elaboration states the machine contract as a Π type; stage 06 proves premises at citations and descent under the ranking; the kernel's `IndexedFamily::ind` / `Term::IndW` (K `indexed.rs:510`, `term.rs:441`) supply induction; closure uses `judgment_assumption_closure` (K `signature.rs:185`). The shape filter and induction cache are incidental. |
| C9 | **Opaque source operations** (`open_term`): any expression with no kernel meaning became an uninterpreted constant. | `:3077-3294` | Dropped: "Unknown calculus identities reject" ([foundation.md#assumptions-and-calculus-identity](../../spec/proofs/foundation.md#assumptions-and-calculus-identity)); a machine is not implicitly a mathematical function ([mathematical_bindings.md#logical-hypotheses-and-machine-use](../../spec/proofs/mathematical_bindings.md#logical-hypotheses-and-machine-use)) | S05 refuses, e.g. "mathematical premise term without explicit integer embedding" (`read/mathematics.rs:469-471`). | **obsolete**; main's refusal is correct. |
| C10 | **Quotients**. The elaborator refused them (`:10840-10844`, `:12269-12279`); formation, lift correspondence, theorem and transport schemas lived in `validation/src/quotients.rs` (8,175 lines: `equivalence_selection` `:5`, `formation_collection` `:515`, `relation_plan` `:916-7901`, `request_admission` `:7902`) | see left | [quotients.md#formation-and-representation](../../spec/proofs/quotients.md#formation-and-representation), [#lifting-operations](../../spec/proofs/quotients.md#lifting-operations), [#quotient-correspondence-judgment](../../spec/proofs/quotients.md#quotient-correspondence-judgment) | S05 refuses "quotient data" (`read/places.rs:1819`), "recursive, open or quotient logical input" (`read/logical.rs:1018`) and generic data with a quotient (`read/subjects.rs:723`). Kernel `quotient_scheme` / `QuotientFamily` (K `quotient.rs:904-928`, `1033-1297`) have test callers only. | **needed**, part of R1 (a quotient formation is a declaration applying `QuotientFamily`) plus R3 (the correspondence judgment for `lift`/`define`). |
| C11 | **Classicality** | none in this file | [classicality.md](../../spec/proofs/classicality.md) audits the bounded rule set | `AcceptedProofRule::foundation` (`psi/semantics/proofs/bounded/classicality.rs:19`) | **covered**, outside elaboration. `classicality.md:10` cites a stale path. |
| C12 | **Plumbing for the old checker**: string declaration records, application scans, normalization, per-application domain matching, evidence aliases, term order, constant meanings | `:10-472`, `:2709-2805`, `:3923-4938`, `:5310-5520`, `:11633-11835`, `:12519-12674`; ≈2,900 | none | each value's evidence travels with the value in Terminal Psi | **obsolete** |

### The three general rules

| Rule | Owner | Covers | Kernel entry points it feeds |
| --- | --- | --- | --- |
| **R1** Declarations to a kernel signature: every typed mathematical declaration (definition, `boundary let`, Identity, quotient formation) becomes a closed `Declaration` with its level telescope; references become constants with level arguments. Publish one signature table in Terminal Psi. | elaboration, stage 05 | C1, C2, C3, C5, C10 formation | `Declaration::definition`/`::assumption` (K `signature.rs:16,24`), `instantiate_levels` (K `substitution.rs:28`), `infer_type`/`check_type` (K `typing.rs:237,3837`) |
| **R2** Families and their values: a typed `data` declaration becomes one indexed W description; a construction operation carries `sup`/`pack` evidence on the value it produces. | elaboration, stage 05 | C6, C7 | `indexed_scheme`, `IndexedFamily::{indexed_w, pack, sup}` (K `indexed.rs:429-500`) |
| **R3** Checking: stage 06 kernel-checks the published signature once, proves citation premises (already done for ordinary calls), checks theorem descent, and takes induction from `indW`; assumption closure comes from the kernel. | lowered-form checker, stage 06 | C8, C10 correspondence | `check_signature_with_attribution` (K `signature.rs:89`), `judgment_assumption_closure` (`:185`), `IndexedFamily::ind` (K `indexed.rs:510`) |

Terminal Psi already has the slots these rules fill: `mathematical_carriers`
and `identity_formations` (`psi/types/mod.rs:27-28`) and `indexed_theorems`
(`psi/proofs/declarations.rs:5`). The wire decoder reads all three
(`psi/artifact/wire/src/identity_formations.rs:51`,
`mathematical_proof_actual.rs:845-863`); nothing in `source-to-psi` writes
them.

## The deleted checker against stages 05 and 06

`checker.rs` had seven obligation kinds (`obligations.rs:3353-3361`), all "a
value lands in a bounded type at placement X": assignment, call argument,
initializer, state return, value, transition argument, guarded transition.
Each placement had its own collector (`obligations.rs:243-1130`), its own
checker (`checker.rs:285-846`) and its own diagnostic constructors
(`checker.rs:2979-3233`, 15 of them: five placements times integer, float and
named constraint). When it built no certificate it accepted
on an interval verdict with no kernel check (`checker.rs:373-395`, and
547, 623, 689, 762-790). Floats were never certified (`checker.rs:824-845`).
Stage 06 has no acceptance without the kernel: every decided obligation goes
through `verify_certificate` to `proof_admission::verify_obligation`
(stage 06 `src/lib.rs:2594-2603`).

S06 = `source-to-psi/pipeline/06_lowered-program-to-checked-program/`.

| Outcome | Old rule (lines, size) | Spec | New path | Label |
| --- | --- | --- | --- | --- |
| A value lands in a range-constrained type, per placement | collectors `obligations.rs:243-1130` (≈890), `bounded_checks` `checker.rs:285-846` (562), `named_constraints` `:5969-6252`, diagnostics `:2979-3233` | [domains.md#literals-and-expected-types](../../spec/language/domains.md#literals-and-expected-types) | S05 `read/predicates.rs` turns `T in D` into ordinary contracts on the value: a formal's `requires` (`CallRequirement`, `lower/calls.rs:1022`), a result's `ensures` (`NormalGuarantee`, `lower/control.rs:226`), a field's `DataDomain` (`default_obligations.rs:91`), `+D` as `ArrivalInvariant` (`lower.rs:1352`) | **subsumed** in stage 06. Stage 05 still refuses some placements the old collector served: an array whose elements carry a domain (S05 `read/arrays.rs:1105,1119`; old `collect_array_element_obligations`, `obligations.rs:938`), and domains on payloads and borrowed formals. Those belong to `CANONICALIZE-SCALAR-RANGE-CONTRACTS`, not to this task. |
| Integer ranges through add, subtract, multiply, cast, left shift, literal divisors | `integer_ranges` `checker.rs:4959-5561` (603), `certificate` `:847-2013` (1,167), `obligations.rs:1332-1833`, `:3655-3888` | [contracts.md#required-automatic-discharge](../../spec/proofs/contracts.md#required-automatic-discharge) (range propagation) | S06 `src/prove.rs` (`bound`/`search` `:3485-3720`, `product` `:4502`), `src/prove/ranges.rs` (`remainder_within_dividend` `:285`, `quotient_order` `:553`, `cast_bound` `:690`, `remainder_by_fixed_divisor` `:808`), `src/prove/widths.rs` | **covered** |
| Integer ranges through divide, remainder or right shift whose divisor or count is known only by an interval | the old code folded intervals without a certificate, so programs it accepted this way are refused today | the same floor: "An operand's bounds are the tightest literal bounds order reasoning gives it" | none: S06 README:1047-1052 ("would take endpoint propagation with the divisor's interval"); `prove/reach.rs:16-19` reports `NoRuleForGoalForm` | **needed**, one extension: a two-interval endpoint form for divide, remainder and shift in `IntegerAffineBound`, as multiply already has. Landing an operand on bounds proved equal (S06 README:1061-1072, `(prefix - 360) + last`, 15 programs) reads as the same floor clause. |
| Order reasoning (transitivity, ±literal, `a < b` ⇔ `a + 1 <= b`) | `apply_assignment_guard` `checker.rs:4608-4672`, `guarded_refold_certificate` `:1463-1688`, `requires_conditions::refine` `:6325-6358` | contracts.md floor, order reasoning | S06 `prove.rs`: `relation`/`steps` `:2812-3025`, `sum_order` `:2220`, `contradiction` `:1524`, `refute_guard` `:1672` | **covered**, except below |
| Contradiction between two order facts when neither is a taken guard (`low <= high`, `high < low`) | no separate rule | floor: "contradiction of an unreachable path" | none: S06 README:1039-1046 | **needed**, one certificate rule: irreflexivity of `<` on an open value (`a < a` proves false). The kernel's denotation already states the law (`psi/semantics/proofs/bounded/bounded_denotation.rs:12572`) but reaches it only when both sides are closed values (`:6608-6640`). |
| Float range derivation | `float_ranges` `checker.rs:3234-4139` (906); `obligations.rs:1489-1553`, `:1652-1669`, `:1753-1788`, `:3750-3848`; interval containment, never a certificate | float-to-integer conversion: [numeric_values.md#destination-owned-conversion](../../spec/language/numeric_values.md#destination-owned-conversion). The floor is integer-only. | conversion: S06 `prove/float_integer.rs:43`, `prove.rs:2063`; IEEE order: `prove/ieee_float_order.rs:145,293,361`; `FloatMeaning`: `prove/float_meaning.rs:20` | Conversion and order **covered**. Interval propagation through float arithmetic is **obsolete** as an automatic requirement. |
| Guard narrowing (if, match, transition conditions) | `guards` `checker.rs:4582-4958` (377), with syntactic expression matching `:4833-4908` and hoisting undo `:4757-4832`; `:5060-5103`, `:5309-5375`; case-guard payloads `obligations.rs:659-730` | [contracts.md#required-automatic-discharge](../../spec/proofs/contracts.md#required-automatic-discharge) (branch and pattern facts), [fact_validity.md](../../spec/terminal-psi/fact_validity.md) | S06 `lib.rs:1218-1245` writes a `BranchCondition` row on each edge, `:1247-1266` `DispatchBinding`; `known.rs` transports equalities; dominance from `graph.rs` | **covered**; syntactic matching **subsumed** (the operand is the same value) |
| Arrival stability (a fact still holds where it is used) | `arrival_stability` `checker.rs:12-127`, `assignment_stability` `:128-284`, `surviving_conditions` `:6263-6324`, `return_arrival` `:6435-6492`, `obligations.rs:452-546`; string-path overlap checks | [fact_validity.md](../../spec/terminal-psi/fact_validity.md) | facts attach to values and never go stale; field reads get read equations from `places.rs`/`places/log.rs` (`lib.rs:1193-1201`); domain clauses broken by a write are invariant windows | **subsumed** |
| Loop entry and state transitions | `obligations.rs:547-729`; `GuardedTransition` skipped (`checker.rs:6597-6599`); `:1168-1267` | [control_flow.md](../../spec/terminal-psi/control_flow.md) | S06 `arrivals.rs:51`, one `ArrivalInvariant` per edge (S05 `lower.rs:1352`), `rankings.rs` adds termination | **covered** |
| Joins | `incoming_state_guard` `obligations.rs:1167-1215` required syntactically equal guards (`:1241-1324`) | [fact_validity.md](../../spec/terminal-psi/fact_validity.md) | S06 `joins.rs` (`infer` `:371`, `check` `:1035`); case splits share a 64 budget (`prove.rs:84`) | **covered** |
| Call contracts, `old()` | no callee-`requires` obligation in this crate; caller premises `checker.rs:6253-6434` | [calls_and_outcomes.md#call-contracts](../../spec/terminal-psi/calls_and_outcomes.md#call-contracts) | `CallRequirement` (S05 `lower/calls.rs:1022`), `substitute_goal` (S06 `lib.rs:1803`), `call_contracts.rs`; `old()` via S05 `read/contracts.rs:1702` | **covered** (more than the old crate did) |
| Certificate synthesis | `certificate` `checker.rs:847-2013`; emitted five rule kinds, accepted the rest without a certificate | [contracts.md#required-automatic-discharge](../../spec/proofs/contracts.md#required-automatic-discharge) (evidence), [stage_trust.md](stage_trust.md) | S06 `prove.rs` → `lib.rs:2566-2610` → `verify_obligation` | **covered** and stricter |
| Proof plans | `obligations.rs` collection `:10-1331` (`build_proof_plan` `:42-195`), plan `:3105-3654` | [check_on_lowered_form.md#one-meaning-per-obligation](check_on_lowered_form.md#one-meaning-per-obligation) | S05 mints rows into the lowered tables; S06 `minted.rs` recomputes the owed rows and refuses a table that differs | **subsumed** |
| Derivation store and cache keys | `derivation_store.rs` (299), `checker.rs:2857-2978`, fingerprints `obligations.rs:1834-3104`; production passed `None` (`checker.rs:6564-6569`) | [check_on_lowered_form.md#the-library-is-a-dependency-not-a-subject](check_on_lowered_form.md#the-library-is-a-dependency-not-a-subject) | S06 `reuse.rs` re-verifies carried certificates for generic specializations (`lib.rs:801-803`) | **obsolete** (the old cache ran only in tests) |
| Plan metrics, `lemmas.rs`, `boundary.rs` | `measurement` `checker.rs:5562-5968`; `lemmas.rs`, `boundary.rs` had no callers | none | index bounds are `OperationSafety` goals (S05 `lower/places.rs:1924`) | **obsolete** |

The old checker never called `mathematical_declarations` (only `pub mod` at
`proof/src/lib.rs:37`); theorem citations reached the kernel through old stage
04/05 and the review-evidence crate. So nothing in `checker.rs` is needed for
R1-R3.

## How a clause becomes a proof term today

Stage 06 treats every value alike, but the path that brings a source clause
to it still has one piece of code per clause position and per kind of
operand. R1-R3 would add to that path, so its shape decides how much each
rule costs. Read at `b8213143d790`; S05 and K are the paths defined above.

| Step | Owner | What it produces |
| --- | --- | --- |
| Read a clause | stage 05: `read/contracts.rs`, `read/logical.rs`, `read/mathematics.rs`, `read/predicates.rs`, `read/clause_calls.rs` (6,705 lines) | a `Proposition` over `ScalarTerm` and `IntegerMathTerm` (`foundation/semantic-records/src/proposition.rs`, `scalar_terms.rs`) |
| Attach it | stage 05 `lower/` | obligation and knowledge rows in the [lowered program](../../../source-rust/subsystems/compiler/source-to-psi/representations/lowered-program/README.md) |
| Prove it | stage 06 `prove.rs` | a `ProofNode` certificate built from 33 rules (`psi/proofs/certificates.rs`, `ProofRule`) |
| Check the proof | bounded admission, `psi/semantics/proofs/bounded/` | each rule is checked, then the certificate is translated into the mathematical kernel's `Term` and judged again (`bounded_denotation.rs`) |

Five term types take part: `ScalarTerm` (runtime scalars and storage reads),
`IntegerMathTerm` (unbounded integers), `Proposition`, `ProofTerm` (erased
values) and the kernel's `Term` (Π, Σ, `Id`, W, `Two`, `Empty`). Only the
last can state a type, and no source stage writes it.

What the code shows:

- **Thirteen term readers.** Thirteen functions in four files return a
  `ScalarTerm` or `IntegerMathTerm` read from a source expression:
  `term`, `read_term`, `old_term`, `clause_cast`, `arithmetic_term`,
  `mathematical_term`, `read_mathematical_term` and `field_term` in
  `read/contracts.rs`; `input_term` in `read/logical.rs`;
  `mathematical_integer_literal`, `integer_embedding` and
  `logical_mathematical_term` in `read/mathematics.rs`; `called_term` in
  `read/clause_calls.rs`. Two more read an erased term
  (`read/expressions.rs:272`, `lower/operations.rs:123`).
- **The clause's position chooses the reader.** `ClauseScope`
  (`read/contracts.rs:34`) records where a clause sits, and the readers
  branch on it: `read_mathematical_term` hands the whole expression to
  `logical_mathematical_term` when `scope.logical` is set
  (`read/contracts.rs:2062`), and `identity_inputs` refuses Identity unless
  the scope is a machine entry or return clause (`:750-752`). Each reader
  then refuses what its position does not serve. Stage 05 has 1,252 refusal sites outside
  its tests, and about 57 of their messages name a position, for example
  "call in a clause of a state other than the entry", "embed outside a
  mathematical comparison", "old outside a callable contract", "formal field
  in a postcondition", "Identity outside a machine entry or return clause",
  "term other than a formal, a literal or an embedded integer" and "term
  other than a field of the value or a literal".
- **The kind of operand chooses the proposition.** `Proposition` has 22
  variants. Equality alone has five: `Equal` (scalars), `IntegerMathEqual`,
  `ByteSequenceEqual`, `StructuralIdentity` and `FloatMeaningEqual`. None
  takes a type, so each type Identity serves needs its own variant, reader
  and prover rule. `identity_clause` (`read/contracts.rs:906`) serves two.
- **A call in a clause is unfolded or refused.** `read/clause_calls.rs`
  serves a call only to a closed machine with no contract, scalar parameters
  and a scalar result, in a clause of the entry state, by reading the
  callee's body in place of the call. A machine with no body cannot be
  unfolded.
- **`ProofTerm` names types by text.** Five of its variants and
  `MathematicalArgument` hold a `type_identity: String`
  (`proposition.rs:1466-1500`), against the lowered program's rule that no
  decision is keyed on text.
- **Some certificates are accepted without the mathematical kernel judging
  them.** `bounded_denotation.rs` names all 33 rules, but nine sites return
  `Unsupported` for a shape it cannot translate (for example "a carrier bound
  of an integer whose width is a parameter"). Admission then records
  `MathematicalCoreDecision::Refused` and still accepts the certificate on
  the bounded rule (`bounded/proof.rs:257-259`). Nothing in `source-to-psi`
  reads that decision, so nobody counts these.
- **Terminal Psi has one slot per construct.** `mathematical_carriers`,
  `identity_formations` and `indexed_theorems` each carry one kind of
  declaration, and none has a writer.

### What to change in this layer

Each change removes one axis of the combinations above. They are proposals,
checked against the code but not built.

| # | Change | What it removes | Measure |
| --- | --- | --- | --- |
| L1 | **One term reader in stage 05.** One function reads an expression into a term at every position. `ClauseScope` supplies only what each name denotes there (a formal, the entry snapshot for `old`, the result, a field of the value being built). | the position axis: a term form served once is served in every clause, domain predicate, definition body and erased argument | readers per term form: 1; refusals that name a position: 0 |
| L2 | **Two general term forms.** Add an application of a published declaration to argument terms, and `Id` at a type. The existing scalar, integer and float forms stay as the cases the arithmetic rules work on. | the operand axis: Identity for every type (C5), a call to a machine with no body, a definition with binders (C1) and a quotient relation (C10) become the same two forms, with one reader, one prover route (reflexivity, symmetry, transitivity, congruence) and one translation to the kernel | equality forms a new type needs: 0 |
| L3 | **One signature table in Terminal Psi** in place of the three per-construct slots, written by stage 05 and checked once by stage 06 with `check_signature`. This answers open question 3. | a table and a wire reader per construct | writers per declaration kind: 1 |
| L4 | **Stage 06 reads `MathematicalCoreDecision`** and reports how many accepted certificates the mathematical kernel did not judge, per obligation kind. Close the nine shapes or refuse them. | a second basis of trust that nobody measures | accepted without the kernel's judgment: 0 |
| L5 | **Typed identities in `ProofTerm`** in place of `type_identity: String`. | decisions keyed on text | text keys in `ProofTerm`: 0 |

L1 and L2 replace R1's separate routes for definitions, Identity and
quotient formation. Build the single reader for the two new forms first, so
they work at every position from the start, then move each existing form
into it and delete the reader it came from in the same change. That avoids
both rewriting 6,705 working lines at once and adding the new forms to
thirteen readers.

## Who needs it today

### Library declarations

| Declaration | Uses | Blocked by |
| --- | --- | --- |
| `core/extent.omg:228` `no_wrap(base: addr, length: u64): core\Strict<0>`, used by `Extent::Granted` (`:235-237`) and `alloc/bump/bump.omg:16` | closed, scalar formals, `Strict<0>` result | nothing in this task: stage 05 serves it as a `Proposition` (C4). Stage 06 test `src/tests/address_bounds.rs:290-319` accepts a call whose caller proves the bound and refuses it without. Its sample consumers fail earlier, see below. |
| `core/extent.omg:219` `core\Identity::<IntervalSet<Nat>>(...)` in `transfer`'s `ensures` | Identity over data | C5 ("Identity of an unserved type") |
| `core/algebra.omg:29-41` `CommutativeSemiring<T>` laws, `ensures core\Identity::<T>(...)` | Identity over a generic T; resultless theorem slots | C5, C8 |
| `core/cauchy.omg` (28 uses), `nat.omg` (46), `nat_metric.omg` (32), `rat.omg` (28), `int.omg` (19), `math.omg` (14), `seq.omg` (12), `float_operations.omg` (78, mostly `FloatMeaning`) | `core\Identity`/`core\Type`/`core\Strict` over `Nat`, `Rat` and generic sequences, in `requires`/`ensures` | C5 for non-scalar T; C8 where a machine is a theorem |

No `[erased]` binding and no other top-level mathematical `let` appears in
`source/library/`. Nor does `core\Level` or `core\Squash`, so the library
needs neither level binders (C2) nor the Squash mapping (C3).

The algebra laws need one thing no capability row above names: a call used
as a term. `ensures core\Identity::<T>(add(a, b), add(b, a))` applies `add`,
a trait machine that has no body and works on a generic `T`.
`read/clause_calls.rs` can only unfold a closed machine with scalar
parameters and a scalar result (`:189-314`), so stage 05 could not read these
operands even if Identity served `T`. The spec already says when a call may
stand as a term: "A call interpreted denotationally needs a checked
observation-free, reach-free, crash-free, terminating invocation"
([contracts.md#total-arithmetic](../../spec/proofs/contracts.md#total-arithmetic)).
It also names call congruence for comparisons: repeated pure comparisons with
the same operation and unchanged arguments have the same Boolean denotation
([contracts.md#equality-bridges-and-comparison-facts](../../spec/proofs/contracts.md#equality-bridges-and-comparison-facts)).
The [proofs lane plan](proofs_evidence_lane.md) lists this as slice 10;
change L2 above serves it and Identity with the same two forms.

### Samples

77 `.omg` files in six sample families declare a top-level mathematical `let`
(`domain_geometry`, `negated_mathematical_order`, `representation_agreement`,
`source_indexed_family`, `probes/declaration_predicate_calls`,
`probes/denotation_lets`). No sample cites `core\Identity`, `core\Squash` or
`core\Level` outside `representation_agreement`. The run added the families
that need erased family evidence, theorem citations or core's extent
(`source_indexed_binary_tree`, `source_indexed_type_parameter`,
`erased_reference_contract`, `induction_call_guarantee`, `resident_backing` and
seven probes): 176 samples, 167 in nine fact families plus nine probes.

Result: 0 pass, 174 fail, 2 are not runnable on `linux_arm64` (they bind other
targets). Grouped by the first error each sample reports:

| Samples | First error | Emitted by | Belongs to |
| --- | --- | --- | --- |
| 53 | "this value's type carries no domain and ... declares `Region::Bounded`/`Wrapping`" (35) and "builtin operator `-` takes unqualified operands only" (18): `domain_geometry` 17, `negated_mathematical_order` 18, `induction_call_guarantee` 18 | stage 03/05 domain typing | the samples predate [domains.md](../../spec/language/domains.md#composition-and-removal); domain surface work, not this task |
| 48 | "declaration using a type written as an expression, which no pass evaluates yet": every `source_indexed_*` sample except `source_indexed_type_parameter`'s invalid twins | `typed-trees/proofs/deferred_refusals.rs:168` | **this task**: value-indexed type arguments (`Vector<previous, previous_total>`) are R2's front end |
| 18 | "`u` is spelled like a builtin type but is not one": `representation_agreement`, at `core\Type<u>` | S03 `type_reference.rs:3821` | **this task**: C2 level binders; see open question 1 |
| 18 | "import `omega::language::core::Access` does not select one declaration": `resident_backing` | stage 02 imports | library/imports, not this task |
| 10 | "parameter erased reference parameter" (9) and a call to that refused machine (1): `erased_reference_contract` programs | stage 05 | references lane |
| 9 | "selected target `linux_arm64` has no bound required root slot": `erased_reference_contract` invalid twins | Build | target binding of the samples |
| 9 | "intended-invalid sample requires diagnostic expectations": `source_indexed_type_parameter` invalid twins | `sample-gate` | sample hygiene |
| 4 | "a data declaration cannot own a machine": `denotation_lets`, `erased_member_proven`, `extent_no_wrap_denotation`, `extent_qualifier_field` | S05 `read/machine.rs:1500` | stale sample syntax |
| 2 | "type addr literal" in a constant initializer: `domain_geometry/const_name` and its twin | stage 05 constants | constants, not this task |
| 1 | "type `Nat` is not Equatable": `axiom_cited_lemma` | S03 `equality_preparation.rs:436` | library `Nat`; meets C5 next |
| 1 | "runtime call callee has no complete declared value type": `erased_witness` | stage 03/05 | unclassified |
| 1 | killed by signal 9 during `update`: `erased_array_evidence` | — | not investigated |
| 2 | not runnable on `linux_arm64`: `declaration_predicate_calls`, `device_extent_access` | — | unmeasured here |

None of the 174 failures stops at a C1-C8 refusal site in stage 05. Fixing
the two front-end groups owned by this task moves 66 samples to their next
failure, not to passing. The samples therefore cannot show when the proof
work is done; the core library can.

## What else was dropped

PR #2823's body says "Proof admission, terminal installation, canonical
artifact execution/value transport, portable review records and their
encodings remain". For proof admission that holds:

- #2823 changed nothing under `omega-rust/psi/compilation/proof/admission/`;
  its proof-crate diff touches only `proof/Cargo.toml`, `checker.rs`,
  `derivation_store.rs`, `lib.rs`, `mathematical_declarations.rs` and
  `obligations.rs`.
- Every admission integration test at `cea9cdcde460^` exists today under
  `psi/semantics/proofs/bounded/tests/` with the same or a larger `#[test]`
  count (e.g. `indexed_*` 2/2/1/1/3/1, `theorem_certificate_admission` 5/5,
  `value_equality_transport` 9 → 10, `quotient_at_most_dividend` 5 → 6).
- The old admission's `mathematical_core.rs` + `mathematical_typing.rs`
  (9,639 + 3,675 lines, 100 tests) is now the `mathematical/` crate
  (14,447 lines, 119 tests).
- Later PRs removed kernel code on purpose: #2943 (`30871086139b`) dropped the
  uncalled `SourceIntegerTheory` search (1,177 lines of `bounded_denotation.rs`);
  nine closed-evaluation tests left `integer_rules.rs` and now live in
  `psi/semantics/closed-predicates/integer.rs:328-359`.

What #2823 did drop that the kernel needs is the **producer side**:

- the only source-to-kernel tests of elaboration (the 1,442 test lines in
  `mathematical_declarations.rs`, including the indexed-vector budget tests at
  `:9847-10330`);
- quotient formation and correspondence checking (`validation/src/quotients.rs`);
- `checked_trees` `CheckedMathematicalDeclaration`/`Signature`, which carried
  the elaborated signature to its consumers. Terminal Psi's
  `indexed_theorems`, `identity_formations` and `mathematical_carriers` slots
  are its intended replacement and have no writer.

## Plan order

Each step is one general mechanism with an end-to-end customer and a
must-reject twin, per [the gate](check_on_lowered_form.md#the-gate). The
[proofs lane plan](proofs_evidence_lane.md) lists finer slices; where a slice
there adds a case per construct, take the rule here instead.

| Step | Build | Owner | First customers |
| --- | --- | --- | --- |
| 1 | L1 and L2 together: the single term reader, serving an application of a declaration and `Id` at a type in every clause position. Stage 06 gets the equality route for them (reflexivity, symmetry, transitivity, congruence). | stage 05, lowered program, stage 06, kernel translation | `core/algebra.omg`: a caller that cites `add_comm` and relies on the Identity, and a twin that relies on it without the citation |
| 2 | L3 with R1: definitions with binders and `boundary let` become declarations in the one published signature; stage 06 checks it once | stage 05, Terminal Psi, stage 06 | `cauchy.omg`, `nat.omg`, `rat.omg`, `extent.omg:219` |
| 3 | Move each existing term form into the single reader and delete the reader it came from | stage 05 | each position refusal listed [above](#how-a-clause-becomes-a-proof-term-today), with its twin |
| 4 | R2: `data` to indexed W description; construction operations carry `sup`/`pack` evidence on their value. Needs value-indexed type arguments (`Vector<previous, previous_total>`) evaluated in stage 03 first. | stages 03, 05 | the first library declaration that needs a family, else `source_indexed_*` once their other blockers are repaired |
| 5 | R3: theorem machines as Π statements, induction through `indW`, assumption closure from the kernel | stage 06 | `source_indexed_family` (`vector_bound`), `induction_call_guarantee` |
| 6 | Quotient formation (L2, L3) and the correspondence judgment (R3) | stages 05, 06 | none measured |
| — | In parallel, each independent: (a) divide/remainder/shift endpoints from an interval divisor in `IntegerAffineBound`; (b) irreflexivity of `<` on an open value; (c) L4; (d) L5; (e) `core\Level` binders in stages 02-03, once open question 1 is answered | stage 06, kernel, stages 02-03 | S06 README:1039-1052 programs; `representation_agreement` for (e) |

The first version of this plan started with level binders and value-indexed
type arguments because they move 66 samples. They move those samples to
their next failure, the library needs neither, and level binders wait on
open question 1, so they no longer lead. The samples' other blockers (domain
surface, imports, stale syntax, references) belong to their own `TASKS.md`
items.

## Open questions

Recorded, not answered:

1. **Level binder spelling.** [modules.md#builtin-type-names](../../spec/language/modules.md#builtin-type-names)
   reserves `u` as an integer-family type spelling and lists which introduced
   names may use one, but not a `core\Level` binder. The spec's own examples
   name levels `u`, `v`, `w` ([mathematical_bindings.md](../../spec/proofs/mathematical_bindings.md#universes-and-mathematical-function-types)).
   Is a level argument a type position, and may a level binder be named `u`?
   The library declares no level binder, so this blocks only
   `representation_agreement`, not steps 1-3.
2. **One mechanism for Identity over data.** The spec forms `core\Identity`
   with the kernel's `Id` rules ([mathematical_bindings.md#scope](../../spec/proofs/mathematical_bindings.md#scope));
   the [proofs lane](proofs_evidence_lane.md) slice 7 expands it into leaf
   equalities over values. Which one is the route, and does the other remain?
   This audit's reading: `Id` is the route, because a generic `T`
   (`core/algebra.omg`) has no fields to expand; leaf equalities would then
   be a consequence the kernel proves for a concrete record, not a second
   route.
3. **Where the signature lives.** Terminal Psi has `mathematical_carriers`,
   `identity_formations` and `indexed_theorems` slots with readers and no
   writer. Is R1's signature table those slots, or a new table? The spec
   requires publishing "the full level telescope and constraints"; no level
   constraint encoding was found. L3 proposes one table for every
   declaration kind.
4. **Floor scope.** Does "the tightest literal bounds order reasoning gives
   it" ([contracts.md#required-automatic-discharge](../../spec/proofs/contracts.md#required-automatic-discharge))
   require landing an operand on bounds proved equal (`(prefix - 360) + last`,
   S06 README:1061-1072)? Does the 64 limit count arm combinations, as the
   spec says, or branches, as `prove.rs:84` does?
5. **Float ranges.** The floor is integer-only. Confirm that no page requires
   automatic interval propagation through float arithmetic, which the old
   checker did without certificates.

## Validation

- Old code read with `git show cea9cdcde460^:<path>`; new code at
  `origin/main` `6bab34687545`. Citations were checked against those files;
  the general-versus-incidental line split in the elaborator is approximate.
- Sample run: `mbx xtask vm start omega-linux-arm64 proof-kernel-audit`
  running `mbx xtask sample-gate --filter <18 family fragments> --jobs 6
  --out build/proof-audit.json` at `6bab34687545` (2026-11-23). Exit 1:
  174 failed, 2 not runnable on `linux_arm64`, 0 passed. Grouping is by the
  first `error:` line of each result.
- Second reading (2026-11-23), old files again from `cea9cdcde460^` and new
  code at `b8213143d790`, with `grep` and `wc` only; nothing was built or
  run. It corrected the diagnostic count (15, not 13), the `check_signature`
  callers and the `validation::` breakdown, and added the layer section. The
  1,252 refusal sites are lines outside test files matching
  `Unserved::<Variant>(` or `unserved(`; the 57 that name a position are a
  keyword count over their messages and are approximate.
- Not determined: whether a full core library check with `no_wrap` passes
  end to end (no sample reaches it); `erased_array_evidence`'s signal 9;
  whether old level constraints were ever published; the two samples bound to
  other targets; which samples compiled before #2823; whether the old checker
  accepted the 15 `(prefix - 360) + last` programs; whether any program in
  use reaches one of the nine untranslated certificate shapes; whether the
  33 certificate rules overlap.
