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
   non-scalar types or quotients. It also deleted the code that proved
   Identity laws, `structural_judgment` in `validation/src/contract_entailment.rs`
   (37,507 lines), which never called the kernel.
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
   Step 5 has since put the term readers into one, `clause_term`
   ([Step 5 as built](#plan-order)).
6. **The deleted corpus was 38% one matrix.** Of its 16,759 fixtures, 6,438
   were 379 features each written through the same nine placements, accepted
   and rejected. The old compiler had one rule per placement and so needed
   one test per placement. The corpus also gives the full list of forms
   programs write in a clause, which is what the single reader must serve.
   See [the deleted corpus](#the-deleted-corpus).
7. **Every deleted line has been read, and the deletion holds up.** 137
   readers covered all 875,311 lines of the deletion's slices, and their
   reports were merged and checked against today's compiler. The kernel
   judged nothing outside the proof crate. The read restates the three
   properties, names twelve patterns the earlier readings missed, lists 1,366
   old rules with what today's compiler does about each, and finds ten of the
   old faults still present today in small form. Probe programs then tested
   eight of the rules the readers could not find. Three were real gaps,
   since repaired on this branch: a machine that issues no authority could
   rewrite the `base` and `length` of an `Extent in Granted` and keep
   `Granted`; a process-exit provider could change the exit status; and two
   parameters of one machine could share a name. See
   [what the line-by-line read found](#what-the-line-by-line-read-found).
8. **Do first.** Build toward the core library, not the samples. It needs no
   level binder, no `core\Squash` and no value-indexed family, but every law
   in `core/nat.omg` is an induction over the recursive `data Nat`. Give
   stage 05 one term reader and the term forms `Proposition` lacks
   (application of a declaration, `Id` at a type, constructor, projection),
   proved first on a law cited over an abstract type. Then make a plain `data` a kernel type and a
   law machine an induction, with the deleted fixture
   `pass/proofs/ring_law_conformance` and its rejected twin as the customer.
   See [plan order](#plan-order).

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
| `validation/src/contract_entailment.rs` | 92,926 | #2823; see [the whole deletion](#the-whole-deletion) |
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
`mathematical_proof_actual.rs:845-863`). Only the carrier table has writers
in `source-to-psi`: theorem parameters in stage 05 and data declarations in
stage 06 (step 2 below).

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

## The whole deletion

#2823 deleted 861,818 lines, and the first versions of this audit had read
about 3% of them. This section covers all of it. Two crates are 79% of the
deletion: the old checking stage (`05_typed-trees-to-checked-trees`, 343,475
lines) and `validation` (336,630 lines). Across those two crates the proof
kernel is referenced on 13 lines. A program was accepted when no check
pushed an error, and each check was the old compiler's own yes or no.
`validation/src/lib.rs:6-7` said so: "Successful validation is a source
judgment, not a portable proof certificate."

Three properties hold in every part. Together they are the case for the
deletion, and the list of things the rebuild must not repeat. They are
stated here as the first pass found them;
[the line-by-line read](#what-the-line-by-line-read-found) restates them.

1. **The verdict was the code's own judgment.** The two crates have about 640
   functions that return `bool`. The contract prover returned
   `Proven`, `Refuted` or `Unknown` from its own rewriting. `ProofNode` and
   `verify_obligation` appear nowhere in either crate.
2. **Text identified values.** Arithmetic atoms were strings such as
   `format!("\0call-result:{expression:?}")`. Structural terms named a
   parameter as `Variable(String)` and anything else as `Opaque(String)`,
   and two `Opaque` terms were equal when their display text was equal
   (`contract_entailment.rs:81552-81556`). Index facts were pairs of labels
   from `display_name`, which `checks_ranges.rs` calls 85 times. Names
   decided meaning: a rank proof looked for a `data` named `"Nat"` with cases
   `"Succ"` and `"Zero"` (`contract_entailment.rs:23838, 23898`).
3. **One rule was written once per placement.** The same delivery checks
   were called separately for an assignment, a local initializer, a return
   value, a call argument and a construction field. Scalar plans carried a
   role with 31 placement variants. `contract_entailment.rs` had three
   expression-to-term builders (`structural_term`, `callee_term_leaf`,
   `bound_contract_term`) and twelve visitors that each asked one question
   of the same walk. Today's stage 05 has thirteen term readers; it is the
   same fault.

### By part

"First pass" is the share the first pass read in full, when every module was
at least outlined (name, signatures, leading comment) and counted by search.
Every line has been read since.

| Part | Lines | What it did | How it decided | First pass |
| --- | --- | --- | --- | --- |
| `validation/src/contract_entailment.rs` | 92,926 | An integer prover over polynomials (`Engine`), a judge over structural terms (`StructuralJudge`), and about forty client modules that each put one kind of question to them: call requirements, citations, induction, law conformance, rank descent, content conservation | `Proven` / `Refuted` / `Unknown`; no kernel | about 60% |
| Old stage: contract checks (`checks_contracts*.rs`) | 37,588 | Each call's `requires` and each exit's `ensures` | A clause passed when any one of an ordered list of routines returned true: 24 route call sites for a call's `requires` and about 40 leaf procedures beneath them. Many compared rendered text; `checks_contracts_calls.rs:2005` says "Contract comparison is currently label-based" | 36% |
| Old stage: ranges and termination | 29,415 | Index and subslice bounds; rank descent; termination summaries | Label-keyed fact tables; three tiers of rank proof | about 40% |
| `validation/src/arithmetic_domains.rs` | 27,042 | Interval analysis: overflow, division, shift, cast, narrowing | Intervals in a map keyed by place-path text; an unknown bound produced no diagnostic (`:795`) | 22% |
| `validation`: proof contracts, quotients, float and mathematical contracts, domains | 39,724 | Proposition `ensures`, erased bindings, `embed`, quotient formation and `lift`/`define`, domain membership, invariant windows on `data ... requires` | Sets of rendered labels; structural matching of theorem shapes | 29% to 72% by file |
| Rest of `validation` | about 177,000 | Calls and write frames (`calls.rs`, 45,408), custody of references, places, traits, recasts, effects | 895 `Diagnostic::error` sites, 888 `bool` functions | 5% |
| Rest of the old stage | 275,560 | Facts in force at each point, borrows, ownership counting, and executable plans for lowering | Diagnostics; one output record, `CheckFacts`, with 55 fields | outline and samples |
| `proof/src` | 24,187 | The mathematical elaborator and range checker [above](#the-deleted-elaborator-by-capability) | The only part that called the kernel | read |
| Build-time evaluation, `checked-trees`, stage 04, review evidence, provider planning, stages 07 and 08 | about 145,000 | Compile-time evaluation, the old checker's output format, preparation, package review records | Consumers of the old checker's output | outline and samples |

The first pass read about 130,000 lines in full, 15% of the deletion. Its
low-coverage rows were ownership, borrowing, layout and lowering.
[The line-by-line read](#what-the-line-by-line-read-found) covered all of
it.

### What the old proof code could do

These are capabilities, not code to restore. Each is stated with where it
lived and what it corresponds to in the kernel, because the kernel already
has a term for each.

| Capability | Where it lived | How it worked | In the kernel |
| --- | --- | --- | --- |
| A called machine's body as a term | `unfold_application`, `contract_entailment.rs:83747-84007` | One state, matching arity, no active identical application; walked statements, tried transitions in order, aborted on a guard it could not decide; depth 32 | a definition and conversion |
| Case split on a `data` value | `judge_case` (`:48739`), `match_paths` (`:62851`) | One path per arm, assuming the arm's case; excluding every other case proved the last | the W eliminator |
| Induction | driver `:92309-92577`, `inductive_judgment` `:16631-17140` | Per arm, each call of the machine on itself whose `requires` held gave its `ensures` as a hypothesis. Descent was checked by a separate recursion check | `IndW` |
| Citing a lemma | `citations` `:9517-10381` | The lemma's `ensures` was instantiated at the call's arguments after its `requires` were judged there. It was trusted because the lemma was checked "in this same validation batch" (`:9838-9842`) | application of a proved declaration |
| Equality of `data` values | `judge_equation` (`:85192`), `data_identity` (`:49677`) | Same constructor compared field by field; different constructors refuted; then rewriting through established equalities, at most 64 forms a side | `Id`, `IdElim`, constructor injectivity |
| A satisfier proves its trait's law | `law_conformance` `:17141-19928` | One-way pattern match of the satisfier's proved `ensures` against the law, with the trait's operation names replaced by the satisfier's machines; "compiling means proven" (`:19670`) | the satisfier's proof has the law's type |
| Rearranging by algebra laws | `algebraic_identity` `:47928-48249`, `licenses` `:57471-59048` | A machine earned a license when its trait's `ensures` had the shape of commutativity, associativity or distributivity; goals were then compared as polynomials of at most 64 monomials | rewriting with the laws as equalities |
| Quotients | `validation/src/quotients.rs` | Formation needed a sealed `Equivalence` conformance; `lift` and `define` needed a theorem machine whose contract had the expected congruence shape, matched structurally | `QuotientFamily` |
| Integer goals | `Engine`, `:6013-7678` | Polynomial normal form, substitution of hypothesis equalities, difference bounds, intervals for division, shifts and bitwise results | stage 06's prover and its certificates |

The judge's term type, `StructuralTerm` (`:80504-80601`), had 20 variants.
Four of them are the general forms that today's `Proposition` lacks:
`Identity { carrier, left, right }`, `Application { target, arguments }`,
`Constructor` and `Projection`. Real programs needed all four, so L2 below
takes all four.

One route in the old compiler did reach the kernel. For a theorem machine
with erased family evidence, the elaborator built a kernel term, and
`validation` accepted the machine's `ensures` when that row existed
(`float_and_mathematical_contracts.rs:2372-2389`). The old output format
carried what the kernel needs: `CheckedMathematicalSignature { arena,
declarations }`, `CheckedIndexedTheorem { term, ty, assumption_closure }`
and `CheckedIdentityFormation`. Today's three empty Terminal Psi slots are
their successors. Laws over plain `data`, such as every law in
`core/nat.omg`, did not take that route; `StructuralJudge` decided them.

### What accepted without proof

The first pass recorded more than a hundred sites; these are the kinds, with
one example each. None was traced to a wrong acceptance of a real program.
The 17 syntheses of the line-by-line read list about 1,150 suspected defects
in the old code. About 520 of those rows name a possible wrong acceptance,
and the synthesis authors mark about 140 rows as checked in the source.
These are counts of table rows, and no program was run against any of them.

| Kind | Example |
| --- | --- |
| A clause deferred as an obligation and the call recorded as decided | `checks_contracts_calls.rs:3447-3514` |
| A declared type trusted after the fact about it was invalidated | `checks_contracts_calls.rs:3728-3783` |
| An `ensures` that is structurally equal to a `requires` recorded as proven | `contract_entailment.rs:92599-92604` |
| A failed lookup that skips the check | `validate_loan_handbacks`, `contract_entailment.rs:21657-21750` |
| A narrowing store reported only when both interval bounds are known | `arithmetic_domains.rs:795` |
| An unknown goal that yields neither a proof nor a diagnostic | `contract_entailment.rs:92819-92833` |

### What the line-by-line read found

Every deleted line has now been read, and the read supports the deletion.
137 readers took the 875,311 lines of the deleted files and of the diff of
the files #2823 changed in part. A script compared each reader's logged
ranges with its slice. The reports were merged into 17 group syntheses, and
a second pass looked up each group's rules in today's compiler. All of it is
in [the full read](../reference/deleted_checker_full_read/README.md).

The plan's steps stand. The read changes this audit in four ways: the three
properties need restating, it shows twelve patterns no earlier reading
named, it gives a list of what today's compiler no longer does, and it finds
several of the old faults in today's compiler in small form.

#### The three properties, restated

| Property | What the full read found |
| --- | --- |
| The verdict was the code's own judgment | Holds in all 17 groups. The kernel judged something in two places: the proof crate, and one call from package review into the proof crate's elaborator (`contracts/mathematical_calls.rs:24-45`). The contract prover's 92,926 lines contain no kernel call. Inside the proof crate the range checker did call `verify_obligation`, but on a fresh atom under assumptions the checker chose itself, with 5 of the kernel's 28 rule variants, and it accepted on its own interval when it built no certificate (`checker.rs:884-965, 5595-5605`). Only the elaborator compared the assumptions of what it proved with an audited list (`mathematical_declarations.rs:5314-5396`). |
| Text identified values | There were three kinds of identity. Text decided wherever a proof-like judgment was needed: range facts, contract labels, write frames, every arithmetic atom, callback refinement, proposition entailment, quotient facts. Handles identified ownership claims, places, loans and effects. The third kind was not named before and did as much harm as text: **source position**. A fact belonged to a (state, statement index, call ordinal) triple or to the expression that produced it, and a memory address stood for identity at more than forty sites. Facts therefore had to be copied when a value moved and found again at each use. |
| One rule was written once per placement | Holds, on a different axis. About a fifth of the lines are second or later copies: about 60,000 of the 304,000 lines in the six groups whose readers gave totals. Copies per local, field, parameter and return are the minority (ownership events, 12 copies; the domain-on-entry rule, 10; the scalar roles). Most copies follow the question asked, the form of the call (statement, expression, transition), where the value came from, or the kind of owner (machine, declaration, specification, proof). The corpus matrix measured placement because placement is what a test author can vary. |

#### Patterns the earlier readings missed

The left column is the pattern, the middle column is the evidence, and the
right column is what the rebuilt proof layer takes from it. Counts are the
readers' and are lower bounds. File names are the deleted files'; the
syntheses give the full paths.

| Pattern | Evidence | For the rebuild |
| --- | --- | --- |
| **One question had several engines, and they disagreed.** | The integer range of an expression was derived by five engines in the proof crate alone, and by seven interval comparers in the contract checks. A `requires` predicate had eight evaluators using `i64`, `i128`, big integers and big rationals. Sixteen functions read "a comparison in a contract or guard", each for a different set of operators; `or` was a union in two and ignored in two. "May this write touch this place" was written at least 16 times, path overlap 5 times, the type of an expression 9 times. The syntheses name more than thirty pairs of copies that give different answers. Which copy ran depended on the placement or on which check asked, so one fact was proved in one place and unknown in another. | L1 has to end with one reader, not one more. Step 5 deletes the others, and a ratchet in `tools/ratchets/` should count stage 05's clause readers so the number only falls. |
| **"Proved" meant something that is not a proof.** | "Running the check again added no diagnostic" stood for proved at 13 or more sites. So did "a row exists", "a flag was set by an assignment", "the stored diagnostic string is empty" and a test on the wording of a message. Missing evidence gave the permissive answer: a missing operator record meant builtin, zero rows meant accepted, a missing call record meant the call completes normally. Twenty verdicts were stored as a bare flag or site list that no consumer could check again. | A verdict is a certificate or a refusal. [Old faults that remain](#old-faults-that-remain-in-todays-compiler) lists where today's compiler still answers from an absence. |
| **A contradiction was trusted like a proof.** | Seven places turned the judge's own "these hypotheses contradict" flag into acceptance of everything on that path. The flag came from the judge's own comparison of constructors, integers and type-atom names. No reader found a diagnostic for a `requires` that can never hold. | A dead path is closed by a kernel proof of falsehood from the hypotheses, the same as any other goal. |
| **A callee's body decided the caller's proof.** | About twenty mechanisms read a callee's body where its contract was silent: a body that returns one Boolean constant, returned-field templates, unfolding, return-range inference, mutation summaries, a variant tag taken from the callee's returns, crash summaries, rank inference for private helpers. Other code inferred clauses nobody wrote: cleanup entry requirements, blocking and suspension interfaces, capability verbs. | The spec settles which of these are rules. [Local call summaries](../../spec/language/state_contracts.md#local-call-summaries) lets an exact call inside one verification unit use body-derived result and state facts, including place paths, versions and branch and case facts. Across a unit boundary, and for an abstract requirement or a boundary, only the published contract counts. A recursive group needs a checked invariant: "assuming a proposed summary around a cycle is not a proof", which is what the old coinductive summaries did. Inference "cannot invent domains, permissions, ownership authority". Today's compiler serves summaries for private scalar results only; the rest of the licensed form is missing work, not an open question. |
| **Assume and guarantee were split and trusted in a circle.** | A caller took a callee's declared `ensures` as a hypothesis because "exits check it". The exit check trusted declared return types because "stores check them". The store check lived in another crate. Law conformance read declared `ensures` ("compiling means proven"), and a citation relied on the lemma being checked "in this same validation batch". Nothing recorded what had been proved. | L3 is what closes the circle: a cited law is a signature entry that carries its proof term, and a use depends on the entry. |
| **Stages checked themselves by running again.** | More than sixty sites rebuilt a result with the same code and compared, and reported a difference to the user as "drifted" or "lost its ...". "Receipt", "commitment" and "replay" named a hash of the record's own fields. Such a check catches a corrupted record and never a wrong rule. In one group, 35 to 40 of about 320 rule rows were of this kind. | The kernel is the independent check. The proof layer should add no replay of its own producer. |
| **Most refusals were silent.** | About 33,000 lines in one group contain no diagnostic. The flow-facts code issued about ten user diagnostics from 57,000 lines. The judge wrote its reasons to about 80 trace prints. A refusal was often a `None` returned by a lowering planner, and dozens of budgets (depth 32 at twenty sites, 64 monomials, 32 worlds) ended in "unknown" with no message. | Each budget in the proof producer needs its own diagnostic, distinct from "no proof exists". The spec already asks for this ([check on the lowered form](check_on_lowered_form.md#one-meaning-per-obligation)). |
| **Rules lived in planners and rewrites.** | Linear overwrite, double move-out, guard exhaustiveness and borrow admission were decided by whether a lowering plan could be built. Three "validation" passes rewrote the program. Later code recognized earlier rewrites by name prefix (`__hoist_`, `__transition_subject#`). | Stage 05 reads and stage 06 judges. A proof term is built once and passed forward, never rebuilt from a rewritten tree. |
| **Build speed shaped the rules.** | Parallel record and replay, caches filled by whichever caller asked first, widening thresholds taken from the program's own literals, about forty fixed points with no bound beside budgets chosen per function. Answers depended on unrelated content: whether `u64` was named anywhere, a sort order, a type annotation elsewhere. | A proof search result must depend only on the goal and its hypotheses. Any cache key is the goal, not an address. |
| **The tests pinned plumbing.** | The contract prover's second half has 57 unit tests: 22 on a query cache, none on a rule. Six of the eight slices of declarations and domains have no tests. No test in the proof crate ran a program through `check_proof_plan`. The corpus tested rules from outside, one program per placement. | Each general rule gets its positive and negative twin beside the code, as the plan's customers column already requires. |
| **The proof crate held two proof systems that shared nothing.** | The range checker never mentioned the elaborator's integer theory, and the elaborator never mentioned a range proof. A range the checker accepted was not a fact an induction could cite, and a theorem never bounded a value. | Step 2's producer must be able to use an integer fact inside an equality proof. L5 (typed identities in `ProofTerm`) and L3 (one signature) are what join them. |
| **The lowered design was already inside the old code.** | The contract checks had a value-and-epoch core (captured operands, read epochs, storage-graph anchors) beside the text routes. Ownership tracked a claim apart from the place holding it. Build-time evaluation of array lengths and calls ran on Terminal Psi. The old contract prover called the lowered prover and fell back when it declined (`contract_entailment.rs:15415-15431`), and the lowered-form checker gated compilation before #2823. | #2823 removed the fallbacks. It did not swap one design for an untried one, which is the strongest support for it. |

Two readers' claims about the old code were checked again by hand for this
section. `seed_index_proofs_from_expression`
(`checks_ranges.rs:3426-3490`) records every `a[i]` it finds anywhere in a
clause as a proved index, keyed by two display names, including under `||`.
The mathematical elaborator's kernel use is as the first row says. Each
synthesis lists the further claims its author checked in the source.

The old judge's authors wrote down rules that the new proof producer must
keep. Forming an `Id` proves nothing, and executable equality is not `Id`:
IEEE equality of signed zeros gave the judge no rule
(`contract_entailment.rs:49781-49782, 52623-52625`). A clause may use a call
as a value only when the call is total and pure (`:84121-84124`). An
inductive hypothesis is registered only after the recursive call's
`requires` and its decrease are checked (`:57183-57187`). The elaborator's
authors left two cautions for step 2. All integer widths shared one kernel
carrier and stayed distinct through alias definitions that conversion did
not unfold, because unfolding a `Wrapping` computation "would silently
replace its denotation with unbounded arithmetic"
(`mathematical_declarations.rs:423-425, 4263-4303`). And "nested or mutual
source-family recursion has no checked encoding yet" (`:11271`).

#### What today's compiler does with the old rules

The second pass looked up 1,366 language rules from the 17 inventories in
today's compiler. The readers searched and read code; they compiled nothing,
so each entry is a reading, not a test.

| Class | Rules | Meaning |
| --- | --- | --- |
| Enforced | 839 | A check exists, in most cases an obligation that stage 06 accepts only with a certificate. Ownership, loans and domains are judged by one structural walk each, with no certificate. |
| Refused wholesale | 253 | The construct is refused by stage 05 or the parser, so the rule cannot be broken. The rule has to be rebuilt when the construct is served. |
| Unclear | 154 | Related code was found and the reader could not decide. |
| Not found | 78 | Neither a check nor a refusal was found. |
| Not looked up | 42 | |

Of the 78 rules not found, about 32 are ways the old checker accepted more
programs: ring rearrangement, facts read from a callee's body, float
ranges, loans in different cases of one sum, array indexes proved different,
coinductive guarantees over recursion. Their absence refuses programs and
accepts nothing new. About 20 are retired surfaces or constructs with no
route from source today. The rest are below, with what was done to check
each.

Treat the rows that were not probed as leads. Eight of these rules were
tested with small programs. The compiler enforces four, three are real gaps,
and one depends on how a spec sentence is read. A search that finds no
check does not show there is none, and a reading that calls a rule "refused
upstream" does not show the wrong program is refused.

| Rule | What the reader found | Checked for this audit |
| --- | --- | --- |
| Two parameters of one machine with one name | Nothing compares a machine's parameter names with each other | **Gap, confirmed by a probe.** `machine pick(a: i32, a: i32) -> i32 requires a > 0 { a }` compiles, and both the contract and the body read the first parameter: `pick(1, 0 - 7)` compiles and runs to Psi exit 0, and `pick(0 - 7, 1)` is refused. [State contracts](../../spec/language/state_contracts.md) forbids it: "A runtime parameter or local cannot reuse the name of a visible value parameter or local". The check for locals is there; `02_.../names/repeated_locals.rs:69-80` adds each parameter name to the list without comparing it with the earlier ones. **Repaired on this branch** (`abce222c1b`): the second parameter is refused where it is declared. |
| Two declarations of one name in one scope (two `data`, two `trait`, a `data` and a `trait`), two machines with one callable identity | No refusal at the declaration; a later use is refused as ambiguous | Probed: all four are refused, so the reader's "likely unenforced" was wrong. The refusal is an internal check with no source position, and it prints a debug dump: "source selection dictionary repeats a declaration in one purpose: 34 SelectionDeclaration { identity: ..." |
| The number of arguments in a type application matches the declaration | A mismatch is skipped with `continue` in const-argument admission (`03_.../src/type_reference.rs:908-932`); no other count check was found | Probed: `Box<i32, i32>` for a one-parameter `Box` is refused, with no source position: "data application does not retain its complete argument tuple" |
| An `AccessCustody` implementation agrees with its schema | Nothing | Confirmed: no Rust file names it, and the trait is declared with no members (`source/library/core/layout.omg:514`), so a conformance has nothing to fail. Reach is limited today, because stage 05 refuses data with an erased field. The check has to exist before erased fields are served. |
| A value stored into a narrowed integer field fits its stored width (four cross-checks) | No fit obligation | Unreachable today; see [old faults that remain](#old-faults-that-remain-in-todays-compiler) |
| Content conservation: a content-bearing result descends from a consumed owned claim (four cross-checks) | Stage 05 has no reader for it and the prover refuses the goal form. No reader found the site that refuses the source form. | **Gap, confirmed by probes.** Three things are refused: a cast that adds an issuer-only qualification in a machine that is not an establishment route, the same forged on an `Extent`, and a declared content projection (`satisfies Content<...>::project`, "not supported yet"). A field write is not: a machine that is no establishment route takes an `Extent in Granted`, sets `base` to 4096 and `length` to 4096, and returns it as `Extent in Granted`. It compiles. So does growing one in place through `@mutable` from 16 bytes to 8,192, and the same on a user domain with one issuer. The write must keep the domain's `requires` (a `length` that breaks `no_wrap` is refused), but the qualification itself survives. [Domains](../../spec/language/domains.md) says "Mutation invalidates subject-bound facts unless preserved or re-established", and `extent.omg` says constructing the geometry does not establish `Granted`. Not shown: a program that uses the forged extent to reach memory. **Repaired on this branch** (`d090365e17`): a machine outside a domain's routes may not change part of a value that carries the domain, or lend part of it for change. All the programs above are refused at the write. Declared conservation equations are still not served. |
| While an owned value is moved out of borrowed storage, boundary, service and blocking calls are fenced | Only a call that may suspend is refused (`psi/semantics/src/plain_affine.rs:2450-2477`) | Probed, and the reader is right about the behavior. With a field moved out, a boundary call with no reach, a boundary call that reaches a service and a blocking boundary call all compile; a suspending call is refused; lending the holder itself is refused. Whether the three accepted programs break the spec depends on a reading. [Ownership](../../spec/language/ownership.md) keeps the fence on "boundary/capability-carrying calls that may observe or expose the affected storage, including through authority not named as an argument" and says "Unknown access is not evidence of non-observation". The compiler has no test of whether a boundary call may observe the storage. **Repaired** (`psi/semantics/src/plain_affine/fences.rs`) under the [ratified restoration rule](../../spec/language/ownership.md#boundary-calls-during-restoration): while the window is open, the ownership walk judges each call by its contract. It refuses a boundary requirement whose envelope reaches a service, may enter a callback (an `invokes` row, including one on a callable parameter) or may suspend, and an ordinary callee whose body can reach any of those, a service instruction or a computed call; each refusal names which. A requirement declared `reaches none` that receives only the detached value, blocking or not, runs inside the window, and every later handback must still restore the field. An ordinary call that lends a disjoint sibling still compiles. Decoding a published module judges these calls with the same module-aware walk and the module's own declarations. |
| An Omega-written provider of process exit never returns and passes its status on | Only native leaves are compared with the requirement's completion class | **Gap, confirmed by a probe.** A provider in the program's own package whose body is `ProcessExitPrimitive(0);` is selected with `Build::select_provider`. The program calls `ProcessExit::exit_process(7)` and exits with status 0 on both engines; the same provider passing `return_code` exits with 7. [Process exit](../../spec/language/process_exit.md) says "the exact exit argument survive[s]" and a provider "cannot reinterpret" the operation. **Repaired on this branch** (`5bc6119e59`): a provider must pass on the very values it received; the program above is refused at the call. |
| A provider body selected by Build has no `crashes`, matches its requirement and is not generic | Crash freedom is checked only for the root of a build-time evaluation | Not checked |
| A public `const` type, conformance header or domain alias does not name a private declaration | Machine signatures are checked. `02_.../names/public_interfaces.rs:16-18` says a domain written as a constraint "is not judged here yet". | That comment confirmed |
| A write through a mutable view retires what is known about the collection | Could not confirm | Enforced, by a probe. After `poke(@mutable backing[..])` writes `view[1]`, a call that requires `backing[1] == 19` is refused; without the write it compiles. A reviewer traced the mechanism: the store is moved back onto the backing array's region (`06_.../src/places.rs:1185-1208`, `places/backing.rs:18-66`). |
| A clause does not read through a stored reference | The clause reader follows a shared stored reference (`05_.../src/read/contracts.rs:2476-2489`); the old rule was that nothing does | Not checked |
| A value live across a suspension has a carry policy that allows it | A comment says it "is checked row by row" (`06_.../src/operational.rs:347-348`); no code was found | Not checked |
| Smaller ones: accessor privacy, destructive-read views, the five properties of a progress-profile domain, at most one `via`, wire schemas with no `encoding` | Not found | Not checked |
| A Boolean expression written as a statement is proved | Today it is evaluated and discarded | Not a gap. Probed: `x > 0;` as a statement is refused with "use the returned value or explicitly discard it with `_ =`". [Contracts](../../spec/proofs/contracts.md) says there is "no new assertion primitive", and the corpus's assertion fixtures use a guarded transition |

#### Old faults that remain in today's compiler

Each of these was confirmed in today's source for this audit, except the
last row, which five cross-checks report. None is shown to accept a wrong
program.

| Fault | Where | Old pattern it repeats |
| --- | --- | --- |
| An operator expression with no recorded selection is completed with no check | `complete_operator` returns `Ok(())` when the loop over `authored_selection_occurrences` finds no `Operator` selection (`05_.../src/read/source_operators.rs:447-484`). Two cross-checks found it independently. Stage 02 gives every binary, indexed and unary expression such a selection, except a type predicate such as `T == u32` and a type operand (`02_.../selections/authored_selections.rs:1323-1337`), so the gap opens only if a selection is lost between the stages. | A missing record means builtin |
| The validator of scalar domains on stores lists the storing operations by hand and ends in `_ => {}` | `psi/semantics/src/scalar_qualifications.rs:376-409`. A field read now carries its declared domain, so soundness rests on every store being in this list. One is already missing: the list has no `ElementViewWrite`, while the read side a few lines below trusts `ElementViewRead` to carry the element's declared domain (`:424-446`). Stage 05 does not emit `ElementViewWrite` today; a Psi module from elsewhere, or a later lowering, would pass unchecked. | A hand-listed match with an accepting default arm |
| The foundation class of each proof rule (constructive, trusted, classical) has no consumer | `AcceptedProofRule::foundation` is called only by tests in its own file (`bounded/classicality.rs`). The tests pin the table; no certificate is checked against it. | Audit described, not connected |
| A const argument is written into the type table as a name with no symbol and parsed back from its digits | `typed-trees/type_records/data_applications.rs:99-108`, `typed-trees/expressions/values.rs:73-77` | A value carried as text |
| The stored-field `range_obligation` slot has no producer | Stage 05 writes `None` at all six sites (`05_.../src/lower/places.rs:3611-4964`). It is unreachable today because stage 05 reads only whole-field placements (`AuthoredFieldStorage::Direct`, `read/source_records/layouts.rs:443-448`). The fit proof that [layout plans](../../spec/layouts/plans.md) require must arrive with narrowed integer fields. | A slot with readers and no writer |
| The spec and the library say the compiler checks an `AccessCustody` implementation against its schema; no Rust file names `AccessCustody` | `wiki/spec/resources/access.md:342`, `source/library/core/layout.omg:511-514` | A check described and not built |
| `ProofOnlyClassification::classify` is called by nothing | `typed-trees/proofs/evidence_and_inspection.rs:463` | Dead code that reads as a check |
| A program that breaks a rule is told "not supported yet", or gets an internal message | `[copy]` data with an owned field is "not supported yet" (`05_.../src/read/places.rs:485`). Two `data` of one name get a debug dump of a `SelectionDeclaration` with no source position. Forging `Granted` on an `Extent` by a cast is refused as "a defect of the checker", while the same cast on a user record gets a clear message (all probed). |
| The stage 05 README lists references, slices, floats and domains as refused; the code serves them | Five cross-checks classified from the code for this reason | A document behind its code |

### What this changes in the plan

- **The fault this audit found in today's stage 05 is the old compiler's
  fault at smaller scale**, so L1 is the main repair, not a refinement.
- **L2 takes four forms, not two**: application, `Id` at a type, constructor
  and projection.
- **There are two kinds of goal and both need evidence.** Integer goals go
  to stage 06's existing prover. Equality goals over terms need a producer
  that does what the old judge did (unfold, split by case, use the
  inductive hypothesis and the cited lemmas, close by congruence) and
  returns a kernel term.
- **Search needs budgets.** Every limit in the old judge (depth 32, 64 forms,
  8,192 work units) answered `Unknown` when exhausted, never yes. Keep that.
- **`core/nat.omg` needs a small part of rearranging by algebra laws.**
  Most of its laws cite their lemmas one by one. Two do not:
  `mul_distributes` and `mul_distributes_right` (`nat.omg:282-322`) cite
  only the inductive hypothesis and leave a shuffle of three or four
  addends that the old judge closed with the proved `add_comm` and
  `add_assoc`. So reordering and regrouping one operation under its two
  proved laws comes before `nat.omg` in full (step 3a), and expansion of
  products stays later (step 4a). An earlier version of this audit said
  `nat.omg` needed none of it; a reviewer caught that.
- **The full read adds guards to the steps, not steps.** L1 ends with a
  ratchet on the number of clause readers in stage 05. The proof producer of
  step 2 closes a dead path with a kernel proof of falsehood, reports an
  exhausted budget separately from a missing proof, and keys nothing by
  source position or address. A cited law is used through its signature
  entry (L3), never because it was checked earlier in the same run.
- **The probes found three gaps that are not proof-layer work, and they are
  repaired on this branch.** A machine outside a domain's establishment
  routes can no longer change part of a value that carries the domain; a
  provider of the process exit must pass on the status it received; two
  parameters of one machine cannot share a name.
- **The first repair is evidence for L1.** The authority gap was a rule
  guarded at one operation (the cast) and missing at another with the same
  effect (a field write). The repair added no case per place. Stage 06
  already had one walk that finds every value a write changes, in order to
  reprove data `requires` clauses, and that walk resolves a store through a
  reference to the storage that was lent. Asking it one more question
  covered an owned local, a borrowed formal, a qualified field of a larger
  record, a local reference to an inner field, and an inner field lent to a
  callee that sees no domain. The old checker would have needed a copy per
  arrangement; the corpus had nine.
- **The second repair shows a limit of contract refinement.** A requirement
  that never returns has no `ensures`, so the refinement proof of a provider
  could not see that the provider changed the exit status. The rule had to
  read the provider's body. It decides by identity of values, with no proof
  search: a provider that passes `return_code + 0` is refused although the
  value is equal. Proving equality there would need a new obligation kind.
- **The faults that remain are small repairs for the parallel row.** In
  `complete_operator`, refuse when the caller did not mark the operator a
  known intrinsic and the loop saw no `Operator` selection (refusing
  whenever nothing is pending would break type predicates and operators
  already resolved to an intrinsic); replace the `_ => {}` arm of the
  scalar store validator with an explicit list that includes
  `ElementViewWrite`; refuse or check an `AccessCustody` implementation; add
  one direct negative test for each family that is refused only indirectly
  (named evidence bindings, content conservation).

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

- **Thirteen term readers.** (Step 5 replaced them with one; see
  [Step 5 as built](#plan-order).) Thirteen functions in four files return a
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
  and prover rule. (Marker 294 retired `StructuralIdentity`: Psi now
  publishes the general Identity of inputs, which takes a type. Marker 295
  retired `ByteSequenceEqual`, whose one producer had no caller. Marker 296
  made FloatMeaning equality an Identity at the FloatMeaning carrier.) `identity_clause` (`read/contracts.rs:906`) serves two.
- **A call in a clause is unfolded or refused.** `read/clause_calls.rs`
  serves a call only to a closed machine with no contract, scalar parameters
  and a scalar result, in a clause of the entry state, by reading the
  callee's body in place of the call. A machine with no body cannot be
  unfolded.
- **`ProofTerm` names types by text.** Five of its variants and
  `MathematicalArgument` hold a `type_identity: String`
  (`proposition.rs:1466-1500`), against the lowered program's rule that no
  decision is keyed on text.
- **A certificate the mathematical kernel cannot translate is refused.**
  `bounded_denotation.rs` names all 33 rules; eleven sites return
  `Unsupported` for a shape it cannot translate (for example "a carrier bound
  of an integer whose width is a parameter"). Since #5715,
  `re_decide_in_mathematical_core` turns every `BoundedDenotationError`,
  `Unsupported` included, into `ProofError::MathematicalCore`
  (`bounded/proof.rs`), and stage 06 reports it as "the proof kernel refused
  the certificate". Rules the core cannot derive still reach the kernel as
  rule-instance assumptions, as [the metatheory](../../spec/proofs/kernel_metatheory.md)
  sanctions. No program on `main` is known to reach an `Unsupported` site;
  most guard invariants an earlier check already established. One of them,
  the citation crossing in `same_denotation`, cannot fire on a well-formed
  proposition over a fixed, non-address carrier: a math equality must hold
  its endpoints in sorted order, so its lowered `Equal` lifts back to the
  same proposition. An address carrier lowers but does not lift back, so a
  math equality over addresses cited at its lowered `Equal` reaches the
  crossing and is refused
  (`a_math_equality_cited_at_its_lowered_form_denotes_identically` pins both).
- **Terminal Psi has one slot per construct.** `mathematical_carriers`,
  `identity_formations` and `indexed_theorems` each carry one kind of
  declaration, and none has a writer.

### What to change in this layer

Each change removes one axis of the combinations above. They are proposals,
checked against the code but not built.

| # | Change | What it removes | Measure |
| --- | --- | --- | --- |
| L1 | **One term reader in stage 05.** One function reads an expression into a term at every position. `ClauseScope` supplies only what each name denotes there (a formal, the entry snapshot for `old`, the result, a field of the value being built). | the position axis: a term form served once is served in every clause, domain predicate, definition body and erased argument | readers per term form: 1; refusals that name a position: 0 |
| L2 | **Four general term forms.** Add an application of a published declaration to argument terms, `Id` at a type, a constructor of a `data` case applied to its fields, and a projection of a field from a term. These are the four forms the old judge's term type had and `Proposition` lacks. The existing scalar, integer and float forms stay as the cases the arithmetic rules work on. | the operand axis: Identity for every type (C5), a call to a machine with no body, a definition with binders (C1) and a quotient relation (C10) become the same four forms, with one reader, one prover route (reflexivity, symmetry, transitivity, congruence, constructor injectivity) and one translation to the kernel | equality forms a new type needs: 0 |
| L3 | **One signature table in the lowered program**, written by stage 05 and checked once by stage 06 with `check_signature`. Publication fills the three tables the wire format already specifies (indexed theorem rows, mathematical carriers and the Id formation catalog with its shared certificate, [encoding](../../../source-rust/subsystems/compiler/psi/artifact/wire/encoding.md)) from that one table. The wire format does not change: [Psi claim admission](psi_claim_admission.md) says "No codec retirement". An earlier version of this row replaced the three wire tables; a reviewer pointed out that they are ratified. | a table and a wire reader per construct | writers per declaration kind: 1 |
| L4 | Done in #5715: **a shape the kernel cannot translate is refused.** `MathematicalCoreDecision` has only `Judged`; every translation error refuses the certificate. | a second basis of trust that nobody measures | accepted without the kernel's judgment: 0 |
| L5 | **Typed identities in `ProofTerm`** in place of `type_identity: String`. | decisions keyed on text | text keys in `ProofTerm`: 0 |

L1 and L2 replace R1's separate routes for definitions, Identity and
quotient formation. Build the single reader for the new forms first, so
they work at every position from the start, then move each existing form
into it and delete the reader it came from in the same change. That avoids
both rewriting 6,705 working lines at once and adding the new forms to
thirteen readers.

## Who needs it today

### Library declarations

| Declaration | Uses | Blocked by |
| --- | --- | --- |
| `core/extent.omg:228` `no_wrap(base: addr, length: u64): core\Strict<0>`, used by `Extent::Granted` (`:235-237`) and `allocation/bump/bump.omg:16` | closed, scalar formals, `Strict<0>` result | nothing in this task: stage 05 serves it as a `Proposition` (C4). Stage 06 test `src/tests/address_bounds.rs:290-319` accepts a call whose caller proves the bound and refuses it without. Its sample consumers fail earlier, see below. |
| `core/extent.omg:219` `core\Identity::<IntervalSet<Nat>>(...)` in `transfer`'s `ensures` | Identity over data | C5 ("Identity of an unserved type") |
| `core/algebra.omg:29-41` `CommutativeSemiring<T>` laws, `ensures core\Identity::<T>(...)` | Identity over a generic T; resultless theorem slots | C5, C8 |
| `core/cauchy.omg` (28 uses), `nat.omg` (46), `nat_metric.omg` (32), `rat.omg` (28), `int.omg` (19), `math.omg` (14), `seq.omg` (12), `float_operations.omg` (78, mostly `FloatMeaning`) | `core\Identity`/`core\Type`/`core\Strict` over `Nat`, `Rat` and generic sequences, in `requires`/`ensures` | C5 for non-scalar T; C8 where a machine is a theorem |

No `[erased]` binding and no other top-level mathematical `let` appears in
`source/library/`. Nor does `core\Level`, `core\Squash` or a quotient type,
so the library needs neither level binders (C2), the Squash mapping (C3) nor
quotients (C10) today.

It does need induction over plain data. `Nat` is `data Nat { case Zero;
case Succ(prev: Nat); }` (`core/nat.omg:34`), and its laws are proved the way
`add_assoc` is (`:217-234`): the machine `terminates by a`, branches on `a`,
and calls itself on `prev` as the inductive hypothesis. The comment there
says both sides "normalize by unfolding plus the inductive hypothesis". So
`nat.omg` needs four things together: `Nat` as a kernel type (R2 without
indices), `add` as a kernel function defined by cases, the law machine as an
induction (R3), and a proof of each case from unfolding and the cited
equalities.

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
change L2 above serves it and Identity with two of its four forms.

### The deleted corpus

`tests/omega/` held 16,759 fixtures and 751,772 lines of Omega when #2828
deleted it (`f4c7a70beb^`). It is not to be restored. It is read here for
two things: what its shape says about the old way of working, and what its
programs say a compiler has to accept and refuse.

**Reading every program in full is not worth its cost; running the rejected
ones is the cheaper test, and today it is blocked by syntax.** Every fixture
is indexed and its stated purpose read, about 1,600 were opened, and the
clause forms below come from a search over every file. What reading cannot
tell is whether today's compiler still refuses what a rejected fixture
tests, and the probes showed that readings of the compiler were wrong about
half the time. The corpus has 8,038 rejected fixtures. A random sample of 60
was run through today's compiler unchanged: 55 stop on syntax the language
has since retired (`&mut`, the old entry form, arms without commas), so
their refusal says nothing about the rule they test; 4 reach a semantic
error; 1 compiles. That one is `fail/data/opaque_copy_unselected`
(`boundary data Token [copy];`, expected "cannot claim `[copy]` without an
admitted property receipt"). Whether the spec still has that rule was not
determined. So a mechanical run finds leads only after the fixtures are
rewritten to current syntax, and the better use of the corpus is the one
this audit already makes: recover the one pair a step needs, in current
syntax, beside the test that owns it.

**38% of the corpus was one matrix.** 6,438 fixtures are named
`<feature>_through_<placement>`: 379 features, each written through the same
nine placements, each as an accepted and a rejected program. Each placement
has about 702 fixtures. This is the per-placement pattern in test form. The
old compiler had one rule per placement, so it needed one test per
placement to know each rule existed.

| Placement | How the value reaches its use (from `range_fact_delivery`) |
| --- | --- |
| `local` | `let index: u64 = input;` then `count(index)` |
| `field` | `data Reading requires index < 4 { index: u64; }` then `count(reading.index)` |
| `parameter` | `machine probe(index: u64) requires index < 4 { count(index) }` |
| `call_result` | `count(source(input))`, where `source` has `ensures result < 4` |
| `const_name` | `const LIMIT: u64 = 4;` then `requires index < LIMIT` |
| `match_join` | `let index: u64 = match flag { true -> left, false -> right };` |
| `other_state` | `transition { _ -> deliver(input) }` into a nested machine |
| `other_machine` | a top-level `forward` machine between the caller and the use |
| `target_body` | the body moved into per-target providers selected in `build.omg` |

What the two readers of the `facts` category found in it:

- **The routing was written by hand.** Across 28 families compared in full,
  helper names, what is routed and even whether the field carries an
  invariant differ from family to family. Only the `target_body` scaffold is
  a template (160 of 237 accepted fixtures in one half use the same one).
- **A rejected twin is one small edit.** Over 198 pairs: a wrong constant or
  expression (56), a bound loosened by one step (37), a clause or conjunct
  removed (18), an ownership change (24), a wrong selection or type (23),
  and smaller kinds.
- **The matrix was applied to features that are not about facts.** Calling
  conventions, `size_of` and `align_of`, ownership, cleanup and foreign
  borrows were all routed through the nine placements, usually by phrasing
  the result as an interval fact so that a changed constant "may overflow".

**By category** (fixtures):

| Category | Fixtures | Category | Fixtures |
| --- | --- | --- | --- |
| `facts` | 7,170 | `termination` | 311 |
| `borrows` and `borrow` | 1,080 | `layouts` | 309 |
| `generics` | 653 | `arithmetic` | 304 |
| `calls` | 566 | `build` | 286 |
| `domains` | 525 | `contracts` | 280 |
| `proofs` | 476 | `float` | 275 |
| `ownership` | 374 | `core` | 255 |
| `traits` | 362 | `control_flow` | 239 |
| `providers` | 339 | `drops` | 226 |
| `slices` | 339 | 67 smaller categories | 2,390 |

**What programs write in a clause.** This is the list a single term reader
(L1) has to serve at every position. Counts are of `main.omg` files in the
whole corpus unless a category is named.

| Form | Example | Files |
| --- | --- | --- |
| Comparison and arithmetic | `requires 2 * acc + n * (n + 1) <= 10100` | most of the 10,321 with `requires` |
| Field, element and length path | `ensures result.rest.length == whole.length - length`; `requires items[0] == 7` | common; 63 `result.field` paths in the proof categories alone |
| A machine call as a term | `requires value <= limit()`; `ensures encode(value) > 0`; `requires valid_utf8(self)` | present in every category read |
| `core::Identity` | `core::Identity<Nat>(add(add(x, y), z), add(x, add(y, z)))` | 460 |
| `embed` | `embed(self.previous) + 1 == embed(Length)` | 470 |
| `old` | `Granted::content(old(&storage))` | 301, nearly all this one shape |
| Membership, also of a call result | `requires ticket in Ticket::Issued`; `ensures Encode::try_from(value) in Result::Ok` | 4,856 |
| Case test with a condition | `(self in Interval::Empty) \|\| ((self in Interval::Range) && self.lo <= self.hi)` | common in `domains` and `dependent` |
| `match` in a clause | `ensures (match result { Reply::Kept { number } -> number == input, ... })` | about 16 |
| Type equation | `requires T == u32 \|\| T == u64`; `requires Length == [u8; Capacity]` | 124 fixtures in `generics` |
| A requirement on a machine parameter | `requires machine Step(input: &mut u64) -> u64 requires input == 0;` | 624 |
| Guarded crash | `crashes Trap value < Limit` | 484 |
| Quantifier | none: `forall` and `exists` appear in no program | 0 |

**Which types Identity is stated over** (94 fixtures in the proof
categories): the core `Nat` 35, a quotient's representative type 22, generic
`T` or `A` 10, plain records about 10, `IntervalSet<Nat>` 4, a user Peano
type 2, floats 3. Elsewhere `core::Identity<IntervalSet<Nat>>(...old(&whole)...)`
is the content-conservation claim, in 122 `core` fixtures.

**What the proofs prove.** The accepted proof fixtures include: Identity
over `Nat` is reflexive, symmetric and transitive and `Succ` is injective;
addition over `Nat` is commutative and associative; `plus` over a user Peano
type is associative, by induction (`pass/proofs/ring_law_conformance`);
`(a+b)(c+d)` expands, by semiring normalization with an empty body
(`pass/proofs/ring_full_polynomial_compile`); a Gauss sum, by integer
induction; and equivalence laws for a quotient. Three accepted fixtures in
those categories state an Identity proved on a structurally recursive
machine. A proof body cites zero to four lemmas, and most cite one.

**What had almost no programs.** `core::Squash` appears in no fixture.
`core::Level` appears in 18, all of the `representation_agreement` family.
The proof categories have no universe test. So level binders (C2) and the
Squash mapping (C3) have no customer in the corpus or the library.

**What the corpus says the compiler must keep doing**, beyond any placement.
Each line is a distinct rule with both accepted and rejected programs:

- A write to a field of a `data` with `requires` opens a window that must be
  closed before the next read, call or return (113 families in `facts`, 56
  fixtures in `dependent`).
- A write or a call retires exactly the facts on the storage it may write,
  and no others (51 fixtures in `contracts`; "write frames" in `calls`).
- A value saved before a write keeps its facts; a re-read does not (47
  families).
- Every arrival at a loop state proves its index again; a join keeps a fact
  only if every edge carries it.
- A domain is established by a guard, a cast, a construction, a callee's
  `ensures` or a named issuer, and by nothing else (525 `domains` fixtures).
- Division needs a nonzero divisor and excludes `MIN / -1`; a shift needs
  its count below the width; an exact cast needs the value to fit.
- `==` on floats excludes NaN as a premise; Identity on floats is on bits.
- Termination by a rank in a range, a declared measure, a lexicographic
  measure, descent on `Nat`, or a shrinking slice (311 fixtures).
- A generic body is checked once, under its own requirements, with its type
  parameter rigid, and owes its obligations even when never instantiated.
- A satisfier may not add premises to a trait member or drop a conclusion,
  and a law satisfier may not assume its conclusion.
- An `[erased]` binding never feeds runtime data or control.
- Quotient formation, `lift` and `define` (25 `proofs` fixtures and 18 in
  `facts`).

**Reading coverage.** Every fixture's index row (name, leading comment,
expected diagnostic) was read: 16,759 rows across eight readers. About
1,600 programs were opened, many only to their first 30 to 60 lines. README
files and most `build.omg` files were not read. Group counts above come from
name and keyword patterns and are approximate.

**What the ledgers recorded.** `corpus_outcomes.txt` holds the old
compiler's verdict on each fixture: of 8,690 accepted-tier rows, 8,440
checked and 221 were rejected; of 8,279 rejected-tier rows, 8,270 were
rejected, 8,096 of them with the expected diagnostic. Its header says it is
a "Historical three-way integration, not a new whole-corpus run", and its
rows carry more than 670 different revision stamps. So no single revision
of the old compiler is recorded as passing the whole corpus. The execution
ledgers record 5,749 programs built for `linux_x86_64` and 6,362 on
`linux_arm64` whose interpreter result agreed with native. There is no
ledger for `windows_arm64`.

Two fixtures are the smallest whole customer for a library law:
`pass/proofs/ring_law_conformance` and its twin
`fail/proofs/ring_law_unproven_rejected`, a law satisfier with no `ensures`.

**What this changes in the plan.**

- **L1's acceptance is the clause-form table above.** The single reader is
  done when each form reads at every position, not when one program passes.
- **Do not rebuild the matrix.** Under the lowered form a fact belongs to a
  value, so one accepted and one rejected program per mechanism is the
  test. Eight of the nine placements are ordinary lowering (a local, a
  field, a call, a constant, a join, a state, a machine); if a mechanism
  needs new code for one of them, the mechanism is too narrow.
- **Level binders and Squash leave the plan** until a program needs them.
- **Rearranging by algebra laws has customers**:
  `pass/proofs/ring_full_polynomial_compile`, its twin
  `fail/proofs/ring_rearrange_unlicensed_rejected`, and the `open_index_*`
  families. It is a step of its own after `core/nat.omg`.
- **Quotients have 43 fixtures** to recover a pair from when their step
  comes.

The only measured comparison of the old and new checkers is
`wiki/drafts/designs/route_one_measures.md` (2026-11-11, removed by #3062;
read from `76bce12121^`). It covers scalar programs: the new route lowered
121 of 3,486 passing fixtures at that time and agreed with the old checker
on 101. Its disagreements were all about scalar arithmetic and contracts.

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
| 18 | "`u` is spelled like a builtin type but is not one": `representation_agreement`, at `core\Type<u>` | S03 `type_reference.rs:3821` | **this task**: C2 level binders under the [builtin naming rules](../../spec/language/modules.md#builtin-type-names) |
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
| 1 | L1 and L2 on an abstract type: the single term reader, serving an application of a declaration and `Id` at a type in every clause position (constructor and projection arrive with step 2). Stage 06 proves such a goal from cited equalities by reflexivity, symmetry, transitivity and congruence, as a kernel term. No `data`, no induction. | stage 05, lowered program, stage 06, kernel translation | one new pair: a generic machine over a `CommutativeSemiring<T>` that cites `add_comm` and relies on the Identity, and a twin that relies on it without the citation |
| 2 | A plain `data` as a kernel type with its constructors as terms (R2 without indices); a machine defined by cases on one parameter as a kernel function; a law machine as an induction (R3); L3, so these declarations are published once and checked once with `check_signature`. The proof producer normalizes both sides with the kernel, then closes the goal by congruence over the cited equalities and the inductive hypothesis. The accepting test reads the proposition the mathematical kernel judged and the assumptions it used (`MathematicalCoreDecision::Judged`). | stages 05 and 06, lowered program | `pass/proofs/ring_law_conformance`, recovered from `f4c7a70beb^` beside the owning test, with two rejected twins. The recovered `fail/proofs/ring_law_unproven_rejected` has no `ensures` and an empty body, so it tests only that a law must be declared. The second twin keeps the positive's exact `ensures` and removes the recursive call in the successor case; the compiler must name the equality left unproved on that path. |
| 3a | Reordering and regrouping one operation under its proved commutativity and associativity laws, as a producer of kernel terms. While a law is being proved, that law is not available to its own proof. | stage 06 | `mul_distributes` and `mul_distributes_right` from `core/nat.omg` on the Peano fixture, and a twin that needs distributivity to prove distributivity |
| 3 | `core/nat.omg` checked in full as a validation subject | stages 05 and 06 | its eleven `satisfies CommutativeSemiring<Nat>` machines |
| 4 | The rest of R1: definitions with binders and `boundary let` as declarations in the same signature | stage 05, stage 06 | `cauchy.omg`, `rat.omg`, `int.omg`, `extent.omg:219` |
| 4a | The rest of rearranging by algebra laws: expansion of products into a polynomial normal form | stage 06 | `pass/proofs/ring_full_polynomial_compile` and `fail/proofs/ring_rearrange_unlicensed_rejected`, recovered beside the owning test |
| 5 | Move each existing term form into the single reader and delete the reader it came from | stage 05 | each position refusal listed [above](#how-a-clause-becomes-a-proof-term-today), with its twin |
| 6 | The rest of R2: value-indexed families. Needs value-indexed type arguments (`Vector<previous, previous_total>`) evaluated in stage 03 first. | stages 03, 05 | `source_indexed_*`, once their other blockers are repaired |
| 7 | Quotient formation (L2, L3) and the correspondence judgment (R3) | stages 05, 06 | none in the library or samples today; the corpus had 43 fixtures to recover a pair from |
| — | In parallel, each independent: (a) divide/remainder/shift endpoints from an interval divisor in `IntegerAffineBound`; (b) irreflexivity of `<` on an open value; (c) L5 | stage 06, kernel | S06 README:1039-1052 programs |
| later | `core\Level` binders in stages 02-03 and the Squash mapping, when a program needs them | stages 02-03, 05 | `representation_agreement`, the only family that uses a level |

Step 1 is small on purpose. [Check on the lowered form](check_on_lowered_form.md#lane-order)
says not to spread a new representation before one checked end-to-end route
works, so the first two term forms are proved on the case with no `data` and
no induction before step 2 leans on them.

Step 2 carries the main risk. The kernel encodes a `data` as a W type, and
the old elaborator needed budget tests for its indexed-vector terms
(`mathematical_declarations.rs:9847-10330`). Measure the kernel's cost on the
Peano fixture before taking on all of `nat.omg`.

**Step 2 as built (2026-11-23).** The recovered `ring_law_conformance` program
is judged by the kernel: `Peano` is a W type, `plus` an `indW` definition and
`plus_assoc` an induction whose certificate rests on no assumption. Both
recovered programs live beside the stage 06 test, with only their spelling
moved to today's syntax; the trait's `add` also gained `reaches none crashes
none`, which a clause application now requires. What step 2 did not build:

- **Publication of laws.** Stage 06 writes one Psi carrier row per data
  declaration into the root module: the data's source type identity and its
  W type under the declarations up to its own. It writes no theorem root and
  no Id formation row. Both row shapes come from the deleted elaborator's
  erased evidence (`f4c7a70beb^`, `05_typed-trees-to-checked-trees/src/checking.rs:1339-1395`
  and `published_facts.rs:4888-5006`): a theorem root's evidence position is
  an erased formal of the theorem's indexed type, and an Id formation is the
  Identity type of an erased formal over captured source expressions. A law
  over plain data has no erased formal, and its `ensures` Identity is a
  clause with no type identity. The owner decided on 2026-11-23 not to
  publish law certificates: under
  [publication and separate consumption](../../spec/terminal-psi/product.md#publication-and-separate-consumption)
  a receiver trusts the producer and does not replay carried certificates,
  so nothing would check a published one. A row for laws waits for a
  consumer that reads it.
- **One table for step 1's goals.** Built. Each generic check form with
  const record parameters has one Identity table in the mathematical
  signature: its carriers, the trait operations its clauses apply and the
  laws its record parameters' traits state. Stage 06 checks it once and
  `equality::discharge` extends it with each obligation's own inputs.
  Concrete modules (closure, witnesses, arrivals) still declare their names
  per obligation, since they have no trait operations to share.
- **Projection, and fields of another type.** A field of a case is read only
  through the dispatch that binds it. Data whose case has a field of another
  type is refused, because that type has no carrier in the signature yet.
- **Cost.** The hand-built proof of the same law checks in 439 conversion
  steps. The producer's proof takes 23,113 of a 65,536 budget, nearly all in
  normalizing and searching, and its scratch terms fill 271,116 arena slots.
  Step 3a should bound the search before `nat.omg` multiplies it.
- **Unit eta.** A W type has no function extensionality, so a branch is
  stated over an arbitrary child function `k` and a field is `k` at its
  canonical position. That is enough when the tree is consumed only by
  recursion, as here. A law that returns the tree itself, such as
  `add(a, Zero) = a`, also needs `Succ(k position) = sup(succ, k)`, which
  this kernel does not convert. Step 3a met it at commutativity and replaced
  the encoding (below).

**Step 3a as built (2026-11-23).** A proof may now
regroup and reorder the operands of one operation without citing the laws
that allow it. The license is the program's declared conformance: a law that
satisfies a requirement of a trait application and states associativity or
commutativity of an operation satisfying a requirement of the same
application (`OperandLicense` in the
[signature](../../../source-rust/subsystems/compiler/source-to-psi/representations/lowered-program/README.md#the-mathematical-signature)).
Stage 05 reads such laws before the other machines, so a law may lean on
them wherever the source declares it, and a proof still rests only on
statements that come before its own. The search brings both sides of a goal
to one arrangement and puts every law it used into the proof term, so the
certificate names it.

- **Customers.** `peano_right_distributivity` proves
  `times(plus(a, b), c) = plus(times(a, c), times(b, c))` by induction,
  calling only the law itself at `pred`. The kernel judges it resting on one
  assumption, the associativity statement. Its twin drops the call on `pred`
  and is refused at the successor arm: the equality left over is the law
  being proved. `peano_left_distributivity` proves `plus(a, Zero) = a`,
  `plus(a, Succ(b)) = Succ(plus(a, b))`, commutativity from those two,
  associativity, and `times(a, plus(b, c)) = plus(times(a, b), times(a, c))`,
  again calling only the law itself at `pred`. The kernel judges that last law
  resting on exactly two assumptions, commutativity and associativity of
  `plus`. Its twin proves commutativity but does not declare it as the
  trait's `add_comm`, and is refused at the successor arm. This is the shape
  of `mul_distributes` and `mul_distributes_right` in `core/nat.omg`.
- **The encoding of data changed.** Under step 2's raw trees no program could
  prove `plus(a, Zero) = a` (the unit-eta bullet above), so none could prove
  commutativity. The data is now the canonical trees of the same W type:
  `Data = Σ (t : W). Canonical(t)`, where `Canonical(sup(case, k))` holds one
  raw tree per field, an identity between the constructor's child function
  over those trees and `k`, and `Canonical` of each field. `J` on that
  identity replaces `k` by the constructor's own function, so the induction
  principle is stated over constructor applications (`P(m) -> P(Succ(m))`)
  and computes on them, for any fields, by `indW`, `J` on `refl`, beta and
  pair eta. It adds no kernel rule and no assumption, and the kernel checks
  every declaration (`PlainData::declarations`). The
  [inductive profile](../../spec/proofs/inductive_profile.md#derived-indexed-families)
  builds indexed families the same way, as a tree paired with a recursively
  defined condition.
- **Cost.** The kernel checks a two-case data's declarations in 964
  conversion steps, a three-case one with two and three fields in 42,317 and
  four unary cases in 59,242 (`a_declaration_checks_within_one_default_budget`).
  Declaring the label and position types once as constants, where they were
  inlined at each use, is what brought the last two under one default
  budget; they were 236,839 and 702,069. A law's proof is checked by the
  kernel once, as its certificate. Before that change each was checked four
  times (per case, whole, against the trait's statement, as the
  certificate). The producer keeps a defined function folded until the
  argument it dispatches on is a case, so the search itself spends almost no
  conversion steps. Per law, of a 65,536 budget: associativity 11,817,
  right distributivity 7,317, left distributivity 7,324, commutativity
  3,529, `plus(a, Zero) = a` 3,512. Under raw trees associativity took 1,148
  and right distributivity 3,136; the canonical evidence is what the
  difference pays for. Each proof leaves 50,000 to 420,000 scratch terms in
  the arena.
- **Not done: a law written as a machine with a result.** `core/nat.omg`
  states `mul_distributes_right` and its lemmas as `-> Nat` machines, with
  `Nat::Succ { prev: mul_distributes_right(prev, b, c) }` in the successor
  arm. The signature reads a machine with a result as a function and refuses
  a guarantee on one. Step 3 needs a reading of that shape; the Peano
  fixtures state each law as a machine with no result.

**Step 3 as built (2026-11-24): the semiring conformance, not yet the whole
file.** A program that uses the library's `Nat` now gets
`core/nat.omg`'s `CommutativeSemiring<Nat>` conformance judged by the
kernel, from the library file as it stands.

- **Customer.** `nat_semiring/pass/library_conformance` calls only `add` and
  `mul` and states `add(mul(a, b), add(c, a)) = add(a, add(c, mul(b, a)))`
  with an empty body. The kernel judges the seven laws `nat.omg` binds to the
  trait (`add_zero_right`, `add_comm`, `add_assoc`, `mul_comm`, `mul_assoc`,
  `mul_identity`, `mul_distributes`) and the lemmas they cite, and the
  program's law rests only on commutativity and associativity laws of `add`
  and `mul`. Its twin states the same law of an addition the program defines
  itself, which no conformance covers, and is refused.
- **The whole conformance is read.** Three of the seven laws are private to
  the library, so no program can cite them. When a machine that satisfies a
  requirement of a trait is read, stage 05 reads every other machine over
  such data that satisfies a requirement of the same trait at the same data
  (`mathematics.rs`). This is the spec's "consume the complete selected
  conformance's law slots".
- **A machine that returns a value and states a law.** `nat.omg` writes most
  lemmas this way (`mul_succ_right(a, b) -> Nat`). Such a machine is read as
  a definition and as a law. Its statement may name its own result, and its
  citations are the calls, anywhere in the value it returns, of machines
  that state a law; the call of itself at a field of the case is the
  induction hypothesis. A nested machine that states nothing is read in
  place where it is called.
- **Cost.** The twelve library laws that program pulls in took 1,574 to
  13,358 kernel conversion steps each when this landed, about 67,000
  together, every time such a program is compiled, because the library is
  compiled from source with it. The repair recorded below cut each by five to
  eight times.
- **Laws with a premise.** Eleven of `nat.omg`'s lemmas state a `requires`
  equality (`add_cancel`, `le_of_succ_le`, `sub_lt`). A premise is bound in
  the law's statement after its formals, is a hypothesis of each case, and is
  proved at each citation from the case's other evidence in any order. The
  prover takes two facts from any equality it holds: constructors are
  injective (`Succ(x) = Succ(y)` gives `x = y`, which `add_cancel`'s
  successor case needs to cite itself) and distinct (`Succ(r) = Zero` proves
  the case, which `le_of_succ_le` and `pred_eq_succ` need). It also
  normalizes again after each rewrite. With that, the kernel judges every
  law in the file but the three below: 34 laws when the file's declarations
  are compiled as a program (`every_law_of_nat_but_division_and_gcd_is_judged`).
  The largest, `sub_lt_of_le`, took 42,717 kernel conversion steps of the
  65,536 budget when this landed; the next bullet brought the largest down
  to 5,628 (`sub_lt`).
- **Where the kernel's steps went, and the repair (2026-11-24).** Search is
  not the cost: it spends 5 to 20 conversion steps per law. The kernel's one
  check of the finished proof is. One computation, `plus(Succ(m), b)` to
  `Succ(plus(m, b))`, cost 4,512 steps and 56,708 scratch terms, because the
  induction principle carried its step over raw trees inline, and the kernel
  checked that whole step again each time unfolding exposed it in a
  recursion it could not finish. The step, and the step of the
  canonical-tree condition, are now declarations of their own, so the kernel
  reads their types from the signature. The same computation costs 546
  steps. Per law: associativity 11,817 to 2,176, the two distributivity laws
  about 7,320 to about 1,660, commutativity 3,529 to 1,105, the seven laws of
  the library conformance 847 to 2,176 each. Declaring a type: two cases 964
  to 816, four unary cases 59,242 to 10,463. What remains is the price of
  the encoding itself: under raw trees the associativity law took 1,148, and
  one computation is still hundreds of steps where a built-in inductive
  type would take about one. The [native inductive ruling](../../spec/proofs/inductive_profile.md#native-inductive-declarations)
  supersedes the W/IW implementation plan recorded in this audit.
  Each law still re-checks the declarations before it, about 1,000 steps a
  law, which a shared check would remove.
- **`mod`, `div` and `gcd`** were not read at this step: their recursion is
  on `saturating_sub(a, b)`, not on a field of the dispatched case. Step 4
  defines them by recursion on fuel (below); `div` and `mod` have since
  become structural.
- **The two operator machines** were not read at this step. `<=` and `-`
  were written as machines the `Nat` declaration owns
  (`machine <= Nat::less_or_equal`), and compiling the file's declarations
  as a program was rejected with "a data declaration cannot own a machine;
  declare an ordinary static machine". Step 4 makes them free operator
  machines (below).
- **The library is judged only through a program.** A toolchain file is
  never the subject of a check, and most of these lemmas are private, so no
  program reaches them directly; the test compiles the file's text as a
  program. A program that uses the library reaches the conformance and what
  it cites, nothing more.

**Step 4a as built (2026-11-24): products expand over sums.** A proof may
now expand a product of sums into a sum of products under a distributivity law
of the program's conformance, then regroup and reorder both sums and products
as step 3a does. Both sides of a goal reach one polynomial form, and each
expansion is an application of the law in the proof term, so the certificate
names it.

- **Customers.** The recovered `polynomial/pass/ring_full_polynomial_compile`
  states `mul(add(a, b), add(c, d))` equal to its four-term expansion over the
  library's `Nat` and cites nothing. It used to be refused with "no available
  equality proves the stated Identity". The kernel now judges it in 993
  conversion steps, resting on exactly four laws of `core/nat.omg`'s
  conformance: `mul_distributes`, `mul_comm`, `add_comm` and `add_assoc`.
  A Peano twin pair adds a law needing two expansions to the
  `peano_left_distributivity` program: judged in 847 steps with
  `times_distributes` declared as the trait's `mul_distributes`, and refused
  once that `satisfies` line is removed.
- **One mechanism.** `operand_law` also reads distributivity
  (`OperandLaw::Distributes`, from the left or the right, either side first).
  The license needs both operations to satisfy requirements of the law's trait
  application. Stage 05 reads distributivity laws after the commutativity and
  associativity laws and before every other machine, so a distributivity law's
  own proof may still regroup under the first ones, as `mul_distributes`
  does, whatever the source order. Stage 06's `rearrange` gains one more kind
  of rewrite: a product whose sum sits on the side the law states expands; one
  whose sum sits on the other side is first commuted when its operation
  commutes. Expansion runs only after the search without it finds no proof,
  so every law proved before this step keeps the same proof and cost.
- **Bounds.** A product may expand to at most 64 summands, the spec's default,
  counted from the term before anything is built and under the exact licensed
  laws (a law that only expands sums on the right counts only those). A larger
  one stops the proof with `SearchError::Budget`, reported as "ran out of
  proof budget" with no claim that the program is wrong. Every rewrite is also
  charged to the existing 65,536-step search budget. A product of 64 summands
  over Peano is judged in 55,949 conversion steps; one of 128 is refused as
  out of budget.
- **Mutual distributivity.** When two operations each distribute over the
  other, the search tries one direction at a time, so neither law disables
  the other and expansion cannot undo itself. A longer cycle of three or more
  operations is not split and runs into the budget.
- **Satisfier groups** (added after step 4). The recovered
  `polynomial/fail/ring_rearrange_unlicensed_rejected` labels its satisfiers
  with `satisfies Trait<...>::requirement as Name`. The signature now reads
  the name: machines that satisfy one trait at the same type arguments under
  different names are different conformances. The program is refused for
  the three reasons it was written to test, each one law left unproved
  while every other law in it is judged: a law merely in scope, another
  trait application's law and another satisfier group's law license nothing.
- **Not done: configuration and reporting of the limit.** The 64-summand limit
  is a constant. Root Build cannot configure it through the
  [proof-work policy](../../spec/build/configuration.md#proof-work-policy) yet,
  and the diagnostic does not say which limit stopped the proof.

**Step 4 as built (2026-11-24): records, named propositions, hypotheses inside sums, cases inside cases, the rationals, and machines generic over machines.**
`core/int.omg` builds the integers as a record of two naturals, so step 4
starts with records.

- **Records of mathematical data.** Data with a field of mathematical data
  has no layout either, so it and the machines over it now go to the
  mathematical signature. A record with no cases whose fields are all such
  data is the product of their types: a record literal is a tuple, `a.neg` a
  projection, in bodies and in clauses, and a `let` in a body is the term of
  its value. The prover rewrites inside a tuple's components and treats a
  tuple of every field of one value as that value, which the kernel
  converts by pair eta.
- **Order between conformances.** `IntPair`'s laws are polynomial identities
  over `Nat`, proved with no citation under `Nat`'s conformance. Stage 05
  reads conformances in the order of the data they are about, so those laws
  come after `Nat`'s.
- **The search.** It rewrites both sides of a goal, smallest term first, and
  never uses an equality in the direction that nests its own left side.
  Before that, a law with one missing citation ran out of budget where it
  should have reported no proof, and `mul_int_identity`, whose six citations
  read backwards grow terms, did not finish.
- **Named propositions.** `int.omg` declares
  `proposition int_pair_equivalent(left, right) = core\Identity::<Nat>(..)`
  and states one law with it. A clause of a mathematical machine that applies
  a proposition declared as one formula is read as that formula, each
  parameter standing for the argument written for it. The arguments are whole
  expressions (`add_int(a, b)`), so a clause now states a field as the
  field's declaration applied to a value, whatever the value is.
- **A hypothesis used on part of a sum.** Four laws hold an equation between
  two sums and need it on some addends of a longer sum:
  `add_int_respects_eq`, `neg_int_respects_eq`, `int_eq_transitive` (in the
  premise of its `add_cancel` citation) and `mul_int_respects_eq`, where the
  hypothesis is first multiplied by `b.pos` and by `b.neg`. Under a
  conformance that lets the operation regroup and reorder, the prover
  rewrites a chain to hold the rule's left side as one operand, or that side
  times a common factor where multiplication distributes, and uses the rule
  there (`exchange` in stage 06's `equality.rs`).
- **Customers.** `record_law/pass/pair_of_peano` and its twin without one
  field's citation; `sum_hypothesis/pass/proposition_under_addition` and its
  twin without the hypothesis. All fourteen laws of `int.omg` are judged when
  the two library files' declarations are compiled as a program. The largest
  is `mul_int_assoc` at 7,533 conversion steps; the four that use a
  hypothesis on part of a sum take 1,021 to 3,637.
- **Cases inside cases.** `core/nat_metric.omg`, which `rat.omg` builds on,
  proves `le_trans` by induction on `a` whose successor case splits on `b`,
  and that case's successor case on `c`. A case of a law may now call a
  nested machine that dispatches on another formal or on a field; stage 05
  reads its cases as a split inside the case and stage 06 proves each. All
  22 laws of `nat_metric.omg` are judged. They are the most expensive so
  far: `nat_gap_triangle` takes 59,947 of the 65,536 conversion steps a law
  may use and `le_add_gap` 31,474, so a larger proof of this shape will not
  fit until the cost per case comes down.
- **A law with a result is a definition only on demand.** These laws return
  `Nat` only so they can be called as statements. Reading every one as a
  definition too refused `le_trans`, whose body is an argument by cases and
  defines nothing. It is now read as a definition only where a term applies
  it or its statement names its result.
- **Recursion that is not on a field.** `mod`, `div` and `gcd` of `nat.omg`
  call themselves at `saturating_sub(a, b)` or at `mod(a, b)`. The kernel
  recurses only on the structure of a value, so such a machine is defined
  over one more argument, the fuel, by recursion on it, and is that
  definition with its ranked formal as the fuel. Nothing is assumed. The
  price: a law about such a machine is provable only if it holds for every
  amount of fuel, as `mod`'s bound does; a law that needs the fuel to be
  enough, such as `div(a, b) * b + mod(a, b) = a`, is not provable this way
  and is refused. The
  [computed descent ruling](../../spec/language/termination.md#computed-recursive-arguments-in-proofs)
  requires a decrease check at each call and specifies when an internal bound
  is sound.
- **Computed descent (2026-11-27).** Each recursive call of a machine defined
  on fuel now owes `saturating_sub(Succ(next), a) = Zero`, its ranked
  argument strictly below the caller's, as a statement of its own. It is
  stated over the variables in scope and proved from the machine's premises,
  the citations read so far and, new, the equation each enclosing dispatch
  keeps between its subject and the case (`saturating_sub(b, a) = Zero` in
  `mod`'s recursive arm). Premises owed at calls use those equations too. The
  order is recognized by what it computes: a machine read so far whose
  definition is exactly core's `saturating_sub`, over data with one case
  without fields and one with one field, so core `Nat` and a program's copy
  of it qualify and a lookalike does not. Data without one, such as a
  program's own `Peano` with no truncated subtraction, is refused. The
  repeated-subtraction `mod` this audit describes is kept as a public
  compiler fixture (`computed_descent/pass/repeated_subtraction_mod`); its
  twin calling `mod(a, b)` is refused at that call. Still owed: the
  well-foundedness of the order discharged once and a proof that the fuel
  suffices.
- **Structural division (2026-11-27).** Core `div` and `mod` are no longer
  defined on fuel. Both read one structural helper, `divide_step(n, k, q, c)`,
  which recurses on the dividend `n` one unit at a time while counting `c`
  down from the divisor less one and bumping the quotient `q` when `c` runs
  out; `div(a, b)` is its quotient and `mod(a, b)` its remainder. Its laws
  `divide_reconstructs` and `divide_bounds` are proved by induction on `n`,
  so `div_mod_reconstruction` (`div(a, b) * b + mod(a, b) = a`) and `mod`'s
  strict bound hold on every admitted input. The prover now reduces a
  projection of a pair, so a law can read a field of a computed record. Only
  `gcd` still recurses at a computed value, on fuel with its descent checked.
- **Structural gcd (2026-11-27).** `gcd` is no longer defined on fuel
  either. It is `gcd_within(b, a, b)`: Euclid's algorithm with an explicit
  bound on its steps, recursing on the bound alone. The divisor bounds the
  steps because each remainder is strictly below its divisor
  (`divide_bounds`). `gcd_within_agrees` proves by induction on the bound
  that every bound at least the divisor gives the same answer, the source
  form of "the counter suffices" in the computed descent ruling, and
  `gcd_step` turns it into Euclid's step, `gcd(a, b) = gcd(b, mod(a, b))`
  for a positive `b`. `le_trans` joins the order lemmas. Core now has no
  machine defined on fuel. The compiler's fuel definition remains for a
  program's own machine with computed descent; for those, the order's
  well-foundedness and the sufficiency of the fuel are still not proved, so
  their recursive equations are not available to a law. The termination
  spec's "core proves once that this order is well founded" is not yet
  true of the implementation.
- **Fuel sufficiency (2026-11-27).** The fuel of a program's machine with
  computed descent is now proved sufficient
  ([design](computed_descent_recursion.md)). Each unit of fuel is guarded by
  `x_r ∸ rest`, so extra fuel does nothing. Each machine on fuel carries two
  laws stage 06 proves. Its fuel bound says any fuel reaching the ranked
  argument gives the same result. Its recursive equation is the source
  equation under its premises, and a law that calls the machine cites it. A
  law that cites itself at a computed natural is proved by induction on
  fuel that reaches its ranked formal, and a self-citation whose descent is
  unproved is refused at that citation. The order's facts are proved once, in
  core: `succ_of_sub_between`, `not_succ_le_self`, `not_succ_le_zero`,
  `le_of_succ_le_succ`, `not_lt_of_le_zero`, `le_of_lt_of_sub_between` and
  `sub_self`. Each is recognized by its exact statement over the recognized
  order, and an order without one is refused, naming it. This is the
  implementation's form of "core proves once that this order is well
  founded": the kernel has no accessibility predicate, so the step-counter
  route of the spec is taken.
- **A choice by a computed value** (`rat_eq`, `mod`) is a definition by
  cases of its own over its subject and the variables in scope, so the
  prover computes with it as with any other. In a law, a split on a computed
  value keeps the equation between the value and the case.
- **A premise on a definition** (`mk_rat`, `div`, `mod`) is owed at each
  call. `gcd` calls `mod` with a divisor it shows nonzero by citing
  `le_one_succ`; that obligation is a law of its own.
- **The proof search** gained two strategies and one accounting change,
  which together made `rat.omg`'s two largest proofs fit and every earlier
  proof cheaper. Each proof is first tried with definitions folded. When the
  search of every order of rewrites exhausts its steps, the cited equalities
  are completed into rewrite rules and both sides are rewritten to one form;
  `rat_gap_triangle_scaled` is thirteen rewrites that the search could not
  order. And the kernel's check of the finished proof has the law's whole
  budget, with each search on a budget of its own.
- **Measured** with all of `nat.omg`, `nat_metric.omg`, `int.omg` and
  `rat.omg` compiled as one program: 92 laws judged. The largest check is
  `nat_gap_mul_right` at 11,784 of 65,536 conversion steps;
  `nat_gap_triangle` fell from 59,947 to 6,553, `mul_int_assoc` is 7,495,
  `rat_close_triangle_split` 9,327 and `mod`'s bound about 4,300.
- **Machines generic over machines.** `cauchy.omg` states what it is for a
  sequence of rationals to be Cauchy for any sequence and any modulus:
  `cauchy_at<machine Sequence, machine Modulus>`. Such a machine is read
  once, with one more formal per machine parameter, of that machine's
  function type; a call of the parameter applies the formal, and
  `cauchy_at::<Left, Right>(..)` in a clause passes functions. The body is
  read through the clause reader with the machine's contract, each `let`
  standing for its value, so only a body of `let`s, citations and one value
  is read. A clause may call a machine that has a premise where that
  premise is one of the stating machine's own, which is how these laws
  state `cauchy_at` under the bounds it requires. With `cauchy.omg` added,
  the five files give 96 judged laws; its four take 1,297 to 4,674
  conversion steps.
- **Machines passed to generic machines (2026-11-27).** A clause may pass
  a concrete mathematical machine for a machine parameter:
  `twice::<succ>(a)` is `twice`'s definition applied to `succ`'s, and the
  kernel unfolds both. The machine must have exactly the parameter's
  formals and result and no premise, since a machine with a premise is no
  total function. One generic machine may cite another, at its own machine
  parameters or at concrete machines; the cited law's premises are owed at
  the machines it names, so citing `twice_keeps_a_fixed_point::<Other>`
  where only `Step` is known to fix `a` is refused. A body's call at
  concrete machines names the closed copy the typed trees made for it; a
  generic machine's citation reads the selected machines from that copy's
  specialization receipt and cites the generic law. Customers:
  `generic_machine/pass/twice_at_a_concrete_machine` and
  `generic_machine/pass/cited_at_its_own_machine`, each with its twin.
- **Generic machines that dispatch (2026-11-27).** A generic mathematical
  machine's body may dispatch once on a formal, and its laws may argue by
  cases and induction like any other. Such a body is lowered and read by
  the signature reader instead of the clause reader, and so is every
  machine declared inside it. A call of a machine parameter is
  `ExpressionKind::Applied` with that parameter as its head; a call of a
  generic machine passes its machines first. In a definition by cases each
  branch binds the machine parameters itself, ahead of the other formals,
  so the branches stay closed and `FunctionCases` records the subject and
  arity past them; the recursion passes the same machines on. A law's
  `Cases` formal and a split's names count the machine parameters first.
  A body that is not generic may call or cite a generic machine at
  concrete machines through the same form: the call no longer names the
  typed trees' closed copy. Customers: `generic_machine/pass/iterate_keeps_a_fixed_point`,
  `pass/cited_by_an_ordinary_law` and `pass/called_by_an_ordinary_definition`,
  each with its twin.
- **Every generic mathematical body (2026-11-27).** A generic machine may
  call itself at other machines: each branch of its definition by cases
  binds the machine parameters, so the recursion passes whichever it
  names, and a law's induction hypothesis is the law at those machines,
  with its premises owed there. One that recurses on fuel takes its machine
  parameters after the fuel in the definition over fuel, its laws over fuel
  count them among the formals, and a dispatch on a computed value is a
  definition over them and the formals alike. A call of a generic machine
  that has a premise owes the premise, stated over its machine parameters
  and formals, at the machines and arguments it passes. A law reads a
  generic machine that states nothing in place, its machine parameters
  standing for the machines the citation passes; such a machine is
  lowered. Any other body the clause reader cannot read as terms is
  lowered and read like any other body. A generic machine's own `result`
  in its clauses is its definition at its machine parameters and formals.
  Customers: `generic_machine/pass/alternate_keeps_two_fixed_points`,
  `pass/pick_by_a_computed_value`, `pass/call_owes_its_premise` and
  `pass/cited_through_a_generic_helper`, each with its twin, and
  `gcd_then` and `mod_then` over `core/nat.omg` in stage 06's
  `generic_machine` tests, a generic `gcd` and repeated-subtraction `mod`
  on fuel with their twins. A concrete machine with a premise is still no
  machine argument: it is no total function for the parameter to stand for.
- **The order and subtraction operators (2026-11-25).** `nat.omg` declares
  `<=` and `-` as free public operator machines,
  `machine <= less_or_equal(left: Nat, right: Nat) -> bool` and
  `machine - subtract(left: Nat, right: Nat) -> Nat requires right <= left`.
  The signature now has one `bool` entry, a definition equal to the
  kernel's `Two`, declared the first time a formal, a result or a clause
  needs it; `true` and `false` are `TwoOne` and `TwoZero`. A clause that
  uses a mathematical machine with a `bool` result as a proposition, such as
  `requires right <= left`, states that its result equals `true`, as a
  clause already did for an abstract Boolean operation. Each `a - b` then
  owes `less_or_equal(b, a) = true`, which the calling machine's own
  `requires b <= a` supplies. The stage 06 tests read the whole file, so
  both machines are judged with it; neither states a law of its own. A
  program outside core imports both machines to select them by their
  tokens (`nat_order` fixtures).
- **Not done:**
  - `extent.omg`'s contract over `IntervalSet<Nat>` (the plan's
    `extent.omg:219`) is a clause of a runtime boundary trait over generic
    data. It is read by the carrier clause reader when a program reaches
    it, not into this signature; Step 5 serves it there (below).
  - A refused law whose goal names a field inside one case of an induction
    prints `?` for that field.

**Step 5 as built (2026-11-24): one reader for every clause term.**
`clause_term` in stage 05's `read/clause_terms.rs` is now the only function
that reads an expression into a clause term, at every position: a machine's
`requires` and `ensures`, a data or domain predicate, the body of a
transparent `let` definition, a logical invocation input, an argument of a
clause call and an operand of Identity. The caller names the sort it needs
(`ClauseUse`: a carrier value, a mathematical integer, an operand of
Identity or of a clause application, a proposition or a proof-relevant
type), and the clause's `ClauseScope` says what each name denotes there.

- **Readers deleted.** Of the thirteen listed
  [above](#how-a-clause-becomes-a-proof-term-today), `called_term` had
  already gone into `clause_term` in steps 1-4. The other twelve are
  deleted: `term`, `read_term`, `old_term`, `clause_cast`,
  `arithmetic_term`, `mathematical_term`, `read_mathematical_term` and
  `field_term` from `read/contracts.rs`, `mathematical_integer_literal`,
  `integer_embedding` and `logical_mathematical_term` from
  `read/mathematics.rs`, and `LogicalScope::term` (the audit's
  `input_term`), which now only says what a logical input denotes
  (`LogicalScope::input`). `tools/ratchets/clause_term_readers.txt` counts
  functions in `read/` that return a clause term: 23 before this step, 13
  after. Three of the 13 are in `clause_terms.rs` (`clause_term` and the
  `ClauseTerm::scalar` and `ClauseTerm::integer` accessors); the other ten
  read call arguments, static arguments and fixed byte arrays, not clauses.
- **Forms that now work where they were refused.** A transparent definition
  and a logical invocation input read mathematical integers with a reader of
  their own, which refused wholly anonymous arithmetic as "mathematical
  premise contains computational arithmetic". `embed(length) <= 4 * 1024`
  now reads as `embed(length) <= 4096` there too
  (`transparent_definition/{pass,fail}` beside stage 06's
  `address_bounds.rs`). `!b` was read only as a whole clause; it is now a
  Boolean operand as well, so `requires a == !b` works
  (`clause_terms/{pass,fail}` beside stage 06's `tests/clause_terms.rs`).
- **Refusals reworded as the rule.** "embed outside a mathematical
  comparison" is now "embed where a carrier value is needed: it is a
  mathematical integer"; "old outside a callable contract" is now "old in a
  data or domain predicate, which has no invocation entry"; "term other than
  a formal, a literal or an embedded integer" is now "expression with no
  carrier value term"; "term other than a field of the value or a literal"
  is now "name other than the clause's subject, a field of it or a literal".
  `ResultTerm::Absent` keeps its reasons ("result in an entry clause"): no
  result exists there.
- **Kept, not yet known to be a rule.** These still name a position. Each
  may state a real rule, so none was removed:
  - "call in a clause of a state other than the entry" (`unfold_definition`)
    states a rule, and is now refused as that rule (2026-11-27). A call
    "invokes its entry, not a saved activation or a private point inside its
    body", and the source `state` declaration is retired
    ([nested machines](../../spec/language/machines.md#nested-machines-and-identity)).
    A clause's call is a call, so the same rule decides it. Resolution gives
    a call only a machine or its entry state (`one::entry()` and
    `one::helper()` resolve to nothing a call may name), so no source
    program reaches the check. The three copies (`unfold`,
    `unfold_definition` and `call_target_symbol` in `read/calls.rs`, which
    raised "state other than the entry") are one: `clause_call_machine`
    resolves the target through `call_target_symbol`, which refuses a
    non-entry state with the `Violation`
    `rejections::call_inside_a_machine_body` instead of "not supported yet"
    (`a_call_runs_a_machine_from_its_entry_never_a_point_inside_it` in
    `calls_visibility_tests.rs`).
  - "borrowed formal outside an admitted storage clause", "clause that
    projects a formal or result" and "extent without a structural subject"
    are no longer raised for a crash guard (2026-11-25). A published crash
    route is "a canonical disjunction of predicates over the same lowered
    values and places as executable Terminal Psi"
    ([crash](../../spec/terminal-psi/calls_and_outcomes.md#crash)), and
    "declared requirements and published crash routes describe invocation
    entry" ([call contracts](../../spec/terminal-psi/calls_and_outcomes.md#call-contracts)),
    so stage 05 reads a guard in the scope of an entry `requires`. Lowering
    names the leaves a guard reads in the callee and at each call, and a
    call's crash continuations name the storage each structural actual
    stands for, its place and static path (`contract_forms::call_storage`
    and `contract_forms::substituted_routes`, which stage 05 and stage 06's
    retained-row check share). Stage 06 judges a call's route with
    the binding a `requires` uses (`at_call_actuals`), so the guard reads
    the actual's storage when the call is made, and states the machine's own
    coverage over its entry storage (`entry_goal`). The three are still
    raised by the other scopes read without `formal_fields` (an erased
    term, a callee clause unfolded in another clause, data and domain
    predicates, a family machine's value-parameter requirements); whether
    each states a rule is not decided. "Formal field in a postcondition" was
    reached by no scope: only a machine's `ensures` scope has a structural
    result, and it reads formal storage. `read_projection` now answers that
    a formal names no storage wherever a scope reads none, as its early
    return already did, and the message is gone (2026-11-25). In
    `samples/cli/facts/indexed_crash_guard/`, `parameter`, `local`,
    `const_name`, `other_machine` and `target_body` are accepted and their
    twins refused. `field` and `call_result` are accepted too, and their
    twins refused (2026-11-25): the call selects the cell with a value
    equal to the caller's selector but not the same value
    (`selection.index`, or a result with `ensures result == index`). A
    cell of a shared input keeps its entry contents, so stage 06 states it
    over that input wherever no operation captured it
    (`Storage::shared_entry_read`, used by the entry, call and join
    bindings), and also states each `requires` over a shared input's cell
    in that form (`at_entry_over_shared_inputs`). The caller's
    `items[index] == 7` and the call's `items[selected] == 0` then name
    one term once `selected == index` is transported, and the prover bounds
    a term with no value of its own by a literal it equals
    (`literal_equality_bound`). `other_state` calls a nested machine whose
    omitted crash route is inferred as `Truth`. `match_join` and the root
    package now reach the checker: stage 05 no longer refuses the nested
    shared loan in `finish(read_seven(@items, 0, true), @items)`
    (2026-11-25). The call `maybe_zero(items, selected)` after its value
    `match` is covered, since the join keeps `items[index] == 7` stated
    over the input. Its private nested `finish` calls
    `read_seven(items, 0, false)` and owes `items[0] == 7`, which
    [private entry inference](../../spec/language/state_contracts.md#private-entry-inference)
    now supplies: an element of a fixed-length shared input crosses into a
    private helper like a record field, named by the read `items[0]` the
    helper's call states for that cell (`field_crossings`, 2026-11-25), so
    `match_join` is accepted. The root application is accepted
    when checked over its own files, as `sample-gate` checks it
    (`--build-input build.omg --build-input main.omg`). A bare
    `omega --check main.omg` is refused for the place projects' errors, and
    that is the rule, not a capture fault: a package checked with no source
    list is its whole directory, unimported subdirectories included
    ([validation subjects](../../spec/build/validation_subjects.md#admission-and-release)),
    and a nested package owner comes from the admitted package graph, not
    from a subdirectory's `build.omg` (`LogicalSourceInventory::parse`).
    `validation_subjects.rs` pins both
    (`an_explicit_inventory_leaves_a_nested_project_out_of_the_subject`,
    2026-11-27). A route
    over an actual that lends a field (`@pair.items`) names that field
    path at the call, and one over an element chosen at run time
    (`@rows[row]`) names that element, its selector kept as an index
    (`terminal_semantics::canonical_read_projection`, 2026-11-25). A
    forwarder over a `@mutable` formal is accepted while nothing has written
    the array it received (2026-11-27): where the whole input still reaches
    the call from entry, every cell holds its entry contents, so the call's
    route reads `items[index]` in the form the forwarder's own entry route
    states (`Storage::entry_read` with `EntryContents::Unwritten`). A write
    to the guarded element first is refused, and so, conservatively, is a
    write to any other part of the array before the call
    (`a_mutable_forwarder_covers_the_call_only_while_the_element_is_unwritten`
    in stage 06's `indexed_and_member_contracts.rs`).
  - "case membership" in a machine clause (`read_clause`) refused a form
    the specification admits, and is now served where the clause states the
    test outright (2026-11-24). A case is a domain: "A case implicitly
    declares its same-named domain. `Type::Case` therefore denotes case
    membership in domain position"
    ([data](../../spec/language/data_and_literals.md)), and "Contracts may
    state `requires value in Domain` and corresponding `ensures`"
    ([dependent values](../../spec/language/dependent_values.md#mutable-receivers-and-declared-field-domains)).
    Such a clause "proves a fact about `x`"
    ([domain casts](../../spec/language/domains.md#domain-casts)), and a
    payload read needs "established case knowledge", which a callee's
    guarantee supplies. #5606 named other pages on its `Spec:` line (case
    constraints, patterns, structural access); those govern data predicates
    and matches, not this clause. The customers are
    `core/representation.omg`, which writes `ensures result in Result::Ok`,
    and 615 clause lines in 55 files of `samples/` and the library that name
    a case of `Result` or `Option` alone. Stage 05 forms the test of a formal or of
    the result; stage 06 answers "which case does this place hold" at entry
    from the machine's own `requires` and at a call's result from the
    callee's `ensures`, where it used to answer "unknown" (`stated_cases`
    in `places.rs`). A caller proves a required case from what it stored in
    the argument, and a callee proves a guaranteed case where it returns.
    A test beside an alternative (`(result in Result::Err) || ..`), which
    may be false while the clause holds, and a test used as a Boolean
    operand (`Model::valid(..) == (result in Result::Ok)`) are served too
    (2026-11-25). The lowered program names the case of each sum an input or
    a call result holds, one Boolean per declared case, at entry and after
    each call, the way it names each scalar leaf there
    (`ValueOrigin::StorageCase` beside `ValueOrigin::StorageContent`); it
    is a logical name, not a Terminal Psi operand. Where structure does not
    decide a test, stage 06 states it over the name (`CaseAnswer::Named`),
    knows that exactly one of a sum's names holds at each boundary, and
    learns the name from a membership test or a dispatch on the sum. Stage
    05 reads `L == R` over a case test as `(L && R) || (!L && !R)`. A
    payload read still needs the case by structure, and a block formal's
    case has no name yet. The three sample families that write these
    clauses (`resource_representation`, `representation_agreement`,
    `buffered_representation_agreement`) are refused earlier for other
    reasons, so this change moves none of them: `representation_agreement`
    stops at `u` in `let same_result<u: core\Level, ..>`, which
    `BuiltinTypeAtom::is_builtin_type_spelling` takes for an integer type
    because the empty digit suffix passes `all(is_ascii_digit)`;
    `resource_representation` reuses the visible formal name `frame` in a
    pattern (`value as frame`), which state_contracts.md forbids; and all
    three then meet generic and lifetime refusals (a static implementation
    family's binder or region proof, a trait with lifetime parameters).
  - "Identity operand has no term at its written carrier" for a field of a
    record formal outside the mathematical signature is no longer raised:
    stage 05 reads `outer.inner` as the input's storage at that field path
    (#5796). `ensures core\Identity::<Inner>(result, outer.inner)` and
    `ensures core\Identity::<Inner>(result, inner)` now stop in stage 06,
    where a caller restates the guarantee: the completion binding
    (`Returned::whole_input` in `places.rs`) looks the result's root up
    among the formals and refuses with "a completion clause names storage
    that is no formal".
- **A clause reads a reference's referent only through `$` (2026-11-25).**
  `requires value == 7` over `value: @u64` was accepted and read as the
  referent, while `requires $value == 7` was refused ("expression with no
  carrier value term"). That is backwards: "Omega never inserts a
  dereference to satisfy an expected value type", operator operands
  included ([explicit dereference](../../spec/language/expressions.md#explicit-dereference)).
  `clause_term` now reads `$value` as the referent's storage at the
  formal's place, which `requires`, `ensures`, crash guards and `old($value)`
  observe at entry or completion as before, and refuses a bare `value` with
  the rule a body applies (`rejections::implicit_dereference`). The clauses
  that wrote the bare form now write `$`: the boundary trait requirements
  of 14 `target_body` programs under `samples/cli/facts/` and their 14
  `invalid_target_body` twins, whose implementations already wrote `$`;
  three test programs in stage 06; and the examples in `domains.md` and
  language guide chapters 8 and 10.
- **A comparison as an operand (added after step 5).** `flag == (a < b)` was
  refused with "arithmetic contract term has no selected integer carrier".
  `clause_term` now reads a comparison as the Boolean value it computes
  wherever a clause writes one. Propositions are still read by `read_clause`,
  a separate function.
- **`extent.omg`'s interval law at a forwarder (2026-11-27).** The
  contract `|proof|::Identity::<IntervalSet<Nat>>(Granted::content(old(@storage)), Granted::content(@result))`
  on `VacantContentTransfer::transfer` and `ResidentContentTransfer::transfer`
  was already read as a `ContentConservation` guarantee and instantiated at
  each call (`boundary_calls::compositions`). A machine that forwards the
  call and promises the same equation over its own input and result was
  still refused: "content equation needs a proof for operations outside
  unchanged structural forwarding". Forwarding (`forwarding.rs`) now treats
  a boundary call like an owned direct call, and content ancestry
  (`ancestry::boundary_input_origins`) relates the call's result claim to
  the claim it consumed exactly where the boundary declares that projection
  unchanged. A boundary that keeps `Granted` but declares no content law
  relates nothing, so the same forwarder is refused
  (`a_forwarder_restates_the_interval_law_its_boundary_call_declares` in
  `content_projection_domains.rs`).

**Step 7 as built (2026-11-24): a quotient of mathematical data, equal
classes for related representatives, and operations on a quotient.** `data Sign = Representative %
same_sign requires same_sign satisfies Equivalence<Representative,
same_sign> as SameSign;` is read into the mathematical signature when
`Representative` is mathematical data.

- **Formation.** The signature declares the relation as a definition and
  the quotient as the kernel's quotient of the carrier by it. The kernel's
  twenty quotient declarations come first in the signature because the
  kernel numbers them from zero. `value as Sign` in a clause is the class of
  `value`; nothing else makes a value of the quotient.
- **The equivalence is checked, not trusted.** Each row of the selected
  conformance must have proved exactly reflexivity, symmetry or transitivity
  of this relation. The twin `quotient/fail/symmetry_of_equal_values_only`
  gives its `symmetric` row one more premise and is refused for that row.
  The comparison matches premises in any order: a machine's `requires`
  clauses reach the signature in canonical order, not as written.
- **Equal classes.** `core\Identity::<Sign>(a as Sign, b as Sign)` is proved
  from a proof of `same_sign(a, b)` by the kernel's soundness assumption.
  The fixture relates two numbers when both are zero or neither is, so the
  law equates the classes of different numbers; its receipt counts three
  assumptions (the quotient type, the class of a value, soundness) and its
  check takes 932 conversion steps.
- **Operations.** A machine whose body is `Quotient::lift` or
  `Quotient::define` is an operation on classes given by an operation on
  representatives. The correspondence judgment is two statements the
  compiler writes itself and compares with what the selected theorems
  proved. The congruence: for two representatives of each class and one
  value of each other argument, related arguments and both calls allowed
  give related results (equal results when the operation returns no class).
  The transport: what the public machine requires of the classes gives what
  the representative operation requires. `lift` with two arguments and
  `define` select no transport theorem, so that statement is proved by the
  compiler with no citation, and `define` proves the converse too. The pair
  recovered from the old corpus is `quotient/pass/lifted_operation` and
  `quotient/fail/lift_selects_another_theorem`, whose lift names a proved
  theorem that is not the congruence.
- **Lift arguments (2026-11-27).** `Quotient::lift` passes each parameter
  of the operation on representatives a formal as it is, or a value it
  computes from the formals (a constant among them), so a formal may be
  left out or passed twice. The congruence is stated of the operation's
  parameters, so a computed argument is one value shared by both calls,
  and a class formal passed twice is two class parameters, each with its
  own two representatives and relation premise. The definition passes the
  one class's representative to every class parameter and cites the
  congruence's relation proof once per parameter. Only a formal is a class:
  a computed argument of the quotient's type is refused, so `lift` always
  eliminates a class the machine was given. A transport theorem is stated
  over the congruence's binders, which reach no computed or left-out
  formal, so such a lift selects none and owes that statement itself over
  its formals; `define` still passes each formal in its place. Tests:
  `an_operation_lifted_with_a_computed_or_omitted_argument_*`,
  `an_operation_lifted_with_a_repeated_class_computes_at_a_class`,
  `a_repeated_class_needs_a_congruence_for_each_pair`,
  `a_lift_that_computes_an_argument_owes_its_transport_itself`,
  `a_computed_argument_is_never_a_class`.
- **Not done:**
  - A lifted operation is a definition only where the kernel's `lift`
    gives one (2026-11-27): one class in, a class or plain data out (the
    quotient is a set by `setQ`, plain data by Hedberg's theorem), and an
    operation on representatives that requires nothing. The definition is
    `lift` of the operation at that class, with the selected congruence as
    `respect` (through soundness when the result is a class). A proof
    rewrites it with `lift`'s point computation, a theorem the scheme now
    derives from `beta` (`QUOTIENT_LIFT_BETA`), so no conversion rule is
    added. An operation that requires something of its representatives
    needs the dependent eliminator, and one over two classes needs
    function extensionality; a term that applies either is refused by
    name, as is one returning `bool`.
  - Anything that needs the kernel's `effective` assumption (equal classes
    give related representatives). The specification allows it only for a
    relation shown to be a mere proposition, and requires the declaration to
    [select a closed `MereEquivalence`](../../spec/proofs/quotients.md#formation-and-representation).
    The reader still needs to consume that conformance.
    The compiler uses soundness only, which holds for any relation.
  - A generic quotient, and a quotient of data that has storage in the
    mathematical signature. The signature now declares generic data that
    contains itself, with payloads (#6295), but a quotient's reader still
    refuses generic quotient data. It states a record only of mathematical
    data, never one with an `i32` field, so a law equating classes of
    stored data is refused as "Identity of classes of a quotient of data
    that has storage"
    (`a_law_over_classes_of_stored_data_is_refused_by_name`). Ordinary
    programs run a quotient of copyable,
    custody-free data stored as its representative: all nineteen programs
    under `samples/cli/facts/quotient_closed_literal/` pass the sample gate
    on Linux x86-64 (2026-11-27), each invalid twin refused because its
    `4i32` misses `representative`'s `amount == 3i32`. The reader names
    the selected trait by its declaration, so `|relation|::Equivalence`
    selects it, and package review reads the stored representative's type
    from the declaration's carrier.

**Step 6 as built (2026-11-24): data indexed by values, theorems over it and
the machines that run with its evidence.** The three `source_indexed_*`
sample families compile and run: all 27 valid placements are accepted and
all 27 invalid twins are refused where the wrong value is built. The plan
row's "evaluated in stage 03 first" did not hold; the samples already passed
stages 03 and 04. What is built:

- **Two kernel pieces came back.** The indexed-data builder
  (`psi/semantics/proofs/mathematical/indexed_data.rs`, ported from the
  deleted elaborator) turns a description of a declaration into the five
  functions of an indexed W family, constructor values and the step of an
  induction. The integer search (`SourceIntegerTheory` in
  `psi/semantics/proofs/bounded/bounded_denotation.rs`, removed by #2943 as
  uncalled) declares an integer vocabulary in a caller's signature and finds
  kernel proofs of integer goals from hypotheses; a later stage can resume it
  against a vocabulary an earlier stage declared. Its one new trusted law,
  that a product distributes over a sum, is tested against computed
  arithmetic.
- **The family.** `data Vector<Length: u64, Total: u64> requires ..` with an
  erased field of its own type at other values is read as a family: indices
  and payload values are integers, each case's label carries a proof of what
  the `requires` say of that case, and each child sits at the index its
  field's type names.
- **The theorem.** `vector_bound` is stated for every index and every value
  of the family and proved by induction over the value. Each case is a goal
  about integers that the integer search proves from the case's constraints
  and the theorem at the children the step cites. The kernel checks the
  assembled induction once: 1,326 conversion steps for the vector's bound,
  1,615 for the two-child tree's, with 16 assumptions (the integer
  vocabulary and the integer laws used).
- **Refused, each by name:** the bound `9 * Length` (the `Cons` case has no
  proof); a step that does not cite the theorem at the tail; a step that
  requires more than the data gives; a citation at another index than the
  child has.
- **Machines that run with evidence.** Evidence has no layout, so a machine
  that runs holds an erased Boolean in its place and the mathematical reader
  checks what the machine does with it. Each case literal owes what the data
  requires of that case, as a law about integers the kernel judges: a list
  whose head is eleven, or whose total is not the sum, is refused at the
  literal. Evidence is passed and held only at the index it has, compared
  as written. A machine cites a theorem by calling it, and imports the
  theorem's guarantee at the call's values from the call on, as the
  [citation rule](../../spec/proofs/contracts.md#citation-and-induction)
  says. The citing machine's `requires` stays as its author wrote it
  ([private entry inference](../../spec/language/state_contracts.md#private-entry-inference):
  "An authored `requires` is complete"). The theorem's own machine has no
  statements, and stage 06 takes the signature's proof of the theorem as
  the proof of its guarantee, so the guarantee is never assumed: with no
  proof of the theorem, the program is refused. (Until 2026-11-24 the
  guarantee was instead added to the citing machine's `requires`, which
  the specification does not allow.) The citation emits no call: it is a
  theorem application that names the theorem's machine, a jump that owes
  the theorem's `requires` on its edge and has its `ensures` after it.
  Terminal Psi lets an application name a machine only when that machine's
  body does nothing.
- **A type parameter beside the indices.** `Vector<T [copy], Length, Total>`
  with a field `marker: T` is the same kernel family as without `T`: no
  clause can say anything of a value of type `T` and no index depends on
  one, so the family leaves `T` and the field out and the theorem holds
  whatever `T` is. Stage 03 already refuses evidence built or passed at
  another type ("constructor or call arguments determine conflicting
  generic arguments"), so stage 05 compares only the indices.
- **One repair outside the proof layer.** A mathematical definition names
  its value binders inside its types (`let retain<Length: u64, Total:
  u64>(value: Vector<Length, Total>)`). Stage 02 read such a name only as a
  type and left it unresolved, and the package review then refused the whole
  sample ("a stale declaration in a carried type"). Stage 02 now binds a
  name written where applied data takes a value to the definition's binder.
- **The rank.** A theorem that cites itself states the index it is ranked
  on, and each cited child's value of it is shown to be smaller. For the
  tree that needs `0 <= right`, so each case carries, for this check only,
  that its unsigned values are not negative, and the integer search learned
  two steps: `a <= a`, and a sum is at least one part when the other is not
  negative. The second rests on two new trusted identities, `a + 0 = a` and
  `0 + a = a`, each tested against computed arithmetic as the distributive
  law is.
  These three laws fall under the rule for
  [admitting an arithmetic rule](../../spec/proofs/foundation.md#admitting-an-arithmetic-rule)
  (checked against it 2026-11-24). #5554 and #5587 name the programs that
  need them, none has a side condition to state, and each is tested against
  computed arithmetic. Each landed with other changes, where the rule asks
  for one per pull request. All the integer model's assumed laws, these
  three and the 39 older ones, are now listed with their statements
  in the [kernel metatheory](../../spec/proofs/kernel_metatheory.md#assumed-laws-of-the-integer-model);
  the [rule table](../../spec/proofs/classicality.md) lists only the rules a
  certificate can cite, and none of these is one. Since 2026-11-27 every
  one of them, the older ones included, is evaluated as declared against
  computed arithmetic with a wrong twin beside it
  (`bounded_denotation/law_arithmetic.rs`); the ones the earlier tests named
  for `DistributeAdd`, `ZeroRight` and `ZeroLeft` no longer exist on `main`,
  and the new test covers those three as well. A theorem may also rank
  on its evidence (`terminates by evidence`), as the specification lets a
  proof "rank on mathematical structure". It then owes no arithmetic at its
  citations, because a step cites it only at a child of its case, a part of
  the evidence; a step that cites it at the whole evidence is refused.
- **A quotient and indexed data in one program.** Both rest on declarations
  of the kernel's own. The quotient's come first and the indexed scheme's
  five follow, each constant moved to where the scheme starts
  (`indexed_scheme_at`, `IndexedFamily::base`).
- **Not done, each waiting on something outside the proof layer:**
  - A computed index (`Vector<Length + 1, ..>`). The compiler refuses any
    operator in a static argument of a generic machine, with or without
    indexed data, in two places. Typed preparation defers the open machine
    itself (`outer<Count>` holding `inner::<Count + 1>()`: "using a type
    written as an expression, which no pass evaluates yet"), because nothing
    evaluates a static value that names an open parameter. And in the copy
    made for a closed value, lowering leaves the `+` of `1 + 1` unanswered
    ("admitted source selection Operator remains unresolved as
    CheckedOperator"), because the folded value is read through the typed
    trees and the written calculation is never read as an expression. A
    trial that completes those operators where the call reads its static
    application (`static_machine_application_arguments` beside the folded
    bindings) made the closed copy check and refused a value past the
    callee's requirement; the open machine's deferral then refuses the
    program. When both work, the evidence reader must read a sum as an index
    and prove two indices equal where it now compares them as written.
  - Evidence held in a field of other data: the ordinary reader refuses any
    data with an erased field.
  - Evidence is not a machine's result. A result is a value that runs, and
    the compiler refuses a runtime use of an erased binding; a definition
    that returns its argument (`retain`) hands evidence on instead.
  - A case the `requires` say nothing of, and a theorem body of any other
    shape than one dispatch whose arms call a step.

The first version of this plan started with level binders and value-indexed
type arguments because they move 66 samples. They move those samples to
their next failure. The library needs neither, so they no longer lead. The samples' other blockers (domain
surface, imports, stale syntax, references) belong to their own `TASKS.md`
items.

## Open questions

The [record identity ruling](../../spec/proofs/contracts.md#record-identity-from-field-identities)
settled the earlier question about leaf equalities. A concrete record's field
proofs can be assembled automatically into one checked kernel `Id` proof.
Slice 7 of the [proofs plan](proofs_evidence_lane.md) concerns executable
comparisons.

Recorded questions and where each stands:

1. **Level binder spelling.** The [builtin naming rules](../../spec/language/modules.md#builtin-type-names)
   allow `u: core\Level`. Stage 03 still needs to resolve `u` in `core\Type<u>`
   as a level argument for `representation_agreement`.
2. **Where the signature lives.** Not an owner question: [OWNER_QUESTIONS.md](../../../OWNER_QUESTIONS.md)
   delegates Psi representation and encoding design and counts a schema
   change needed for accepted behavior as implementation work. Terminal Psi has `mathematical_carriers`,
   `identity_formations` and `indexed_theorems` slots. Stage 05 writes
   carriers for theorem parameters (`read/theorems.rs`) and stage 06 writes
   one for each data declaration; nothing writes the other two. Is R1's
   signature table those slots, or a new table? The spec
   requires publishing "the full level telescope and constraints"; no level
   constraint encoding was found. L3 proposes one table in the lowered
   program for every declaration kind and leaves the wire tables as the
   spec has them. The level constraint encoding remains open. Which row a
   judged law over plain data publishes is no longer open: the owner decided
   on 2026-11-23 that such laws are not published (step 2's publication
   bullet above says why).
3. **Floor scope.** Decided by the spec; the compiler does not do it yet
   (`TASKS.md` **REQUIRED-AUTOMATIC-DISCHARGE**).
   [contracts.md#required-automatic-discharge](../../spec/proofs/contracts.md#required-automatic-discharge)
   gives an operand "the tightest literal bounds order reasoning gives it",
   and order reasoning includes equality substitution, so an operand proved
   equal to 360 lands on [360, 360] (`(prefix - 360) + last`, S06 README,
   "An operand fixed by its bounds, used as a literal"). The 64 limit counts "arm combinations per
   obligation" of the disjunctions that "share a subject with the goal",
   not every branch visited, as `prove.rs` does.
4. **Float ranges.** Decided by the spec: nothing to build. The floor's
   order reasoning "compares two integer terms" and its range propagation
   covers "each integer value"; no page requires interval propagation
   through float arithmetic, and "A checker may discharge more".
5. **A `requires` that can never hold.** Mostly decided. Soundness does not
   depend on it: "contradiction of an unreachable path" is in the floor,
   and every caller must still prove the impossible `requires`.
   [state_contracts.md](../../spec/language/state_contracts.md) already asks
   for an advisory warning on an authored branch that contradicts the entry
   contract. Whether a whole machine with a contradictory `requires` also
   gets a warning is an optional diagnostic choice.
6. **Boundary calls while storage is vacant.** Answered. The
   [ownership ruling](../../spec/language/ownership.md#boundary-calls-during-restoration) admits a call when its contract
   rules out exposure and accounts for restoration. Blocking alone is permitted.
   The checker now applies that contract check in place of the blanket
   refusal recorded [above](#what-the-line-by-line-read-found).

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
- Third reading (2026-11-23), after the question of whether the plan rests
  on the deleted code and corpus: read `check_on_lowered_form.md`,
  `route_one_measures.md` from `76bce12121^`, the module outline of
  `contract_entailment.rs` from `cea9cdcde460^`, the corpus file list from
  `f4c7a70beb^` and the two `ring_law` fixtures. Corpus counts are of
  directory paths containing a keyword and are approximate. It added the
  deleted Identity prover and the corpus section, corrected the claim that
  the library needs no `data` type in the kernel, and reordered the plan.
  Of the 861,818 lines #2823 deleted, this audit has read the proof crate,
  `quotients.rs` and the outline of `contract_entailment.rs`; the rest is
  unread.
- Fourth reading (2026-11-23), of the whole deletion and the whole corpus.
  The deleted crates were extracted from `cea9cdcde460^` (640 files, 881,353
  lines of Rust) and the corpus from `f4c7a70beb^` (51,626 files). Ten
  readers took the deleted code by crate and eight took the corpus by
  category; each reported the line ranges or rows it read in full. About
  130,000 deleted lines were read in full and every module outlined; every
  corpus fixture's index row was read and about 1,600 programs opened.
  Whole-set counts (kernel references, `bool` functions, the placement
  matrix, clause forms, ledger rows) are searches over every file. Six
  claims about the deleted code and the ledger row counts were checked
  directly against the files. Counts by group are from name and keyword
  patterns and are approximate. Nothing was built or run.
- Fifth reading (2026-11-23), line by line. The deletion was cut into 137
  slices holding 875,311 lines; one reader per slice read its ranges in
  order and logged each read, and `verify.py` in
  [the full read](../reference/deleted_checker_full_read/README.md) finds
  every range logged: 875,311 of 875,311 lines. Thirteen reports shortened
  file names in their logs; the script lists each mapping, and without them
  it counts 795,020 lines. A reviewer found that the script first committed
  lacked the mappings and pointed at a scratch directory. Seventeen readers merged the reports by group and
  seventeen more looked up each group's rules in the current tree at
  `b8213143d790`. Readers ran on two model tiers; the proof crate and the
  contract prover went to the stronger one. The author of this audit read
  every cross-check's summary, "not found" and "unclear" sections and every
  synthesis's mechanisms, properties and patterns sections, and did not
  read the 17 rule inventories row by row. The counts of rules (1,366 and
  its classes) are sums of the cross-checks' own summaries. The groups
  overlap, so one rule can be counted more than once and classed
  differently: content conservation is "not found" in one cross-check,
  "unclear" in two and "refused" in a fourth. The rows marked
  confirmed in [old faults that remain](#old-faults-that-remain-in-todays-compiler)
  were read in the current source for this audit. The counts in the
  patterns table are the readers' and were not recounted.
- Probes (2026-11-23): `CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p omega`
  at `b8213143d790` plus this branch's documentation commits, on the cloud
  session's `linux_x86_64` container, then `omega --check --target
  linux_x86_64 main.omg` on thirteen two-file programs kept outside the
  repository. An empty `main` compiled, and a call that breaks its callee's
  `requires` was refused with the expected message; these two are the
  controls. Results for duplicate parameters, duplicate declarations, a
  type-argument count and a Boolean statement are in
  [the table of rules not found](#what-todays-compiler-does-with-the-old-rules).
  The duplicate-parameter program was also run with `omega run --engine
  psi` (Psi exit 0); it was not run natively.
- Second round of probes (2026-11-23), same binary, 24 more programs, each
  checked with `omega --check --target linux_x86_64`. The qualification
  probes: a pass-through and a forged cast as controls (accepted, refused);
  a field write on a qualified user record, through an owned local and
  through `@mutable` (both accepted); the same on `Extent in Granted` with
  `base` 4096 and `length` 0, then 4096, and a growth in place from 16 to
  8,192 (all accepted); a `length` that breaks `no_wrap` (refused, which
  shows the machine was judged); an unused forging machine (refused, which
  shows unused machines are judged). The vacant-storage probes: eight programs,
  most with one call between the move and the restoring store. The process-exit probes were
  run with `omega run build.omg` on both engines: Psi exit 7 and native
  exit 7 for the honest provider, Psi exit 0 and native exit 0 for the one
  that passes 0. The view probe is a pair that differs in one call. Only
  `linux_x86_64` was tried. A program rooted at `main.omg` does not read
  the Build rows, so provider probes must be rooted at `build.omg`.
- Repairs (2026-11-23), commits `abce222c1b`, `d090365e17` and
  `5bc6119e59` on this branch. Each commit body states its tests. In sum:
  `mbx test` on the stage 02 crate (143 passed), the stage 06 crate (1,329
  passed, 2 ignored), `terminal-semantics` (311), `terminal-wire` (180),
  the stage 05 crate (212) and `omega --test process_exit_operand` (4), with
  `mbx clippy -D warnings` on each changed crate. The probe programs were
  rerun against the repaired compiler: every program that should be refused
  is, at its source position, and every control still compiles. 47 probe
  programs exist in all. `mbx xtask fmt --check` fails across the workspace
  on `main` without these changes, so it was not used as a check. Nothing
  was run on a target other than `linux_x86_64`, and no full-workspace test
  run was made.
- Corpus sample (2026-11-23): 60 directories drawn at random (seed 7) from
  the 8,038 under `fail/` at `f4c7a70beb^`; each fixture's `.omg` files
  were copied beside a current `build.omg` that binds `main`, and checked
  with `omega --check --target linux_x86_64`. Outcomes are grouped by the
  first error line. The replaced `build.omg` drops any dependency the
  fixture declared, which is one more reason a refusal here does not test
  the fixture's rule.
- Two reviews by other models (2026-11-23) were checked claim by claim
  against the source, and these corrections come from them: the spec does
  forbid a repeated parameter name; `ElementViewWrite` is missing from the
  scalar store validator; the repair of `complete_operator` must not refuse
  type predicates; the three mathematical tables are ratified wire format,
  so L3 moves to the lowered program; local call summaries are settled by
  the spec; two laws in `core/nat.omg` need reordering under proved laws;
  the recovered negative fixture tests only a missing `ensures`; and the
  committed `verify.py` did not reproduce the coverage count.
- Not determined: whether a full core library check with `no_wrap` passes
  end to end (no sample reaches it); `erased_array_evidence`'s signal 9;
  whether old level constraints were ever published; the two samples bound to
  other targets; which samples compiled before #2823; whether the old checker
  accepted the 15 `(prefix - 360) + last` programs; whether any program in
  use reaches one of the nine untranslated certificate shapes; whether the
  33 certificate rules overlap.
