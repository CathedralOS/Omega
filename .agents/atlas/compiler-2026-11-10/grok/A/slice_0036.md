# slice_0036

Digest preamble (read): `digests/area_11.md:1-7` — Area 11 covers proof, proof admission, psi-compiler, and checked trees; path shorthand maps `adm/` → `omega-rust/psi/compilation/proof/admission/src/`, `prf/` → `omega-rust/psi/compilation/proof/src/`, commit baseline `457adf4a11f` (lines in this checkout; not re-diffed per row unless noted).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0269 | omega-rust/psi/compilation/proof/admission/src/integer_rules.rs:686-701 | UNDECIDED | Early cast-bound accept on structural far-side equality; `IntegerCastBound` certificate emission from `prf/` not traced to a rejecting program. |
| A0270 | omega-rust/psi/compilation/proof/admission/src/integer_affine.rs:2945-2964 | UNDECIDED | Recognisers 7–8 skip `context.validate` on the root; whether bound conversion or core re-decide rejects out-of-context values not fully traced. |
| A0271 | omega-rust/psi/compilation/proof/admission/src/integer_affine.rs:2953-2964 | CLOSED | Shift recogniser ignores `count`, but non-`Truth` bound rules require count evidence in `map_direct_shift_left_bound`. |
| A0272 | omega-rust/psi/compilation/proof/admission/src/integer_affine.rs:2491-2535 | REACHABLE | `bounded_bitwise_sibling` can take the first matching uncited `LessOrEqual`, allowing a looser mask cap than a tighter cited axiom. |
| A0273 | omega-rust/psi/compilation/proof/admission/src/mathematical_core.rs:3664-3666 | CLOSED | Missing signature indices are skipped only in closure bookkeeping; certificate verify still errors on unknown constants during normalization. |
| A0274 | omega-rust/psi/compilation/proof/admission/src/mathematical_typing.rs:1375-1378 | CLOSED | Strict-sort conversion is intentional; `convert` infers `shared_type` before `SharedSort`, and admission certificates go through `check_type`. |
| A0275 | omega-rust/psi/compilation/proof/src/checker.rs:342-344 | REACHABLE | Overwrite detection matches `display_name` only; a later distinct binding with the same spelling can skip all assignment checks. |
| A0276 | omega-rust/psi/compilation/proof/src/checker.rs:374-399 | UNDECIDED | `Uncovered` interval/occurrence fallbacks avoid the kernel; soundness of `validation::*` helpers not read. |

## A0269

**Cited site (read):** `integer_rules.rs:664-701` in `check_integer_cast_bound_conversion` (`omega-rust/psi/compilation/proof/admission/src/integer_rules.rs`). Cite matches this checkout.

After lifting root and conclusion with `lift_fixed_integer_relation` (`:682-685`), when `mathematical_order` succeeds on both and `root_strict == target_strict`, the function returns `Ok(())` if embedded chain values align on matching sides and the opposite sides are structurally equal (`==`) without requiring those sides to be literals (`:686-700`). Otherwise the literal-oriented path runs (`:702+`).

**Callers (read):** `rg 'check_integer_cast_bound_conversion' omega-rust` → 9 matches: definition and tests in `integer_rules.rs` (6), `proof.rs` cast path (`:733`), re-export `lib.rs` (1). Production admission path: `cast_bound_relation` at `proof.rs:713-734` after `check_integer_cast_chain_witness`.

**Upstream (read):** `accept_certificate_with_machine_parameters` validates goal, assumptions, and every semantic axiom (`proof.rs:181-193`), dispatches `IntegerCastBound` through `cast_bound_relation` (`proof.rs:773-784`), then `re_decide_in_mathematical_core` (`proof.rs:209-216`).

**What would decide REACHABLE vs CLOSED (not traced):** How `prf/checker.rs` and obligation discharge build `ProofRule::IntegerCastBound` witnesses and whether a program that should fail range checking can supply `root_bound`/`conclusion` pairs that satisfy the early branch unsoundly.

**Verdict:** UNDECIDED.

## A0270

**Cited site (read):** `integer_affine.rs:2945-2964` inside `check_integer_affine_witness` (`omega-rust/psi/compilation/proof/admission/src/integer_affine.rs`). Matches digest.

When `definition_axioms` and `literal_axioms` are empty, recogniser 7 (`identity_ring_target`, `:2947-2949`) and recogniser 8 (shift-left target with `count: _`, `:2953-2963`) return `trivial_affine_form` with no `context.validate` on the root (`:2959-2963`). The general path that follows requires `ScalarTerm::Value` root (`:2965-2967`).

**Callers (read):** `rg 'check_integer_affine_witness' omega-rust` → 93 matches: `proof.rs` production (`affine_bound_relation` `:482`; other dispatches), extensive tests in `integer_affine.rs`, re-export `lib.rs`. Count: 3 files (`proof.rs`, `integer_affine.rs`, `lib.rs`).

**Downstream (read):** `affine_bound_relation` (`proof.rs:475-497`) maps the checked form through `check_integer_affine_bound_conversion` or truth-bound containment; admission still runs `re_decide_in_mathematical_core` after certificate rule dispatch (`proof.rs:209-216`). Entry validates all semantic axioms (`proof.rs:189-193`) but does not require the witness root to appear in axioms.

**Verdict:** UNDECIDED — whether a root `Value` absent from `PropositionContext` can complete `IntegerAffineBound` without an earlier validate and still admit a whole program.

## A0271

**Cited site (read):** `integer_affine.rs:2953-2963` — recogniser 8 binds `count: _` and accepts trivial affine form for `ExactIntegerShiftLeft` targets (`integer_affine.rs`).

**Closing check (read):** `bound_route` routes empty `endpoint_steps` with shift target to `BoundRoute::DirectShift` (`integer_affine.rs:442-448`). `map_integer_affine_bound_with_remainder_policy` calls `map_direct_shift_left_bound` (`integer_affine.rs:625`). That function requires count evidence in `root_bound` (equal or bounded literals on `count`, `:1409-1437`, `:1443-1450`) or errors (`DirectShiftCountEvidenceMismatch` / `DirectShiftRootBoundMissing`).

**Admission path (read):** Non-`Truth` `IntegerAffineBound` uses `check_integer_affine_bound_conversion` (`proof.rs:495-496`), which delegates to `map_integer_affine_bound` (`integer_affine.rs:372-382`).

**Input that reaches the branch (read):** A certificate whose affine witness uses recogniser 8 but whose cited `root_bound` lacks count facts cannot complete bound conversion unless `root_bound.conclusion` is `Truth` (truth-bounds path at `proof.rs:486-492` uses carrier-wide interval arithmetic; empty steps skip shift scaling in `integer_affine_truth_bounds` loop `:1967+`).

**Verdict:** CLOSED — witness admission without count is harmless for cited bound rules because bound mapping demands count evidence on the non-`Truth` path.

## A0272

**Cited site (read):** `integer_affine.rs:2491-2534` — `bounded_bitwise_sibling` collects mask aliases from `prior_axioms` Equalities then `find_map`s the first `LessOrEqual` with `left == mask` and nonnegative upper (`integer_affine.rs`).

**Call site (read):** Invoked from `apply_definition` bitwise-and/or branches (`integer_affine.rs:2383-2384`, `:2421-2422`) with `prior_axioms` = `&semantic_axioms[..index]` (see call at `:3020`).

**Entry validation (read):** `accept_certificate_with_machine_parameters` validates every semantic axiom before rule checking (`proof.rs:189-193`). `denote_bounded_certificate_with_machine_parameters` repeats validation (`bounded_denotation.rs:15300-15307`). Axioms are validated but need not be cited in the witness index list.

**Input that reaches the branch (read):** A definition step whose sibling mask is linked by Equal axioms to another term that also has an uncited loose `LessOrEqual` bound appearing before a tighter bound in the prefix slice; `find_map` returns the first match (`:2508-2517`).

**Verdict:** REACHABLE — replay can use a weaker uncited upper mask bound and admit a `BitwiseAndBoundedMask` step with an inflated cap.

## A0273

**Cited site (read):** `mathematical_core.rs:3660-3666` in `closure_over` — `signature.get(index)` `None` → `continue` without error (`mathematical_core.rs`).

**Verifier (read):** `weak_head_normalize_retaining` on `Term::Constant` returns `CoreError::UnknownDeclaration` when `signature.get(index)` is `None` (`mathematical_core.rs:1146-1151`). `verify_mathematical_certificate` runs `check_signature` then `check_type` on the certificate term (`mathematical_core.rs:479-497`).

**Callers of closure (read):** `assumption_closure` / `judgment_assumption_closure` (`mathematical_core.rs:3680-3687`) feed `certificate_assumption_closure` for receipt metadata (`mathematical_core.rs:500-511`; used from `bounded_denotation.rs` receipt assembly).

**Verdict:** CLOSED — skipping a missing index affects assumption-set metadata, not whether a malformed constant index can pass certificate verification; normalization still rejects unknown declarations.

## A0274

**Cited site (read):** `mathematical_typing.rs:1371-1378` in `comparison::Work::SharedSort` resume — when inferred sort `is_strict()`, conversion completes `true` without head comparison unless structurally equal (`mathematical_typing.rs`). Module header states strict irrelevance by design (`mathematical_typing.rs:7-8`).

**Call chain (read):** `mathematical_core::convertible_retaining` forwards to `super::typing::convert_retaining` (`mathematical_core.rs:1302-1311`) → `judgments::convert` (`mathematical_typing.rs:343-352`) → `drive` with `Operation::Convert`, which first `Infer`s `shared_type` (`mathematical_typing.rs:2295-2301`) then schedules `SharedSort`.

**Production convert uses (read):** `rg 'convertible\(' omega-rust/psi/compilation/proof/admission/src/bounded_denotation.rs` → 3 matches, all inside `#[cfg(test)]` (`bounded_denotation.rs:12354-12375`). Certificate admission uses `verify_mathematical_certificate` / elaboration `check_type` paths (`mathematical_core.rs:491-497`; `accept_certificate_with_machine_parameters` proposition validation `proof.rs:181-193`).

**Verdict:** CLOSED — strict irrelevance is explicit policy; admission judgments reach conversion only through typing judgements that infer the shared type first, not through unchecked `convertible` calls in production denotation.

## A0275

**Cited site (read):** `checker.rs:342-344` in `check_bounded_assignment` — early `return None` when `assignment_is_overwritten_before_consumption` is true (`omega-rust/psi/compilation/proof/src/checker.rs`).

**Overwrite logic (read):** `assignment_is_overwritten_before_consumption` (`checker.rs:420-473`) walks statements after the obligation assignment; on a later `Assignment`, if `display_name(assignment.target) == target_name` (stored from obligation target at `:439`, `:472-473`), returns `true`. No symbol-handle equality; `written_place_path` / alias checks apply to other paths only.

**Callers (read):** `rg 'check_bounded_assignment' omega-rust` → 3 matches in `checker.rs` (definition plus discharge sites).

**Input that reaches the branch (read):** A machine state where the obligation assignment target shares display spelling with a later assignment to a different expression node (for example an inner-scoped local with the same rendered name) that is not proven overlapping via `member_paths_may_alias`.

**Verdict:** REACHABLE — bounded-assignment checking is skipped entirely, including named constraints at `:348`, without proving the original assignment lies in range.

## A0276

**Cited site (read):** `checker.rs:373-397` in `check_bounded_assignment` — on `CertificateVerdict::Uncovered`, acceptance when interval analysis fits the target or when `validation::occurrence_integer_expression_fits_range` returns true (`checker.rs:374-390`). Parallel `Uncovered` branches exist at `:547-577`, `:623-642`, `:689-715`, `:762-808` (read cited spans).

**Kernel path (read):** `bounded_integer_value_verdict` returns `Uncovered` when certificate construction does not cover the expression (`checker.rs:947-948` area per digest).

**What would decide soundness (not read):** Implementations of `validation::occurrence_integer_expression_fits_range` (`validation/src/arithmetic_domains.rs:10555+`) and related interval helpers invoked from the other `Uncovered` arms.

**Verdict:** UNDECIDED.

## What I did not read

- `prf/` certificate builders for `IntegerCastBound` / `IntegerAffineBound` witness shapes tied to real corpus programs.
- Full `integer_affine_truth_bounds` behavior for every trivial-form target shape beyond the empty-step shift case.
- Entire `validation::occurrence_integer_expression_fits_range` and `arrival_integer_expression_bounds` bodies.
- Whether `display_name` collisions occur in normal lowering (only the checker comparison logic was read).
- Mathematical-core elaboration paths beyond `check_type` / `verify_bounded_certificate_with_machine_parameters` for every rule family.

SLICE COMPLETE
