# Record: unit 044, second pass gap_025 (area 07-psi-semantics)

Unit file: `units/gap_025.tsv`, one row: `source-rust/subsystems/compiler/psi/semantics/proofs/mathematical/tests.rs`, lines 1-5282.
All paths below are relative to `source-rust/subsystems/compiler/psi/semantics/proofs/mathematical/` (called `M/`) unless they start with `source-rust`.
I did not read the crate map section beyond locating this crate (grep "mathematical", crate-map/07-psi-semantics.md:66-105): this is the `mathematical-kernel` crate's unit-test file.

## `M/tests.rs` (5282 lines, read in full)

### 1. Purpose
Unit tests for the mathematical kernel, the dependently typed core (sorts, Pi, Sigma, Two, identity, W-types, squash/box strict layer, level-polymorphic constants, quotient and indexed-W schemes) that judges mathematical certificates. The file pins what the kernel accepts and the exact `CoreError` it gives for each refusal. It also pins that deep terms (131,072 nested nodes) are handled on the default thread stack.

### 2. Shape
- Wiring (grepped): `M/mathematical.rs:16-17` declares `#[cfg(test)] mod tests;`. The file has no `use` for its own top-level items inside the submodules; each submodule imports helpers from `super` or `crate::tests::` and kernel items from `crate::`.
- 14 inline test modules, 88 `#[test]` functions (Grep count mode on `#[test]` = 88):
  - `conversion_judgments` (1; 5 tests): `convertible`/`convertible_retaining` form both operands first.
  - `assumption_closure` (240; 2): `certificate_assumption_closure`, `judgment_assumption_closure`.
  - `identity_and_w_types` (314; 11): `Id`/`Refl`/`IdElim`, `W`/`Sup`/`IndW`.
  - `level_worklists` (1070; 3): deep `Level` chains, derived `Hash`/`Debug`, `instantiate_levels`, `levels_equal`.
  - `levels_and_declarations` (1218; 6): polymorphic identity, `check_signature`, constants, retained carrier aliases.
  - `normalization_worklists` (1496; 4): `weak_head_normalize`, `structurally_equal`.
  - `schemes_and_quotients` (1657) with four submodules: `indexed_schemes` (1658; 3), `quotient_derived` (1877; 5), `quotient_receiver_policies` (2190; 3), `quotient_schemes` (2622; 5).
  - `spine_chains` (2909; 16): long lambda/pair/application/snd/case/binder chains.
  - `strict_layer` (3533; 10): `Empty`, `Squash`, `Box`, strict Sigma.
  - `substitution_worklists` (4146; 6): `shift`, `substitute`, `instantiate_levels`, `required_bindings`.
  - `type_checking` (4391; 9): unbound variable, step ceiling, Sigma sorts, pair eta, `CaseTwo`.
- Shared helpers at file scope (4723-5282), all taking `&mut TermArena` and returning `TermHandle` unless noted: `type_sort` 4729, `strict_sort` 4733, `variable` 4737, `pi` 4741, `lambda` 4749, `apply` 4757, `sigma` 4761, `pair` 4769, `fst` 4773, `snd` 4777, `two`/`two_zero`/`two_one` 4781-4791, `two_identity` 4793, `case_two` 4800, `id` 4815, `refl` 4824, `id_elim` 4828, `w_type` 4843, `sup` 4847, `ind_w` 4862, `empty`/`empty_elim` 4871-4875, `squash`/`squash_intro`/`squash_elim` 4879-4887, `boxed`/`box_intro`/`box_elim` 4900-4908, `default_budget` 4921 (`Budget::new(DEFAULT_CONVERSION_STEPS)`), `universe_motive` 4925, `branching_family` 4931, `symmetry_motive` 4943, `w_step_binding` 4959, `w_context` 4989 (8-binding context with carrier, B, A, k, P, step, t, u), `concrete_step` 5023, `sort_level` 5043, `constant` 5047, `polymorphic_identity_declaration` 5054 (returns `Declaration`), `two_indexed_family` 5068 (returns `IndexedFamily`), `scheme_signature` 5106, `indexed_context` 5112, `two_quotient_over` 5189 and `two_quotient` 5208 (return `QuotientFamily`), `equivalence_hypotheses` 5215 (four closed hypothesis types), `equivalence_proofs` 5264 (`EquivalenceProofs` from de Bruijn index), `quotient_signature` 5273, `is_set_zero` 5279.
- Local macro `conv!` (identical, 325, 3543, 4401): rewrites `a => b => c => d => e => f` to `convertible(a,b,c,d,e,f)`; used only to keep long calls on one line.

### 3. Inputs and outputs
- Reads the kernel's public surface through `crate::`: `Budget`, `Context`, `CoreError`, `Declaration`, `Level`, `Sort`, `Term`, `TermArena`, `TermHandle`, `Signature`, `check_signature`, `check_type`, `infer_type`, `convertible`, `convertible_retaining`, `weak_head_normalize`, `indexed_scheme`, `quotient_scheme`, `MathematicalCertificate`, `verify_mathematical_certificate`, `certificate_assumption_closure`, `judgment_assumption_closure`, `assumption_closure`, and constants `INDEXED_W`, `INDEXED_IND`, `QUOTIENT*` (imports at 6-9, 242-246, 320-322, 1663-1666, 2199-2205, 2627-2632).
- Also reaches into private modules: `crate::term::{instantiate_level, levels_equal}` (1177), `crate::substitution::{instantiate_levels, shift, substitute}` (4148) and `crate::substitution::remapped_nodes()` (4229, 4253, 4257), a counter that exists only for tests (`M/substitution.rs:5,10,179` carry `#[cfg(test)]`; grepped, not read).
- Calls `record_storage::{encode, decode, RecordLimits}` once (4220-4222) to round-trip an arena.
- Produces only assertions. Mutates only local arenas. 7 tests spawn a thread (`std::thread::spawn(...).join()`) so deep structures run on a default-size stack rather than the test harness's.

### 4. How it decides (what the tests pin the kernel's judgments on)
Tests assert judgments keyed on term structure and arena handle identity, not on names:
- Conversion forms both operands against the shared type before any reduction or irrelevance (11-91). The error kind distinguishes a bad operand (`ArgumentTypeMismatch`, 42) from a bad shared type (`NotASort`, 50) and a non-proof under a squash proposition (`TypeMismatch`, 82).
- `convertible_retaining` keeps definitions the predicate names as opaque (116-144, 1430-1493): the predicate is a closure on declaration index, here `|declaration| declaration == 0` (130) and `declaration < 2` (1430). Judged on declaration index.
- Level arity is checked before reflexivity (149-196): a malformed `Constant{declaration:0, levels:[Parameter(0)]}` against arity 2 returns `DeclarationArityMismatch` even for `bad == bad`.
- Budget: each conversion question gets its own step budget and the remainder is asserted (229-236, 1524-1534, 1565-1575, 1598-1607, 3198-3206, 3238-3257). Neutral spines cost zero steps (1574).
- Strict versus relevant layer: strict-sort carriers are refused for `Id`, `Refl`, `W`, `Sup`, `Squash`, `Box`, and as motive codomains (`StrictIdentityDomain` 392, `StrictIdentityMotiveCodomain` 605, `StrictWCarrier` 703, `StrictWChildrenCodomain` 725, `StrictInductionMotiveCodomain` 889, `StrictSquashDomain` 3526/3641, `NonStrictBoxDomain` 3515/3849, `StrictCaseMotiveCodomain` 4691, `SquashTargetNotStrict` 3756/2336).
- `Dummy` term: `structurally_equal(dummy, dummy)` is false (1634-1637); `infer_type`/`check_type` return `DummyTerm` before looking at a later component (3094-3119, 3209-3219).
- Quotient tests pin that `QUOTIENT_*` assumptions are exactly seven named declarations (2809-2834) and that every other declaration needs a checked body (the comment says "the kernel cannot grow an unlisted axiom", 2812). The assumption-closure sets for a certificate are compared to expected `BTreeSet<u32>` of declaration indices (2413-2419, 2577-2619).
- `effective` requires four equivalence proofs (propositional, reflexive, symmetric, transitive); with the relation `R := fun _ _ => Id Two zero one`, the bare form is refused (2473-2479) and checks only under hypotheses (2501). Doc comment at 2424-2428.

### 5. Cases by shape
- The tests themselves are fixed-shape: the quotient and indexed families are always over `Two` (`two_quotient`, `two_indexed_family`), and the W family is `w_context` over a fixed 8-binding context. No test builds a quotient over any other carrier except the `squashed_two_quotient` (2207) and the absurd-relation family (2433), both still over `Two`.
- `Level::Constant` levels are always 0 in scheme tests (`Level::Constant(0)` throughout 1692-2620); level-polymorphic instantiation is tested only in `levels_and_declarations` (1218-1396) and `substitution_worklists` (4351-4389).
- Depth constants: `DEEP = 131_072` (1076, 2915) and `SHALLOW = 4_096` (2916); also literal `131_072` appears 10 times (Grep count on `DEEP: usize|131_072`). 4,096 is used for constructors that recurse through binder prefixes; 131,072 for single-path chains. No comment explains why these two numbers (inferred: they are chosen to overflow a default stack if recursion returned).

### 6. Same job elsewhere
- `source-rust/subsystems/compiler/psi/semantics/proofs/bounded/tests/` holds a parallel set of indexed/quotient tests (`bounded/tests/indexed_derivation.rs`, `indexed_mutual.rs`, `indexed_relations.rs`, `common/mod.rs`; seen only in grep for `argument_error_pair_pattern`, not read). Suspected: similar term-building helpers (`variable`, `pi`, `lambda`) exist there; I did not read them.
- `M/certificate.rs:214` and `:696` and `M/typing.rs:3854`, `M/term.rs:814` each hold their own `#[cfg(test)]` modules (grepped). Not read here.
- Within this file the same helper-building pattern repeats: the 4-line `let x = variable(&mut arena, n); ...` hand-built de Bruijn terms are repeated hundreds of times; helper functions `w_step_binding` (4959) and `concrete_step` (5023) build near-identical Pi telescopes with different indices.
- The checks "`carrier` is strict -> `Strict*` error" repeat across Id (384-398), W (689-703), Sup (766-773), Squash (3630-3647), Box (3838-3855).

### 7. Unused or one-caller code
- `crate::argument_error_pair_pattern!()`: used in this file 4 times (1833, 1840, 1847, 2873). Grep count across `source-rust/` = 14 occurrences in 7 files (includes its own definition site(s) in `bounded.rs` and `M/typing.rs`). It is a macro defined outside this file; not otherwise assessed.
- Helpers possibly used once: `strict_sort`, `snd`, `fst`, `universe_motive`, `branching_family` have uses in several modules; I did not grep each. The `use` lists at 3-4 and 1659-1662 import helpers by name; any helper imported but unused would be a compiler warning, so none are expected (inferred, not checked).
- `Budget`, `Signature`, `Context` imports in each submodule are the full set the module uses (inferred).

### 8. Stated limits
- 2811-2813: "admit exactly seven named assumptions; every other quotient declaration must carry a checked body".
- 227-228: "The shared type's inferred sort takes one beta step. Neither checking its two inhabitants nor comparison owes that step again."
- 1048: "The level-0 W is not an operand at the requested level-1 type."
- 1257: "Both compared Type(u+1) universes inhabit Type(u+2)."
- 1203: "instantiation preserves explicit syntax for wire-depth admission" (a stated consumer of `instantiate_level`, not tested further here).
- 3205-3206 and 1574: "inference does not evaluate the lambda body", "inspection is not a reduction step".
- 2185: "pointwise uniqueness is not function extensionality" (test asserts `unique` does not give a function identity).
- 1194: "syntactic equality must not become semantic conversion"; 1650: "structural comparison does not silently become semantic level conversion".
- No TODO, FIXME or "unsupported" text in the file (read; not a Grep run).

### 9. Fail-open risks
Test file; it accepts nothing in production. Points where a test asserts that something IS accepted and would hide a fail-open if wrong:
- `convertible` returns `Ok(true)` for any two proofs of a squash proposition (64-91, 3714-3724) and for any two `squash_elim` results with different functions (3743-3749). Stated (test names: "squashed proofs are irrelevant", 3670). Traced for the wrong-input half: values (non-proofs) are refused with `TypeMismatch` (66-68, 82).
- `convertible_retaining` with a predicate `|_| true` (188) is used only to check arity refusal. Not a risk.
- 2603-2606: a "quotient-refusing" policy is modeled in the test as the empty `BTreeSet`, and `is_subset` is asserted; the production policy that consumes the closure is not in this file (inferred from the names; the consumer is in `M/certificate.rs` or `bounded`, not read).
- None seen that let a wrong program through in this file; coverage gaps (no test for a malformed `Context` signature other than arity) not assessed.

## Coverage
- Read in full: `M/tests.rs` lines 1-5282 (5,282 lines), in four reads (1-1400, 1400-2700, 2700-4000, 4000-5283).
- Skimmed: none.
- Not read: none.

## Five things a maintainer most needs to know about this unit
1. `M/tests.rs` is included only by `M/mathematical.rs:16-17` (`#[cfg(test)] mod tests;`); the helper functions at 4723-5282 are `pub(crate)` for only `type_sort`, `strict_sort`, `variable`, `pi`, `lambda`, `sigma`, `default_budget`, `polymorphic_identity_declaration`; the rest are private to the module but visible to child modules, so changing a helper's signature affects all 14 modules.
2. 88 tests; the largest groups are `spine_chains` (16) and `identity_and_w_types` (11). Seven of them run on a freshly spawned thread to prove deep terms do not use the call stack; the depths are 131,072 and 4,096.
3. The tests reach into private internals: `crate::substitution::remapped_nodes()` (a test-only counter), `crate::term::{instantiate_level, levels_equal}`, and `TermHandle::from_parts`/`from_arena_index`/`generation` (4177-4184). Renaming those breaks this file.
4. Every quotient and indexed scheme test uses `Two` as the carrier/index and `Level::Constant(0)`; the generality of the quotient scheme (other carriers, higher levels) is not exercised here.
5. The quotient assumption interface is pinned twice: the set of seven assumption indexes (2809-2834) and the exact closure sets for particular certificates (2413-2419, 2577-2619). Adding or reordering a `QUOTIENT_*` declaration changes both.

## What you are unsure about
- Whether `argument_error_pair_pattern!` is defined once or in two crates; Grep found uses in `bounded` and `mathematical` (14 occurrences in 7 files) but I did not open the definitions.
- Whether any helper in this file is unused; I did not grep each helper name.
- Whether the "quotient-refusing / elim-refusing" policies modeled at 2591-2619 correspond to real consumer policies; the consumers are outside this unit.
- The parallel tests in `bounded/tests/` were only seen through one grep and not compared line by line.
