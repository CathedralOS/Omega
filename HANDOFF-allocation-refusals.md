# Handoff: PROOF-KERNEL-CORE Step 5, remaining allocation refusals

Written 2026-11-27 (PT) when the work moved to Astra. Goal: `omega --check
source/library/allocation/main.omg` compiles with no refusal, so that Step 5 is
fully done.

## Done (merged to main)

- #6256, #6275, #6293, #6297: the four Step 5 sub-items.
- #6306: the package review handles guarded crash routes.
- #6310: a stored value delivers the content domains its destination displays (bump.omg:60).
- #6640: the stage 06 prover reasons through equations among the facts:
  - `equal_term_order` bounds a sum by a fixed integer it equals.
  - `equal_through_equations` equates two values when two equations meet in one term.
  - Tests: `checking_equation_facts`, with 3 positives and 3 negative twins.

## In flight

Branch `work/psi-linear-equations`, one WIP commit on top of #6640's
pre-squash commit. It is not reviewed and has no PR. Rebase it onto main
first. #6640 was squash-merged, so drop the parent commit if it conflicts.

- New `src/proofs/prove/linear.rs` in
  `source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program`.
  `linear_equality` proves a fixed integer or mathematical equality from an
  integer linear combination of the equations among the facts.
  - Gaussian elimination over rationals. A combination that needs a fraction
    is not used, and at most 32 steps are allowed.
  - It uses **only existing kernel rules**, so there is no kernel or spec change.
  - Each step works on a chain endpoint `T` and an equation `a == b`:
    the closed-relation primitive proves `T <= a + (T - a)`, because the two
    sides have the same ring normal form. `IntegerMathOrderTranslation` gives
    `a + t <= b + t`. `IntegerLessOrEqualTransitivity` composes the steps.
  - `IntegerOrderAntisymmetry` joins the two directions. For a scalar goal,
    each direction is first restated as a scalar order with
    `restate_integer_relation`.
  - Value equalities among the facts are substituted into the goal and every
    equation first (`check_value_equality_denotation` and
    `ValueEqualityTransport`). This is what makes address equalities usable.
    Only an acyclic subset is kept (`acyclic_value_equalities`), because the
    denotation refuses cyclic sets with `CyclicValueEquality`.
- Hooks in `prove.rs`:
  - The scalar `Equal` branch, after `equal_through_equations`.
  - The `IntegerMathEqual` branch, after `by_transport`.
- Result: `bump::reset` (merge adjacency requirement and `Bump` clause 4) and
  `bump::allocate` (clause 4) now pass. The existing case split supplies the
  arms.
- Scratch checks that passed: `/tmp/claude-0/t2/{a,b,c}.omg`. These are lost
  with the container.

### Next steps for the branch

1. Rebase, then rename the commit. A suggested title: `psi: prove equalities by linear combinations of the equations among the facts`.
2. Add `tests/equation_facts` fixtures, or a new `checking_linear_equations`
   test registered in `source-rust/subsystems/compiler/source-to-psi/Cargo.toml`.
   Each positive needs a negative twin:
   - Positive: `rb == b`, `embed(rl)+embed(rem) == embed(cap)`, `rem == tl`,
     `embed(tb)+embed(tl) == embed(b)+embed(cap)` proves
     `embed(tb) == embed(rb)+embed(rl)`. Negative twin: change one equation
     to `<=`.
   - Positive: the same setup with a case split through
     `tl == 0 || ...`, proving `rl == 0 || tl == 0 || ...`.
   - Positive: addresses (`tb`, `b`, `rb` of type `address`) together with
     `rl == tl - len` and a split.
   - Negative: a combination that needs a fraction (`2*x == y` with goal
     `x == ...`) must stay unproved.
3. Run clippy, fmt `--check` (apply only the hunks it reports), the stage 06
   suites and the allocation check. Open the PR and merge it on admission.

## The three remaining refusals (each is unbuilt)

Do them in this order, as the coordinator asked. Where a pinned refusal or the
spec needs a ruling, record it in `OWNER_QUESTIONS.md` and keep going. Do not
edit `wiki/spec/`.

1. **`vec.omg:52` `store_cell::change`.** The error is "bodyless call needs
   exactly one argument per declared type parameter".
   - It is raised in
     `source-rust/subsystems/compiler/source-to-psi/representations/typed-trees/callables/callable_applications.rs`
     at about line 3647, in `prepare_bodyless_callable_application`.
   - The cause is that `|layout|::write(@mutable view.value, value)` calls the
     generic boundary machine `write<Accessor, Value>` (`source/library/core/layout.omg:542`)
     without type arguments.
   - What it needs: type-argument inference for bodyless calls, from the
     actuals' types. Per the comment at layout.omg:535-538, the compiler
     closes this application only for an accessor issued by the selected
     access plan. `read` at vec.omg:102 is the twin case.
2. **`access.omg:165` `view_extent_mut`.** The error is "record application
   whose reference regions are not retained".
   - It is raised in stage 05,
     `pipeline/05_prepared-typed-trees-to-lowered-program/src/read/source_records.rs:402`.
     The refusal is deliberate and pinned by a test at about line 1945 of the
     same file.
   - What it needs: a record application such as
     `AccessOutcome<@'storage mutable View, ...>` must keep the reference
     argument's lifetime correspondence, instead of the region-free nominal
     identity (`scope.region_free_type_identity`).
   - This is a design-level change to the pinned refusal: record it in
     OWNER_QUESTIONS.
3. **`resident_backing`: resident-view custody.** The error is a deliberate
   unavailable: "resident view needs evaluated layout, content, profile and
   policy custody".
   - It is raised in stage 05 `read/calls.rs`, at about line 1447, and pinned
     by `source-rust/application/tests/resident_domain_arguments.rs`.
   - This is the largest of the three features.

## Commands

- Build: `mbx build -p omega`, then
  `./target/debug/omega --check source/library/allocation/main.omg`. Never use
  raw cargo.
- Tests:
  - `mbx test -p source-compiler --features test-support --test checking_equation_facts --test checking_required_floor`
  - `mbx test -p lowered-program-to-checked-program` (prover unit tests)
- Clippy: `mbx clippy -p lowered-program-to-checked-program --all-targets --no-deps -- -D warnings`
- Format: `mbx xtask fmt --check --base origin/main`. Never run fmt without
  `--check`, because it reformats the workspace.
  - `prove/mathematical_bounds.rs` is unformatted on main. Leave its hunks alone.
- PR rules: `mbx xtask check-rules --base origin/main --title ... --body-file ...`
  - The body starts with the attribution comment and the "Requested by Jarod" line.
  - It has Gate (RC-* only), Spec and Proof lines.
  - `Proof: <tests> | <clean, or what failed and what it was>`
  - It ends with the Agent line from `mbx xtask provenance`.
  - No Co-Authored-By.
  - Branch `work/<lane>-<topic>`; title `lane: statement`, at most 85 characters.
  - One PR per fix. Squash-merge it once admission passes.

## Gotchas

- Failures that already occur on main, not caused by this work:
  - The `provider_choices` lib tests in source-compiler (5 tests).
  - `tests::type_parameters::generic_fields_form_their_qualifications_without_assuming_the_predicate`,
    in its unknown-domain case.
- The checker reports Incomplete obligations only after the definite
  refusals are gone. Fixing one refusal can reveal the next.
- Kernel facts to keep in mind:
  - `IntegerMathEqual` operands must be canonical (sorted), so positional
    `EqualityTransitivity` often cannot chain. Prefer order chains with
    `IntegerOrderSubstitution`, which takes either orientation.
  - `restate_integer_relation` only bridges an exact lift or lowering, which
    sorts `Equal`. That is why the scalar goal is restated per order direction.
  - `lift_fixed_integer_relation` excludes `address`. Use value-equality
    transport for addresses.
