Second pass for unit 74

Ranges covered (gap_019.tsv, one row):
- `omega-rust/psi/compilation/proof/admission/src/mathematical_core.rs` lines 4774-9639 (4866 lines), all read in full.

Checkout: `<checkout>/`. Short path below: `mathematical_core.rs` = the file above. Everything in this range is `#[cfg(test)]` code. I read lines 4760-4773 as header context only.

Where the range starts: line 4774 is inside `mod tests` (opened :4769) -> `mod assumption_closure` (opened :4770), in the middle of its `use crate::mathematical_core::{...}` statement (:4772-4776). It ends at the last line of the file, after `mod bounded_observations` (:9622-9639).

---

## 1. Purpose

The range is the whole test module of the mathematical core (the dependent type theory with `Two`, Id, W, Empty, Squash, Box, Sigma/Pi, and universe-polymorphic signatures). It contains no production logic. It does three things:
1. Pins the behaviour of the checker (`infer_type`, `check_type`, `convertible`, `weak_head_normalize`, `check_signature`, assumption closures, `verify_mathematical_certificate`) with positive cases and one negative case per `CoreError` variant it names.
2. Pins that long term chains (131,072 deep or 4,096 deep) do not overflow the default stack, by running the check inside `std::thread::spawn(...).join()`.
3. Builds the fixed `indexed_scheme` and `quotient_scheme` signatures and checks them, and checks certificates against the quotient signature.

A thread-local counter module `bounded_observations` (:9622-9639) is the only non-`mod tests` code in the range.

## 2. Shape (with line numbers)

`mod tests` :4769-9620 contains ten submodules and one block of shared helpers.

| Submodule | Lines | Tests (name:line) |
|---|---|---|
| `assumption_closure` | 4770-4849 | `certificate_assumption_closure_stays_on_callers_stack` :4780; `shared_term_graph_retains_exact_signature_dependencies` :4816 |
| `identity_and_w_types` | 4850-5603 | `identity_forms_at_the_carrier_level_and_refl_introduces` :4868; `identity_elimination_computes_on_refl_and_respects_the_ceiling` :4960; `stuck_identity_eliminations_compare_componentwise` :5023; `identity_elimination_rejects_malformed_motives_and_endpoints` :5097; `w_formation_rejects_strict_and_malformed_families` :5221; `sup_checks_its_annotation_and_components` :5276; `w_induction_checks_the_step_against_its_dependent_type` :5331; `w_induction_rejects_strict_and_non_universe_motives` :5395; `w_induction_computes_on_sup_with_a_neutral_child_function` :5442; `stuck_w_inductions_compare_componentwise` :5505; `w_types_convert_componentwise_and_sup_has_no_eta` :5550 |
| `level_worklists` | 5604-5751 | `deep_universe_formation_and_scope_refusal_use_default_stack` :5620; `level_syntax_traits_keep_derived_hash_and_diagnostic_contracts` :5666; `deep_level_operations_preserve_syntax_algebra_and_first_refusal` :5709 |
| `levels_and_declarations` | 5752-6008 | `a_universe_polymorphic_identity_checks_parametrically` :5763; `check_signature_rejects_self_and_forward_references` :5794; `a_declaration_whose_statement_is_not_a_type_rejects` :5828; `a_definition_constant_unfolds_and_an_assumption_stays_neutral` :5838; `stuck_constants_convert_at_semantically_equal_instantiations` :5873; `retained_carrier_aliases_survive_dependent_type_conversion` :5912 |
| `normalization_worklists` | 6009-6169 | `deep_typed_conversion_preserves_each_questions_budget_on_the_default_stack` :6017; `deep_neutral_application_normalization_stays_on_default_stack` :6054; `normalization_frames_preserve_reduction_budgets_and_refusals` :6094; `deep_structural_comparison_keeps_every_payload_and_tail` :6133 |
| `schemes_and_quotients` | 6170-7401 | inner modules `indexed_schemes` :6171, `quotient_derived` :6390, `quotient_receiver_policies` :6703, `quotient_schemes` :7039 (tests listed in section 3) |
| `spine_chains` | 7402-8025 | 17 tests, first `long_lambda_spine_infers_off_the_call_stack` :7421, last `deep_relevant_and_strict_formations_preserve_layer_refusals` :7978 |
| `strict_layer` | 8026-8635 | `empty_elimination_rejects_non_type_targets_and_foreign_scrutinees` :8043; `stuck_empty_eliminations_collapse_under_scrutinee_irrelevance` :8085; `squash_rejects_strict_carriers_and_mismatched_annotations` :8116; `squashed_proofs_are_irrelevant_but_never_extract_to_relevant_data` :8160; `squash_elimination_computes_on_canonical_witnesses` :8264; `box_formation_and_introduction_reject_relevant_domains` :8324; `unbox_computes_and_its_motive_may_land_at_either_sort` :8360; `stuck_unbox_eliminations_compare_componentwise` :8481; `empty_proposition_is_typed_only_at_strict_level_zero` :8590; `a_sigma_is_strict_only_when_both_components_are_strict` :8606 |
| `substitution_worklists` | 8636-8790 | `deep_closed_telescope_remapping_stays_on_default_stack` :8642; `shared_variables_keep_their_exact_binder_scope` :8668; `repeated_subterms_are_remapped_once_per_scope_and_query` :8715; `universe_instantiation_preserves_scope_and_refuses_missing_parameters` :8752 |
| `type_checking` | 8791-9119 | `unbound_variable_rejects` :8808; `conversion_refuses_at_the_step_ceiling` :8825; `dependent_pair_type_tracks_component_sorts` :8843; `pair_eta_converts_a_neutral_with_its_projections` :8877; `projections_and_pairs_reject_ill_typed_uses` :8919; `function_eta_never_deletes_an_untyped_wrapper` :8942; `dependent_two_elimination_checks_at_branch_specific_types` :8969; `case_two_computes_on_each_constructor_and_stays_stuck_on_neutrals` :9000; `case_two_rejects_motive_and_scrutinee_violations` :9068 |

`spine_chains` tests (all in :7402-8025): `long_lambda_spine_infers_off_the_call_stack` :7421, `long_pair_witness_checks_against_a_sigma_chain` :7447, `left_nested_pair_witness_checks_on_the_default_stack` :7479, `pair_checking_restores_binders_and_substitutes_only_checked_components` :7522, `pair_judgments_refuse_the_first_component_before_visiting_the_second` :7587, `mixed_constructor_inference_stays_on_the_default_stack` :7616, `mixed_constructor_children_rejoin_their_own_binder_prefix` :7643, `mixed_application_and_projection_preserve_order_and_reduction_budget` :7681, `dependent_pair_arguments_reenter_inference_on_the_default_stack` :7716, `nested_case_branches_check_on_the_default_stack_including_the_unselected_branch` :7758, `long_application_spine_consumes_arguments_in_order` :7819, `long_snd_chain_projects_off_the_call_stack` :7852, `a_chain_rejects_at_its_end` :7871, `deep_binder_domains_form_on_the_default_stack` :7893, `binder_formation_restores_the_exact_prefix_between_children` :7916, `deep_relevant_and_strict_formations_preserve_layer_refusals` :7978. Constants `DEEP = 131_072`, `SHALLOW = 4_096` :7408-7409; `DEEP` also :5610.

Shared helpers inside `mod tests` (file scope of `tests`, :9121-9619):
- imports :9121-9125;
- term builders: `type_sort` :9127, `strict_sort` :9131, `variable` :9135, `pi` :9139, `lambda` :9147, `apply` :9155, `sigma` :9159, `pair` :9167, `fst` :9171, `snd` :9175, `two` :9179, `two_zero` :9183, `two_one` :9187, `two_identity` :9191, `case_two` :9198, `id` :9213, `refl` :9222, `id_elim` :9226, `w_type` :9241, `sup` :9245, `ind_w` :9260, `empty` :9269, `empty_elim` :9273, `squash` :9277, `squash_intro` :9281, `squash_elim` :9285, `boxed` :9298, `box_intro` :9302, `box_elim` :9306, `sort_level` :9441, `constant` :9445;
- fixtures: `default_budget` :9319 (`Budget::new(DEFAULT_CONVERSION_STEPS)`), `universe_motive` :9323, `branching_family` :9329, `symmetry_motive` :9341, `w_step_binding` :9357, `w_context` :9387, `concrete_step` :9421, `polymorphic_identity_declaration` :9452, `two_indexed_family` :9466, `scheme_signature` :9504, `indexed_context` :9510, `two_quotient_over` :9587, `two_quotient` :9606, `quotient_signature` :9610, `is_set_zero` :9616.

`bounded_observations` (`#[cfg(test)] pub(crate) mod`, :9622-9639): thread-local `ENTRIES: Cell<(usize, usize)>` :9624-9626; `bounded_verification_entries()` :9628 reads it; `record_admission()` :9632 and `record_denotation()` :9636 increment the first and second counter.

## 3. Inputs and outputs

Reads: the non-test API of `mathematical_core` (I did not re-read it; earlier notes cover :1-4766): `TermArena`, `Term`, `Level`, `Sort`, `Context`, `Signature`, `Declaration`, `Budget`, `CoreError`, `MathematicalCertificate`, `QuotientFamily`, `IndexedFamily`, `indexed_scheme`, `quotient_scheme`, `QUOTIENT_*` and `INDEXED_*` constants, and `shift`/`substitute`/`instantiate_levels`/`instantiate_level`/`levels_equal` (imported directly from `substitution`/`term` at :8638, :5711).

Outputs: assertions only. No production state is mutated. The only global-ish state is the thread-local counter at :9624.

Tests worth recording by what they pin (all read):
- `quotient_receiver_policies` (:6703-7038): `a_squashed_relation_yields_only_squashed_evidence` :6728 checks that with a relation of type `Box(Squash(...))`, `effective` returns `Box(Squash(..))` evidence, `squash_elim` to a relevant target fails with `SquashTargetNotStrict` (:6840), and `sound` with a raw identity witness fails `ArgumentTypeMismatch` (:6853). It then verifies a `MathematicalCertificate` over `quotient_scheme` (:6900-6908) and asserts `certificate_assumption_closure` is exactly `{QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_EFFECTIVE}` (:6910-6917).
- `a_quotient_refusing_policy_distinguishes_the_same_certificate` :6921: verifies two certificates against `quotient_scheme`. The `coverage` certificate has closure `{QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_SOUND, QUOTIENT_ELIM}` (:6995-6999). The `id_sym` certificate has an empty closure (:7000-7005). A forged certificate (claims `Squash(Two)` for `coverage z`) fails `TypeMismatch` (:6976-6991). The test defines `quotient_refusing = {}` and `quotient_admitting` = the 8 assumed names (:7007-7019) and asserts subset relations on them. These sets exist only inside the test; they are not read from production code.
- `quotient_schemes` (:7039-7400): `quotient_beta_is_propositional_evidence_not_conversion` :7051 (beta is an Id witness, not a reduction); `quotient_lift_rejects_a_malformed_congruence` :7146; `quotient_lift_precondition_admits_the_transport_backed_shape` :7224; `quotient_scheme_rejects_a_wrong_derived_body` :7330 replaces `declarations[QUOTIENT_SOUND]` with `Declaration::definition(.. wrong_body)` and expects `check_signature` to reject (:7362-7368); `quotient_elim_rejects_missing_set_evidence` :7372.
- `quotient_derived` (:6390-6702): lemma checks for `id_sym`, `id_trans`, `id_cancel`, `prop_is_set`, `box_prop`, `ind_prop`, `unique` at `Two`.
- `indexed_schemes` (:6171-6389): `indexed_scheme` gives 5 declarations and checks as a signature (:6185-6191); constructor/eliminator type checks; `DeclarationArityMismatch` for the wrong number of universe levels at `INDEXED_W` (expected 3, supplied 1, :6365-6372) and `INDEXED_IND` (expected 4, supplied 3, :6380-6387).

## 4. How it decides

This section records what the tests pin about the checker's judgments; the checker's own code is outside my range. Keyed on:
- Term identity/structure: `arena.structurally_equal` is used for expected-vs-inferred type checks (e.g. :4886, :5289).
- Definitional equality: `convertible(arena, ctx, a, b, ty, budget)`, with the type argument deciding the rule. Examples:
  - Proof irrelevance for `Squash`: at type `Squash(A)` two distinct inhabitants/neutrals convert, including a neutral and a `squash_intro` (:8204-8214). At a relevant type `variable(7)` two different variables do not (:8216-8221).
  - `Empty` eliminations collapse on scrutinee, and differ when the target differs (:8085-8113).
  - Pair eta holds for a neutral against `(fst w, snd w)` (:8894-8901).
  - `sup` has no eta (test name :5550; asserted `node` vs `t` not convertible :5599-5601).
  - W-type children `B` vs `lambda a. B a` convert (:5562-5566). `W(A,B)` vs `W(A, const Two)` does not (:5568-5576).
  - Wrapped lambda `lambda x. f x` vs `f` compared at `variable(1)` is NOT convertible (:8942-8965). I infer `variable(1)` is the sort-typed context entry, not a Pi, so this test shows only "no eta at a non-function type", not "no function eta anywhere". Inferred, not traced.
  - Constants convert at `max(u,v)` vs `max(v,u)` (normal-form level equality, :5883-5889) and not at different level instantiations (:5891-5895).
  - Distinct assumption constants at the same levels do not convert (:5900-5908).
- Declaration index through a `retained` predicate: `convertible_retaining` with `retained = |declaration| declaration < 2` stops unfolding of declarations 0 and 1 (:5943-5957). Two aliases of `Two` are convertible without `retained` and not with it (:5945-5957).
- Budget: every normalization step consumes one unit. `Budget::new(0)` yields `Err(StepCeiling)` for one reduction (e.g. :4992-5000). Inspecting a neutral spine costs 0 (:6077-6087). Typed conversion of two deep Pi chains costs exactly 1 step (`(1, true), (0, false)` :6037).
- Universe levels: `Level` `PartialEq` is structural while `levels_equal` is semantic: `level != level.max(Constant(DEEP))` but `levels_equal` is true (:5726-5730). `arena.structurally_equal` on constants with `max(p,0)` vs `p` returns false (:6161-6164).

## 5. Cases by shape

The tests exercise fixed list shapes, not code that refuses shapes. Items that read like "fixed list" statements pinned by tests:
- `weak_head_normalize` expected to reduce exactly: Apply/Lambda, Fst/Pair, CaseTwo on `TwoZero`/`TwoOne`, IdElim on Refl, IndW on Sup, SquashElim on SquashIntro, BoxElim on BoxIntro (tests :5457-5459 for IndW in 1 step; :6111-6120 Fst-of-Apply in 2 steps; :8291-8297; :8409-8415; :9014-9021). Constants unfold only when the declaration has a body (:5848-5857) and stay neutral for an assumption (:5865-5869).
- `Dummy` handling pinned: `Term::Dummy` is refused with `CoreError::DummyTerm` in `infer_type`/`check_type` (:7594-7612, :7702-7712). `structurally_equal(dummy, dummy)` is false (:6147-6151, quote: "invalid placeholders are never equality evidence").
- Level ceiling: `Level::Constant(u32::MAX)` as a type sort gives `LevelOverflow` (:5648-5657).
- Strict layer rules pinned: strict carrier to W is `StrictWCarrier` (:5239); identity over a strict carrier is `StrictIdentityDomain` (:4928); squash of a strict carrier is `StrictSquashDomain` (:8131); box of a relevant type is `NonStrictBoxDomain` (:8339); `Empty` has type `Strict 0` only (:8590-8602); sigma is strict only when both components are strict (:8606-8634).

## 6. Same job elsewhere

- The `conv!` macro is defined twice with identical bodies: `identity_and_w_types` :4861-4865, `strict_layer` :8036-8040, and `type_checking` :8801-8805 (three copies, read). Each just forwards six arguments to `convertible`.
- Helpers `type_sort`, `variable`, `pi`, `lambda`, `sigma`, `two`, etc. are rebuilt by hand in tests of other files: grep for `fn type_sort(` is outside what I read; suspected (admission/tests/common/mod.rs uses `argument_error_pair_pattern`, 3 hits, so it is a shared test-support file; I did not read it).
- The deep-chain idiom (build 131,072 nested `pi`, run in a spawned thread, `.join().expect(..)`) repeats in `assumption_closure` :4781, :4817, `level_worklists` :5621, :5710, `normalization_worklists` :6018, :6055, :6134, `spine_chains` :7480, :7617, :7717, :7759, :7894, :7979, `substitution_worklists` :8643, :8716. The same 131,072 depth is also used in certificate tests at mathematical_core.rs:401-424 (earlier reader's range, suspected same idiom; only the grep line was seen).
- `quotient_scheme`-based setup (`two_quotient` + `quotient_signature`) is repeated at the start of 10 quotient tests (e.g. :6403-6404, :6456-6457, :6503-6504, :6556-6557, :6605-6606, :6731-6732, :7054-7055, :7149-7150, :7227-7228, :7375-7376).

## 7. Unused or one-caller code (greps)

- `bounded_observations::record_admission` / `record_denotation` / `bounded_verification_entries`: `Grep` count mode over `omega-rust` for `record_admission|record_denotation|bounded_verification_entries` gives 29 lines in 6 files. Of those, two files are unrelated name matches (`scalar_record.rs:265` test name `c_record_admission_...`, and `validation.rs:915` / `ranked_rewrites.rs:647` `invariant_record_admission`). Real uses: `record_admission` called once, `proof.rs:180`; `record_denotation` called once, `bounded_denotation.rs:15299`; `bounded_verification_entries` read in `proof.rs` (about 12 lines, :2787-4128) and in `mathematical_core.rs` at :401/:424 and in my range :4799/:4806. Note: the call sites at `proof.rs:180` and `bounded_denotation.rs:15299` are plain statements; I did not check whether they are themselves under `#[cfg(test)]`. Because `bounded_observations` is `#[cfg(test)]` (:9622), a non-test caller would not compile, so I infer they are cfg(test) (inferred).
- `crate::argument_error_pair_pattern!` macro used in my range at :6346, :6353, :6360, :7366: `Grep` count for `argument_error_pair_pattern` over `omega-rust` = 13 lines in 6 files: mathematical_core.rs 4 (all mine), mathematical_typing.rs 1 (likely the definition), admission/tests 8. So the macro is shared between this file's tests and the integration tests.
- Quotient builders used in my range (already counted by the earlier reader as non-test no-callers): in tests I saw `family.id_sym` :6418, `id_trans` :6435, `id_cancel` :6441, `prop_is_set` :6485, `box_prop` :6526, `ind_prop` :6577, `unique` :6661, `project`, `quotient`, `sound`, `effective`, `beta` :7124, `elim` :7136, `lift` :7189, `lift_precondition` :7310, `transport` :7100, `coverage` :6931. That is all 20 builders used only from `#[cfg(test)]` here, which matches the earlier note that the quotient scheme has no non-test caller. A builder I did NOT see used in my range: `is_set` as a `QuotientFamily` method (tests use the `QUOTIENT_IS_SET` constant through `is_set_zero` :9616 instead).
- `identity_substitution` (earlier note: 0 call sites): not referenced anywhere in my range (read).

## 8. Stated limits

Comments in the range that state a limit or non-feature:
- :5728 "syntactic equality must not become semantic conversion".
- :5737 "instantiation preserves explicit syntax for wire-depth admission".
- :6548 "boxed inhabitants are propositionally, not definitionally, equal".
- :6699 "pointwise uniqueness is not function extensionality" (test `quotient_unique_collapses_sections_agreeing_on_projections` :6602 asserts that `unique` does not prove `s1 = s2` as functions).
- :7051 test name `quotient_beta_is_propositional_evidence_not_conversion`.
- :7366 "a wrong body never passes as the law".
- :8942 test name `function_eta_never_deletes_an_untyped_wrapper`.
- :6990 "a forged certificate is refused before any policy runs".
- :4803 "the definition's body contributes its axiom; unused declarations do not" and :4842 "equal term handles cannot reuse another signature's assumption closure".
No TODO, "unsupported", "migration" or "remains" text in the range (searched by eye while reading; not grepped for this range).

## 9. Fail-open risks

None seen in this range, because the range is test code that asserts rejections. Things a reader could mistake for fail-open, with status:
- :6926 `Signature::from_declarations(declarations.clone())` builds a signature WITHOUT `check_signature` (read); it is used only to compute `assumption_closure` (:7026). Test-only; status "traced" (the same declarations were checked earlier through `quotient_signature`/`verify_mathematical_certificate` in the same test at :6951-6974).
- :7339 the wrong-body test builds a signature by editing a declaration and relies on `check_signature` to reject (:7364); it asserts rejection with `argument_error_pair_pattern!` (not a specific error), so what it pins is "some argument-type error". Stated: quote :7367 "a wrong body never passes as the law".
- Assumed postulates: tests at :7008-7019 name the 8 assumed quotient declarations (`QUOTIENT`, `QUOTIENT_PROJECT`, `QUOTIENT_SET`, `QUOTIENT_SOUND`, `QUOTIENT_EFFECTIVE`, `QUOTIENT_ELIM`, `QUOTIENT_BETA`, `QUOTIENT_LIFT_PRECONDITION`). This agrees with the earlier notes (assumed declarations listed there at lines 1886-2300). Consistent, not a new finding.
- Coverage gap I can state from reading: I saw no test in the range that submits a certificate whose signature has an assumption that `check_signature` should refuse for a non-type statement through `verify_mathematical_certificate` (the non-type statement test is at :5828 through `check_signature` directly). This is "no test seen here", not a claim that none exists elsewhere (mathematical_core.rs:521-1059 is not in my unit).

## Differences from the earlier reader's notes (unit_074.md)

- The earlier notes say `mod tests` spans 4769-9621 (notes line 144). In the file, `mod tests` closes at :9620 and `bounded_observations` starts at :9622. Immaterial.
- The earlier notes' submodule list (assumption_closure, identity_and_w_types, level_worklists, levels_and_declarations, normalization_worklists, schemes_and_quotients, spine_chains, strict_layer, substitution_worklists, type_checking) and helper range 9127-9620 match what I read. The `quotient_scheme` test-use lines in the earlier notes (:6716, :6901, :6925, :7047, :7333, :9124, :9612) all match my read.
- The earlier notes' count "12 uses of quotient_scheme in mathematical_core.rs" is not checked here.
- Nothing in my range contradicts the earlier reader's claims about the production code.

---

## Coverage

- Read in full: `mathematical_core.rs` 4774-9639 = 4866 lines.
- Skimmed: 0 lines.
- Not read: 0 lines.
- Extra lines read for context only (not counted): 4760-4773.
- Greps run (all with the Grep tool, count or content mode, over `<checkout>/omega-rust`): `record_admission|record_denotation|bounded_verification_entries` (29 lines in 6 files, listed in item 7); `argument_error_pair_pattern` (13 lines in 6 files); a method/constant search for `box_prop|ind_prop|lift_precondition|coverage|unique|QUOTIENT_LIFT_PRECONDITION` restricted to `mathematical_core.rs` (the output showed definitions at :3048, :3116, :3127, :3143, :3147 and constant :2811).

## Five things a maintainer most needs to know about this range

1. The range is entirely test code: ten test submodules (:4770-9119), a helper block (:9121-9619) and a thread-local counter module (:9622-9639). No production behaviour lives here.
2. The depth tests (131,072 nested terms; 4,096 for `SHALLOW`) run inside `std::thread::spawn` so that a regression to recursion overflows a default-sized stack in the spawned thread (e.g. :4781, :5621, :7480, :8643). They are the only guard in this file that the iterative worklists in `term`, `substitution`, `conversion` and `signature` stay iterative.
3. The quotient signature is tested as postulates plus derived lemmas: the test at :7008-7019 lists the 8 assumed names and `a_quotient_refusing_policy_distinguishes_the_same_certificate` :6921 shows the effect (certificates that cite them get a non-empty assumption closure; ones that only use derived definitions get an empty one). All 20 quotient builders are exercised only from these tests.
4. Budget accounting is pinned exactly: one reduction = one step (`Budget::new(1)` fails a 2-step reduction :6112; neutral spines cost 0 :6087; typed conversion of deep Pi chains costs 1 :6037; W induction on `sup` costs 1 :5459).
5. `bounded_observations` (:9622-9639) is a test-only thread-local pair of counters. `record_admission` is called from `proof.rs:180` and `record_denotation` from `bounded_denotation.rs:15299`, and the tests at `proof.rs:2840-4128` and mathematical_core.rs:4799-4809 assert exact counts, which pins that assumption-closure collection and public admission do not re-enter the bounded verifier.

## What I am unsure about

- Whether `proof.rs:180` and `bounded_denotation.rs:15299` are `#[cfg(test)]`-gated; I did not read those sites, only the grep lines.
- The type of `variable(1)` in the `function_eta_never_deletes_an_untyped_wrapper` test (:8958): I inferred it is the sort entry, so the test may not pin "no function eta" in general (see section 4).
- I did not read non-test checker code (`mathematical_typing.rs`), so statements about which `CoreError` each case maps to come from the test assertions only, not from the checker.
- I did not check whether every `CoreError` variant named in these tests (e.g. `StrictCaseMotiveCodomain`, `BoxMotiveCodomainNotAUniverse`, `IdentityMotiveCodomainNotAFunction`) has a production raise site; only that tests expect them.
