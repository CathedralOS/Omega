//! Laws the mathematical kernel judges, through the `omega` command.
//!
//! Stage 06's own tests read the kernel's judgment and re-verify its
//! certificate. These run the same fixtures through every later stage, so an
//! accepted law also survives publication and a refused one reaches the user
//! as a diagnostic at its source position.
//!
//! The generic semiring machine once passed the checker and then failed
//! publication: the trait's type argument was recorded under the requirement
//! it instantiates, not under the machine that declares the record binder.

#[path = "support/project.rs"]
mod project;
mod support;

use std::path::PathBuf;
use std::process::Output;

const CHECKER_FIXTURES: &str = "source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures";
const UNPROVED: &str = "no available equality proves the stated Identity";

fn check(fixture: &str) -> (Output, String) {
    let directory: PathBuf = support::repository_root()
        .join(CHECKER_FIXTURES)
        .join(fixture);
    let output = project::omega(&directory, &["--check", "main.omg"]);
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    (output, report)
}

#[test]
fn a_generic_machine_that_cites_its_semiring_law_compiles() {
    let (output, report) = check("semiring_identity/pass/cited");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn the_same_machine_without_the_citation_is_refused_at_its_ensures() {
    let (output, report) = check("semiring_identity/fail/uncited");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("|proof|::Identity::<T>(Ring::add(b, a), Ring::add(a, b))")
            && report.contains(UNPROVED)
            && report.contains("main.omg:5:9"),
        "{report}"
    );
}

#[test]
fn peano_addition_associativity_compiles_and_its_saved_program_names_the_kernel_type() {
    // One test for this fixture: two `omega` runs in one project directory
    // at the same time contend for its package lock, and one is refused.
    let (output, report) = check("ring_law/pass/ring_law_conformance");
    assert_eq!(output.status.code(), Some(0), "{report}");

    // Peano has no storage, so the saved program carries it only as a
    // mathematical carrier: its source type identity and its kernel type.
    let directory: PathBuf = support::repository_root()
        .join(CHECKER_FIXTURES)
        .join("ring_law/pass/ring_law_conformance");
    let scratch = platform_custody::temporary_directory::TemporaryDirectory::create(
        "kernel-judged-law-psi",
        false,
    )
    .unwrap();
    let artifact = scratch.path().join("ring_law_conformance.psi");
    let omega = |arguments: &[&str]| {
        let output = project::omega(&directory, arguments);
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert_eq!(output.status.code(), Some(0), "{arguments:?}: {report}");
    };
    let path = artifact.to_str().unwrap();
    omega(&["--emit-psi", path, "main.omg"]);
    omega(&["psi", "inspect", "--artifact", path]);

    let bytes = std::fs::read(&artifact).expect("the compiler saved the program");
    let saved = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
        .expect("the saved program reopens");
    let module = saved.semantic_module();
    let [carrier] = module.types.mathematical_carriers.as_slice() else {
        panic!("one carrier: {:?}", module.types.mathematical_carriers)
    };
    let identity = module.identities.text(carrier.type_identity).unwrap();
    assert!(identity.contains("Peano"), "{identity}");
    let key = terminal_codec::decode_mathematical_certificate(&carrier.carrier_key).unwrap();
    assert!(
        key.arena
            .structurally_equal(key.certificate.term, key.certificate.expected)
    );
}

#[test]
fn peano_right_distributivity_regroups_under_its_associativity_law_and_compiles() {
    let (output, report) = check("ring_law/pass/peano_right_distributivity");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn right_distributivity_without_its_recursive_call_is_refused_at_the_successor_arm() {
    let (output, report) = check("ring_law/fail/peano_right_distributivity_uncited");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains(
            "cannot prove ensures contract for `times_distributes_right` in the case \
             `Peano::Succ`: core\\\\Identity::<Peano>(times(plus(Peano::Succ(pred), b), c), \
             plus(times(Peano::Succ(pred), c), times(b, c)))"
        ) && report.contains(UNPROVED)
            && report.contains("main.omg:52:30"),
        "{report}"
    );
}

#[test]
fn peano_left_distributivity_reorders_under_its_commutativity_law_and_compiles() {
    let (output, report) = check("ring_law/pass/peano_left_distributivity");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn left_distributivity_is_refused_when_commutativity_is_outside_the_conformance() {
    let (output, report) = check("ring_law/fail/peano_left_distributivity_unlicensed");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains(
            "cannot prove ensures contract for `times_distributes` in the case \
             `Peano::Succ`: core\\\\Identity::<Peano>(times(Peano::Succ(pred), plus(b, c)), \
             plus(times(Peano::Succ(pred), b), times(Peano::Succ(pred), c)))"
        ) && report.contains(UNPROVED),
        "{report}"
    );
}

#[test]
fn a_program_over_the_library_nat_compiles_on_its_semiring_conformance() {
    let (output, report) = check("nat_semiring/pass/library_conformance");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn a_local_operation_gets_no_license_from_the_library_conformance() {
    let (output, report) = check("nat_semiring/fail/local_operation_unlicensed");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("cannot prove ensures contract for `relies_on_nothing`")
            && report.contains(UNPROVED),
        "{report}"
    );
}

#[test]
fn library_subtraction_whose_order_premise_is_in_hand_compiles() {
    // `difference` requires `b <= a`, which is exactly what core's `-`
    // requires at `a - b`.
    let (output, report) = check("nat_order/pass/exact_subtraction");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn library_subtraction_under_the_swapped_order_is_refused_at_its_declaration() {
    // The same machine requiring `a <= b` instead: the premise of `-` is
    // not one it holds, and nothing proves it from `a <= b`. The refusal
    // names the premise of the call and is placed at `difference`'s
    // declaration (`main.omg:7:9`), the only position it carries.
    let (output, report) = check("nat_order/fail/swapped_order");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains(
            "cannot prove ensures contract for `difference`: the premise of `subtract` at its call"
        ) && report.contains(UNPROVED)
            && report.contains("main.omg:7:9"),
        "{report}"
    );
}

#[test]
fn library_subtraction_without_an_order_premise_is_refused_for_that_premise() {
    // The same machine with no `requires`. Its `a - b` is the first use of
    // `subtract`, whose signature is the first to read `less_or_equal`. The
    // call must still be judged against `subtract`, so the refusal is the
    // missing premise, not a result type taken from `less_or_equal`.
    let (output, report) = check("nat_order/fail/missing_premise");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains(
            "cannot prove ensures contract for `difference`: the premise of `subtract` at its call"
        ) && report.contains(UNPROVED)
            && report.contains("main.omg:9:9")
            && !report.contains("typing arm value type"),
        "{report}"
    );
}

#[test]
fn laws_with_a_premise_compile() {
    let (output, report) = check("law_premise/pass/peano_cancellation");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn a_citation_without_its_premise_is_refused() {
    let (output, report) = check("law_premise/fail/citation_without_its_premise");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("cannot prove ensures contract for `cancels_without_its_premise`")
            && report.contains("a cited law's premise is not established at its citation"),
        "{report}"
    );
}

#[test]
fn a_product_of_two_sums_over_the_library_nat_expands_and_compiles() {
    let (output, report) = check("polynomial/pass/ring_full_polynomial_compile");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn the_recovered_unlicensed_rearrangement_is_refused_for_its_three_laws() {
    // Each claim of the program is one law left unproved: a law merely in
    // scope, another trait application's law and another satisfier group's
    // law license nothing.
    let (output, report) = check("polynomial/fail/ring_rearrange_unlicensed_rejected");
    assert_eq!(output.status.code(), Some(1), "{report}");
    for machine in [
        "needs_rearrange",
        "mixed_application_associativity",
        "mixed_family_associativity",
    ] {
        assert!(
            report.contains(&format!("cannot prove ensures contract for `{machine}`")),
            "{machine}: {report}"
        );
    }
    assert!(report.contains(UNPROVED), "{report}");
}

#[test]
fn laws_about_a_record_of_mathematical_values_compile() {
    let (output, report) = check("record_law/pass/pair_of_peano");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn a_record_law_with_one_field_uncited_is_refused() {
    let (output, report) = check("record_law/fail/pair_field_uncited");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("cannot prove ensures contract for `add_pair_zero`")
            && report.contains(UNPROVED),
        "{report}"
    );
}

#[test]
fn a_hypothesis_about_part_of_a_sum_proves_a_named_proposition() {
    let (output, report) = check("sum_hypothesis/pass/proposition_under_addition");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn a_named_proposition_without_its_hypothesis_is_refused() {
    let (output, report) = check("sum_hypothesis/fail/proposition_without_its_hypothesis");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains(
            "cannot prove ensures contract for `add_pair_keeps_difference`: \
             same_difference(add_pair(a, b), add_pair(a2, b))"
        ) && report.contains(UNPROVED),
        "{report}"
    );
}

#[test]
fn a_law_proved_by_cases_inside_cases_compiles() {
    let (output, report) = check("nested_cases/pass/order_is_transitive");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn a_law_with_its_innermost_case_uncited_is_refused() {
    let (output, report) = check("nested_cases/fail/innermost_case_uncited");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains(
            "cannot prove ensures contract for `order_is_transitive` in the case `Peano::Succ`"
        ) && report.contains(UNPROVED),
        "{report}"
    );
}

#[test]
fn repeated_subtraction_modulo_whose_descent_is_cited_compiles() {
    // Core's modulo before it became one structural division helper. Its
    // recursive call is at the computed `saturating_sub(a, b)`, and citing
    // `sub_lt(a, b)` shows that argument is below `a`.
    let (output, report) = check("computed_descent/pass/repeated_subtraction_mod");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn the_same_modulo_recursing_at_its_own_dividend_is_refused_at_that_call() {
    // The twin keeps the citation of `sub_lt(a, b)` and calls `mod(a, b)`.
    // Its bound would still hold for every amount of fuel, so only the
    // descent check refuses it, at the call (`main.omg:26:28`).
    let (output, report) = check("computed_descent/fail/mod_at_unchanged_dividend");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains(
            "cannot prove ensures contract for `mod`: the descent of `mod` at its recursive call"
        ) && report.contains(UNPROVED)
            && report.contains("main.omg:26:28"),
        "{report}"
    );
}

#[test]
fn a_law_that_needs_the_modulo_s_recursive_equation_compiles() {
    // `mod(a, b) = mod(saturating_sub(a, b), b)` for `1 <= b <= a` holds only
    // where the fuel reaches the recursive call. Citing `mod(a, b)` cites the
    // recursive equation the compiler proves for it.
    let (output, report) = check("computed_descent/pass/mod_steps_down_by_its_divisor");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn the_same_law_without_its_dividend_bound_is_refused_at_its_ensures() {
    // Below the divisor the law is false, and the equation leaves the
    // dispatch undecided.
    let (output, report) = check("computed_descent/fail/mod_step_without_its_dividend_bound");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("cannot prove ensures contract for `mod_step`")
            && report.contains(UNPROVED),
        "{report}"
    );
}

#[test]
fn a_law_that_cites_itself_at_a_computed_dividend_compiles() {
    // `modulos_agree` cites itself at `saturating_sub(a, b)`, below `a`: it
    // is proved by induction on fuel that reaches `a`, each side unfolded by
    // its machine's recursive equation.
    let (output, report) = check("computed_descent/pass/two_modulos_agree");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn the_same_law_citing_itself_at_its_own_dividend_is_refused_at_that_citation() {
    // At `a` itself the call does not descend. The induction on fuel needs
    // that descent where the law cites itself, so the refusal names the
    // citation (`main.omg:70:13`), not the law's clause.
    let (output, report) = check("computed_descent/fail/modulos_agree_at_an_unchanged_dividend");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains(
            "cannot prove ensures contract for `modulos_agree`: the descent of `modulos_agree` at its recursive call"
        ) && report.contains("main.omg:70:13")
            && !report.contains("in the case `Nat::Zero`"),
        "{report}"
    );
}

#[test]
fn a_local_order_without_its_fuel_bound_is_refused_for_that_lemma() {
    // `Peano` spells core's truncated subtraction, so `drain`'s descent is
    // checked and holds. The law that its fuel suffices needs the order's
    // `succ_of_sub_between`, which the program's copy leaves out.
    let (output, report) = check("computed_descent/fail/local_order_without_its_bound");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("not supported yet")
            && report.contains("has no lemma `succ_of_sub_between(a, k, w)`")
            && report.contains("`drain`"),
        "{report}"
    );
}

#[test]
fn core_division_computes_each_kind_of_quotient_and_remainder() {
    // Zero dividend, divisor one, a dividend smaller than the divisor, exact
    // and inexact division, each checked by computation through `div` and
    // `mod` of `core/nat.omg`.
    let (output, report) = check("core_division/pass/quotients_and_remainders");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn core_division_by_zero_is_refused_for_its_premise() {
    let (output, report) = check("core_division/fail/divisor_zero");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("cannot prove ensures contract for `quotient_by`")
            && report.contains("saturating_sub(Nat::Succ(Nat::Zero), Nat::Zero), Nat::Zero)")
            && report.contains(UNPROVED),
        "{report}"
    );
}

#[test]
fn core_gcd_computes_and_takes_euclids_step() {
    // A zero divisor, a zero dividend, coprime numbers and common factors,
    // each checked by computation through `gcd` of `core/nat.omg`, and
    // Euclid's step at any positive divisor by citing `gcd_step`.
    let (output, report) = check("core_gcd/pass/euclid");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn euclids_step_without_its_law_is_refused() {
    let (output, report) = check("core_gcd/fail/euclid_without_its_step");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("cannot prove ensures contract for `euclid_at`")
            && report.contains(UNPROVED),
        "{report}"
    );
}

#[test]
fn a_machine_that_recurses_on_a_computed_value_of_local_data_is_refused() {
    // `Peano` is the program's own data, so no natural order checks the
    // descent of `drain(before(a))`.
    let (output, report) = check("fuel_recursion/fail/drain_local_data");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("not supported yet")
            && report
                .contains("recursive call at a computed value whose data has no natural order"),
        "{report}"
    );
}

#[test]
fn a_law_about_every_machine_of_a_signature_compiles() {
    let (output, report) = check("generic_machine/pass/twice_a_fixed_point");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn a_generic_law_without_its_premise_is_refused() {
    let (output, report) = check("generic_machine/fail/twice_without_the_fixed_point");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("cannot prove ensures contract for `twice_keeps_a_fixed_point`")
            && report.contains(UNPROVED),
        "{report}"
    );
}

#[test]
fn a_satisfier_that_proves_nothing_is_refused_for_the_law_it_inherits() {
    let (output, report) = check("ring_law/fail/ring_law_unproven_rejected");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains(
            "cannot prove the law `lawless` inherits from the requirement it satisfies: \
             core\\\\Identity::<Peano>(plus(plus(a, b), c), plus(a, plus(b, c)))"
        ) && report.contains(UNPROVED)
            && report.contains("main.omg:19:9"),
        "{report}"
    );
}

#[test]
fn related_representatives_of_a_quotient_have_one_class() {
    let (output, report) = check("quotient/pass/equivalence_class");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn a_quotient_whose_symmetry_row_asks_for_more_is_refused() {
    let (output, report) = check("quotient/fail/symmetry_of_equal_values_only");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("cannot prove ensures contract for `SameSign::symmetric`")
            && report.contains(
                "the proved equality is not the `symmetric` law that the quotient `Sign` needs \
                 of `same_sign`"
            ),
        "{report}"
    );
}

#[test]
fn an_operation_on_a_quotient_compiles_with_the_theorems_it_selects() {
    let (output, report) = check("quotient/pass/lifted_operation");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn a_quotient_operation_that_selects_another_theorem_is_refused() {
    let (output, report) = check("quotient/fail/lift_selects_another_theorem");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains("the proved equality is not the congruence `normal_class` needs"),
        "{report}"
    );
}

#[test]
fn a_theorem_over_data_indexed_by_values_compiles() {
    let (output, report) = check("indexed_family/pass/vector_sum_bound");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn a_theorem_over_indexed_data_with_a_false_bound_is_refused() {
    let (output, report) = check("indexed_family/fail/vector_sum_bound_too_tight");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains(
            "cannot prove ensures contract for `vector_bound` in the case `Vector::Cons`"
        ) && report.contains("embed(Total) <= 9 * embed(Length)"),
        "{report}"
    );
}

#[test]
fn a_running_machine_with_indexed_evidence_compiles() {
    let (output, report) = check("indexed_family/pass/evidence_in_a_running_machine");
    assert_eq!(output.status.code(), Some(0), "{report}");
}

#[test]
fn evidence_the_data_does_not_allow_is_refused_where_it_is_built() {
    let (output, report) = check("indexed_family/fail/evidence_of_an_element_too_large");
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert!(
        report.contains(
            "cannot prove what `Vector::Cons` requires of the value built here in `room`"
        ),
        "{report}"
    );
}

const ABOVE_ONE_IS_POSITIVE: &str = "data AboveOneIsPositive = machine(x: f64)
    requires x > 1.0f64
    ensures x > 0.0f64;

machine above_one_is_positive(x: f64)
requires x > 1.0f64
ensures x > 0.0f64
{
}

machine use_step<Step: AboveOneIsPositive>(x: f64)
requires x > 1.0f64
ensures x > 0.0f64
{
    Step(x);
}

machine main() {
    use_step::<above_one_is_positive>(2.0f64);
}
";

fn float_theorem(name: &str, source: &str) -> (bool, String) {
    project::check(name, &[("main.omg", source)], &["--check", "main.omg"])
}

#[test]
fn a_float_theorem_supplies_its_alias() {
    let (accepted, report) = float_theorem("float_theorem", ABOVE_ONE_IS_POSITIVE);
    assert!(accepted, "{report}");
}

#[test]
fn a_float_theorem_with_a_narrower_premise_cannot_supply_its_alias() {
    let source = ABOVE_ONE_IS_POSITIVE.replacen(
        "machine above_one_is_positive(x: f64)\nrequires x > 1.0f64",
        "machine above_one_is_positive(x: f64)\nrequires x > 2.0f64",
        1,
    );
    assert!(source.contains("requires x > 2.0f64"), "{source}");
    let (accepted, report) = float_theorem("float_theorem_narrower", &source);
    assert!(!accepted, "{report}");
    assert!(
        report.contains("explicit proof argument does not establish the complete aliased contract"),
        "{report}"
    );
}

#[test]
fn a_theorem_over_another_float_format_cannot_supply_the_alias() {
    let source = ABOVE_ONE_IS_POSITIVE.replacen(
        "machine above_one_is_positive(x: f64)\nrequires x > 1.0f64\nensures x > 0.0f64",
        "machine above_one_is_positive(x: f32)\nrequires x > 1.0f32\nensures x > 0.0f32",
        1,
    );
    assert!(source.contains("above_one_is_positive(x: f32)"), "{source}");
    let (accepted, report) = float_theorem("float_theorem_format", &source);
    assert!(!accepted, "{report}");
    assert!(
        report.contains(
            "cited theorem's subjects are not the aliased contract's subjects and carriers"
        ),
        "{report}"
    );
}
