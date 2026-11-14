//! The checker on the lowered form judges every declaration a check claims,
//! not only the machines the entry calls. Each fixture here is a `fail/`
//! program whose entry is `machine main() {}` and whose defect sits in a
//! machine nothing calls. When only the entry's calls were lowered, the route
//! accepted all of them: the defective machine was never in the module, so
//! it had no obligation and no refusal.
//!
//! `wiki/spec/build/validation_subjects.md` sets what a check claims: "every
//! declaration of the subject packages is valid", and the closure "starts
//! from every declaration of every subject, including unused ones". So each
//! program must be rejected or reported as unsupported by the route, and
//! none may be accepted.
#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

use semantic_vocabulary::{IdTable, MachineId};
use symbols::SymbolHandle;
use terminal_psi_to_terminal_psi::{RouteVerdict, judge};
use typed_trees_to_terminal_psi::{Compilation, elaborate, elaborate_closure};

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// What the route must conclude about a program with a defect in an
/// uncalled machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expected {
    /// The route lowers the whole program and an obligation of the uncalled
    /// machine is unproved.
    Rejected,
    /// A subject declaration is outside what the route lowers, so the
    /// compilation is reported as unsupported.
    FallsBack,
    /// The route lowers the whole program and an obligation of the uncalled
    /// machine has a goal form the prover has no rule for: not accepted,
    /// and reported as incomplete rather than as a proof failure.
    Incomplete,
}

/// Thirteen `fail/` programs with an empty entry and a defect in an uncalled
/// machine, all of which the route accepted when it lowered only the entry's
/// calls. They are the five the route now rejects, one program for each of
/// seven different reasons it hands a compilation back, and one it now lowers
/// and stops at a goal form it has no rule for (it handed that one back when
/// this list was made). `#2317` counted 13
/// such programs without recording which, so this list is re-derived: it
/// includes the four that PR names.
const UNCALLED_DEFECTS: &[(&str, Expected)] = &[
    (
        "tests/fixtures/fail/arithmetic/unformed_contradiction_cannot_hide_overflow",
        Expected::Rejected,
    ),
    (
        "tests/fixtures/fail/constants/target_layout_derived_ground_guarantee_reject",
        Expected::Rejected,
    ),
    (
        "tests/fixtures/fail/contracts/overwritten_requirement_cannot_rule_out_branch",
        Expected::Rejected,
    ),
    (
        "tests/fixtures/fail/contracts/ruled_out_caller_does_not_bound_callee_body",
        Expected::Rejected,
    ),
    (
        "tests/fixtures/fail/contracts/unproved_branch_is_not_contradiction",
        Expected::Rejected,
    ),
    (
        "tests/fixtures/fail/arithmetic/integer_multiple_remainder_stale_capture",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/contracts/adapter_input_membership_is_not_result",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/contracts/ruled_out_branch_requires_resolved_names",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/dyn_requirement_float_nonreflexive",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/normal_return_expression_statement_is_not_return",
        Expected::Incomplete,
    ),
    (
        "tests/fixtures/fail/proofs/wrapping_zero_product_does_not_cancel_factor",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/termination/computed_rank_no_decrease",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/termination/native_layout_family_circular",
        Expected::FallsBack,
    ),
];

/// The other programs a sweep found with the same shape and the same wrong
/// acceptance: `fail/` fixtures of `arithmetic`, `calls`, `contracts`,
/// `proofs`, `ranges` and `termination`, and screened fixtures elsewhere,
/// whose entry is `machine main() {}`.
const OTHER_SWEPT_DEFECTS: &[(&str, Expected)] = &[
    (
        "tests/fixtures/fail/arithmetic/integer_multiple_remainder_exact_overflow",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/arithmetic/integer_multiple_remainder_saturation",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/arithmetic/integer_multiple_remainder_signed_minimum",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/arithmetic/integer_multiple_remainder_wrapping",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/arithmetic/integer_multiple_remainder_zero_divisor",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/contracts/match_contract_partial_operand_rejected",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/contracts/match_contract_replaced_subject_rejected",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/contracts/match_contract_unknown_arm_rejected",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/contracts/match_contract_wrong_case_rejected",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/contracts/recursive_result_bound_changed_actual",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/contracts/ruled_out_arm_value_does_not_reach_join",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/contracts/written_element_cannot_rule_out_branch",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/core/content_projection_foreign_package",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/core/scoped_integer_call_wrapping_actual_requires_formation",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/applied_callable_unproven_law",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/dyn_requirement_float_alias_inequality",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/dyn_requirement_float_alias_negated",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/dyn_requirement_float_alias_strict",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/mathematical_negated_custom_order",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/mathematical_negated_ieee_order",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/quotient_runtime_affine_carrier",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/quotient_runtime_false_relation",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/quotient_runtime_false_result",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/quotient_runtime_routed_carrier",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/quotient_theorem_private_crash_rejected",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/referenced_law_slot_ambiguous",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/result_index_authored_operator",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/result_index_lexical_operator",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/result_index_shadowed_result",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/proofs/structural_application_premise_unknown_rejected",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/targets/target_local_precondition_rejected",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/termination/computed_rank_guard_write",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/termination/computed_rank_operand_write",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/termination/computed_rank_static_arity",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/termination/native_layout_family_missing_premise",
        Expected::FallsBack,
    ),
    (
        "tests/fixtures/fail/termination/native_layout_family_unpromised",
        Expected::FallsBack,
    ),
];

/// The typed trees of a kept `fail/` program that types, with `main` bound
/// and the program's own package as the subject.
fn rejected_frontier(fixture: &str) -> Box<compiler::TypedFrontier> {
    unbuilt_frontier::fixture_frontier(fixture)
}

/// The route's verdict on everything the check claims, and the typed-tree
/// machine of each `MachineId` when the program lowered.
fn route_verdict(
    frontier: &compiler::TypedFrontier,
) -> (
    Option<IdTable<MachineId, SymbolHandle>>,
    terminal_check_tables::Checked<RouteVerdict>,
) {
    let elaborated = elaborate(
        &frontier.typed,
        Compilation {
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
            supplies: &[],
        },
    );
    let machines = elaborated
        .as_ref()
        .ok()
        .map(|elaboration| elaboration.machines.clone());
    (machines, judge(elaborated))
}

/// The verdicts that differ from the table, one line each.
fn wrong_verdicts(programs: &[(&str, Expected)]) -> Vec<String> {
    let mut wrong = Vec::new();
    for (fixture, expected) in programs {
        let frontier = rejected_frontier(fixture);
        let found = match route_verdict(&frontier).1 {
            Ok(RouteVerdict::Rejected { .. }) => Some(Expected::Rejected),
            Ok(RouteVerdict::FallBack(_)) => Some(Expected::FallsBack),
            Ok(RouteVerdict::Incomplete(_)) => Some(Expected::Incomplete),
            // Accepted is the defect these tests pin. A refusal that is not
            // a fallback is not expected of any of these programs either.
            Ok(RouteVerdict::Accepted(_)) | Err(_) => None,
        };
        if found != Some(*expected) {
            wrong.push(format!("{fixture}: expected {expected:?}, found {found:?}"));
        }
    }
    wrong
}

#[test]
fn a_defect_in_a_machine_the_entry_never_calls_is_never_accepted() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let wrong = wrong_verdicts(UNCALLED_DEFECTS);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
#[ignore = "36 more fixture compilations, several minutes; run with --ignored"]
fn no_other_swept_program_with_an_uncalled_defect_is_accepted() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let wrong = wrong_verdicts(OTHER_SWEPT_DEFECTS);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The rejection comes from judging the uncalled machine, not from refusing
/// it wholesale. `pick` requires `count >= 10` and subtracts in both arms of
/// `transition count < 50`: `count - 10` is safe and `count - 100` is not.
/// The route proves the first, leaves the second unproved, and reports
/// nothing against the entry.
#[test]
fn an_uncalled_machine_is_lowered_and_judged_like_a_called_one() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier =
        rejected_frontier("tests/fixtures/fail/contracts/unproved_branch_is_not_contradiction");
    let entry = *frontier.entries.first().expect("the Build binds an entry");
    let (machines, verdict) = route_verdict(&frontier);
    let machines = machines.expect("the whole program is inside the lowered subset");
    assert_eq!(
        machines.len(),
        2,
        "the module holds the entry and the machine it never calls"
    );
    let first = MachineId::new(1).unwrap();
    assert_eq!(machines.get(first), Some(&entry), "the entry is machine 1");
    let Ok(RouteVerdict::Rejected {
        unproved,
        unestablished,
        ..
    }) = verdict
    else {
        panic!("the route must reject this program: {verdict:?}");
    };
    assert!(unestablished.is_empty());
    let [only] = unproved.as_slice() else {
        panic!("exactly one obligation is unproved: {unproved:?}");
    };
    assert_ne!(only.machine, first, "the entry owes nothing");
    assert!(
        matches!(
            only.kind,
            terminal_check_tables::obligations::ObligationKind::OperationSafety { .. }
        ),
        "the unproved obligation is the subtraction that can underflow: {only:?}"
    );
}

/// Wholly anonymous arithmetic denotes an exact rational
/// (`wiki/spec/language/numeric_values.md`: "Anonymous integer-literal
/// division is rational division, not truncation"), so `7 / 2` is `7/2`,
/// which no `i32` holds. The route lowered such a division as an integer
/// division yielding a truncated value, proved that, and accepted each
/// machine here. Integer landing also "requires ... explicit `as` when
/// acknowledging fractional intermediates", so `(7 / 2) * 2` must not land
/// as 7 either, and builtin remainder has no anonymous meaning at all. Each
/// tree is now evaluated exactly and refused. None of these fixtures binds
/// an entry, so each machine is judged as the root of its own closure.
const ANONYMOUS_TREES_WITHOUT_A_LANDING: &[(&str, &str, &str)] = &[
    (
        "tests/fixtures/fail/arithmetic/fractional_integer_landing",
        "value",
        "expression fractional anonymous value",
    ),
    (
        "tests/fixtures/fail/arithmetic/fractional_integer_operand",
        "value",
        "expression fractional anonymous value",
    ),
    (
        "tests/fixtures/fail/arithmetic/anonymous_remainder_rejected",
        "run",
        "expression anonymous operation without an exact meaning",
    ),
    (
        "tests/fixtures/fail/arithmetic/implicit_fractional_integer_landing",
        "returned",
        "expression anonymous value with a fractional intermediate",
    ),
    (
        "tests/fixtures/fail/arithmetic/implicit_fractional_integer_landing",
        "local",
        "expression anonymous value with a fractional intermediate",
    ),
    (
        "tests/fixtures/fail/arithmetic/implicit_fractional_integer_landing",
        "field",
        "expression anonymous value with a fractional intermediate",
    ),
    (
        "tests/fixtures/fail/arithmetic/implicit_fractional_integer_landing",
        "argument",
        "expression anonymous value with a fractional intermediate",
    ),
    (
        "tests/fixtures/fail/arithmetic/implicit_fractional_integer_landing",
        "half",
        "expression anonymous value with a fractional intermediate",
    ),
];

#[test]
fn an_anonymous_tree_that_cannot_land_is_handed_back_not_truncated() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut wrong = Vec::new();
    let mut frontiers: Vec<(&str, Box<compiler::TypedFrontier>)> = Vec::new();
    for (fixture, machine, detail) in ANONYMOUS_TREES_WITHOUT_A_LANDING {
        if !frontiers.iter().any(|(checked, _)| checked == fixture) {
            frontiers.push((fixture, rejected_frontier(fixture)));
        }
        let (_, frontier) = frontiers
            .iter()
            .find(|(checked, _)| checked == fixture)
            .expect("checked above");
        let typed = &frontier.typed;
        let root = unbuilt_frontier::program_machine(typed, machine)
            .unwrap_or_else(|| panic!("{fixture} declares `{machine}`"));
        match judge(elaborate_closure(typed, root)) {
            Ok(RouteVerdict::FallBack(refusal)) if refusal.detail == *detail => {}
            other => wrong.push(format!("{fixture} `{machine}`: {other:?}")),
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A package identity that is not the fixture's, so that the fixture's own
/// declarations read as a dependency's.
fn another_package(frontier: &compiler::TypedFrontier) -> semantic_vocabulary::PackageKeyIdentity {
    let subject = frontier.subject.expect("a fixture is checked as a package");
    let mut digest = subject.digest();
    digest[0] ^= 1;
    semantic_vocabulary::PackageKeyIdentity::from_digest(digest).expect("a nonzero digest")
}

/// With no subject declaration beyond the entry, the scope is the entry's
/// calls again, and the same program is accepted. This is the acceptance the
/// first test forbids; it shows the subject, not some other change, is what
/// brings the uncalled machine in.
#[test]
fn the_uncalled_machine_is_judged_because_it_belongs_to_the_subject() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier =
        rejected_frontier("tests/fixtures/fail/contracts/unproved_branch_is_not_contradiction");
    let another = another_package(&frontier);
    // Checked as a dependency of some other package, the fixture's machines
    // are judged only when an entry reaches them.
    let as_dependency = judge(elaborate(
        &frontier.typed,
        Compilation {
            entries: &frontier.entries,
            subjects: &[Some(another)],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
            supplies: &[],
        },
    ));
    let Ok(RouteVerdict::Accepted(accepted)) = as_dependency else {
        panic!("only the empty entry is in scope: {as_dependency:?}");
    };
    assert_eq!(accepted.elaboration.machines.len(), 1);
}

/// What the Build's selections name is in scope whichever package declares
/// it. The same program is checked as a dependency again, so only the entry
/// would be judged, and each selection is then added by hand. Selecting the
/// defective machine brings it in and the program is rejected. Selecting a
/// trait, which the route cannot judge a provider against, hands the
/// compilation back instead of leaving the selection unjudged.
#[test]
fn a_declaration_the_build_selects_is_judged_or_handed_back() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier =
        rejected_frontier("tests/fixtures/fail/contracts/unproved_branch_is_not_contradiction");
    let entry = *frontier.entries.first().expect("the Build binds an entry");
    let another = another_package(&frontier);
    let typed = &frontier.typed;
    let verdict_selecting = |selected: SymbolHandle| {
        judge(elaborate(
            typed,
            Compilation {
                entries: &frontier.entries,
                subjects: &[Some(another)],
                purpose: source::DependencyScope::Product,
                selected: &[selected],
                bindings: &frontier.bindings,
                supplies: &[],
            },
        ))
    };

    // The only other machine this package's Product sources declare.
    let uncalled = typed
        .machines()
        .iter()
        .map(|machine| machine.symbol)
        .find(|symbol| {
            *symbol != entry
                && typed
                    .symbols
                    .symbol_provenance_source_span(*symbol)
                    .and_then(|span| typed.symbols.source_file(span))
                    .is_some_and(|file| {
                        file.origin == source::SourceOrigin::User
                            && file.dependency_scope == source::DependencyScope::Product
                    })
        })
        .expect("the fixture declares `pick` beside its entry");
    let selected_machine = verdict_selecting(uncalled);
    assert!(
        matches!(selected_machine, Ok(RouteVerdict::Rejected { .. })),
        "a selected machine is judged: {selected_machine:?}"
    );

    let a_trait = typed
        .traits()
        .first()
        .expect("the bundled library declares traits")
        .symbol;
    let Ok(RouteVerdict::FallBack(refusal)) = verdict_selecting(a_trait) else {
        panic!("a selection the route cannot judge hands the compilation back");
    };
    assert_eq!(
        refusal.detail,
        "declaration Build selection that is not a machine or a data type"
    );
}

/// What the route must conclude about a one-declaration program.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// Accepted, with this many machines in the module.
    Accepted(usize),
    Rejected,
    /// Handed back, with the refusal's detail.
    FallsBack(String),
}

/// One program per kind of declaration a subject package can hold. Each is
/// the declaration followed by `machine main() {}`, so nothing reaches it
/// and only its being a subject declaration brings it into the check.
const UNUSED_DECLARATIONS: &[(&str, &str)] = &[
    // A valid machine: `count + 1` cannot overflow under its `requires`.
    (
        "valid_machine",
        "machine spare(count: u64) -> u64 requires count <= 10 { (count + 1u64) }",
    ),
    // The same body with no `requires`: the addition can overflow.
    (
        "defective_machine",
        "machine spare(count: u64) -> u64 { (count + 1u64) }",
    ),
    ("plain_data", "data Pair [copy] { left: u64; right: u64; }"),
    // An invariant with nothing constructing the type owes nothing yet.
    (
        "data_invariant",
        "data Small [copy] requires self.value <= 9 { value: u64; }",
    ),
    // Plain affine data: owned, moved at most once, discarded with no code
    // (ownership.md, "Affine ownership permits eligible automatic
    // cleanup"). Nothing holds a value of it here, so nothing is owed; its
    // moves are judged where a value exists (`lowered_route_ownership.rs`).
    ("data_affine", "data Owned { value: u64; }"),
    (
        "generic_machine",
        "machine keep<T>(value: u64) -> u64 { value }",
    ),
    // A machine parameter is how a callback is passed.
    (
        "callback",
        "machine apply<machine Observe>(kept: bool) -> bool\n\
         requires machine Observe(kept: bool) -> bool;\n{\n    Observe(kept)\n}",
    ),
    ("constant", "const GOOD: u64 = 2;"),
    ("domain", "domain u64::AtMostSeven requires self <= 7;"),
    (
        "trait",
        "trait Walk {\n    machine step(remaining: u64) -> u64;\n}",
    ),
    (
        "measure",
        "data Countdown [copy] { remaining: u64; }\n\
         measure Countdown::Remaining(countdown: Countdown) -> u64 { countdown.remaining }",
    ),
    (
        "proposition",
        "data Point [copy] { x: u64; y: u64; }\n\
         proposition same_x(left: Point, right: Point) = left.x == right.x;",
    ),
    // A proof: an empty body whose contract states a lemma.
    (
        "lemma",
        "data Point [copy] { x: u64; y: u64; }\n\
         proposition same_x(left: Point, right: Point) = left.x == right.x;\n\
         machine same_x_symmetric(a: Point, b: Point)\n\
         requires same_x(a, b)\nensures same_x(b, a)\n{\n}",
    ),
    // Wholly anonymous arithmetic is exact: `(2147483647 + 1) - 1` lands in
    // `i32` although its intermediate does not, where `i32` addition would
    // overflow.
    (
        "exact_anonymous_intermediate",
        "machine spare() -> i32 ensures result == 2147483647 { (2147483647 + 1) - 1 }",
    ),
    // The twin of `fail/arithmetic/fractional_integer_operand`: `4096 / 2`
    // is an integer, so the operand lands and the product is judged.
    (
        "integral_anonymous_operand",
        "machine spare(input: i32) -> i32 requires input >= 0, input <= 1 { input * (4096 / 2) }",
    ),
    // An uncalled machine that depends on a library declaration.
    (
        "dependency_call",
        "use omega::language::core::CompactBinary;\n\
         machine spare(word: u64) -> i64 { CompactBinary::signed_from_word(word) }",
    ),
];

/// The route's verdict on `declaration` and an empty `main`, staged as a
/// standalone source with `main` bound.
fn verdict_on_unused(name: &str, declaration: &str) -> Verdict {
    let frontier = staged_unused(name, declaration);
    match route_verdict(&frontier).1 {
        Ok(RouteVerdict::Accepted(accepted)) => {
            Verdict::Accepted(accepted.elaboration.machines.len())
        }
        Ok(RouteVerdict::Rejected { .. }) => Verdict::Rejected,
        Ok(RouteVerdict::FallBack(refusal)) => Verdict::FallsBack(refusal.detail),
        other => panic!("{name}: neither a verdict nor a fallback: {other:?}"),
    }
}

/// `declaration` and an empty entry, staged as `verdict_on_unused` stages
/// them and checked through Psi 04.
fn staged_unused(name: &str, declaration: &str) -> Box<compiler::TypedFrontier> {
    unbuilt_frontier::staged_frontier(name, &format!("{declaration}\nmachine main() {{}}\n"))
}

/// A generic machine nothing applies is still in the check, as its own
/// check form: judged once as written, with the closure judged beside it
/// (`judge_compilation`). generics.md: "An unspecialized body checks every
/// admitted alternative". A closure judged without it decides nothing, so
/// `judge` alone hands the compilation back (the table above), and
/// `judge_compilation` judges it: a valid body is accepted, and one that
/// can overflow for a type it admits is rejected though no caller uses that
/// type or any other.
#[test]
fn an_unused_generic_machine_is_judged_as_its_own_check_form() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let parts = judged_parts;
    // The closure (the empty entry) and the generic form, both accepted.
    assert_eq!(
        parts(
            "generic_machine",
            "machine keep<T>(value: u64) -> u64 { value }"
        ),
        [Verdict::Accepted(1), Verdict::Accepted(1)]
    );
    // One check per listed alternative; the body can overflow in each.
    assert_eq!(
        parts(
            "generic_family_overflow",
            "machine bump<T>(value: u64) -> u64 requires T == u8 || T == u16 { value + 1u64 }"
        ),
        [Verdict::Accepted(1), Verdict::Rejected, Verdict::Rejected]
    );
}

/// Every part `judge_compilation` judges of `declaration` and an empty
/// entry: the closure first, then each generic check form.
fn judged_parts(name: &str, declaration: &str) -> Vec<Verdict> {
    let frontier = staged_unused(name, declaration);
    terminal_psi_to_terminal_psi::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &frontier.typed,
            Compilation {
                entries: &frontier.entries,
                subjects: &[frontier.subject],
                purpose: source::DependencyScope::Product,
                selected: &frontier.selected,
                bindings: &frontier.bindings,
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Portable,
        )
        .program,
    )
    .into_iter()
    .map(|verdict| match verdict {
        Ok(RouteVerdict::Accepted(accepted)) => {
            Verdict::Accepted(accepted.elaboration.machines.len())
        }
        Ok(RouteVerdict::Rejected { .. }) => Verdict::Rejected,
        Ok(RouteVerdict::FallBack(refusal)) => Verdict::FallsBack(refusal.detail),
        other => panic!("{name}: neither a verdict nor a fallback: {other:?}"),
    })
    .collect()
}

/// A generic body that calls another generic machine at its own type
/// parameter is judged, not handed back. `relay<T>` holds `forward<T>` as a
/// contract with no body; `forward`'s own check form, judged in the same
/// compilation, is what proves that contract for every `T`, so `relay`'s
/// form is accepted on its own proofs. A bound the callee does not give
/// still rejects the caller. A machine that applies itself is refused by
/// name, with or without an `ensures`: its form would take its own contract
/// as given, and nothing in a form judges the cycle. The one with no
/// `ensures` and a `terminates` promise is the case that was once accepted:
/// it has no contract row to leave undecided, and it never returns.
#[test]
fn a_generic_body_that_applies_another_generic_machine_is_judged() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let forward = "machine forward<T>(item: T, limit: u32) -> u32 \
                   requires limit <= 10 ensures result <= 10 { limit }";
    let relay = |bound: u32| {
        format!(
            "{forward}\nmachine relay<T>(item: T, limit: u32) -> u32 \
             requires limit <= 10 ensures result <= {bound} {{ forward<T>(item, limit) + 1 }}"
        )
    };
    // The closure (the empty entry), `forward`'s form, and `relay`'s form
    // with `forward<T>` beside it.
    assert_eq!(
        judged_parts("generic_application", &relay(11)),
        [
            Verdict::Accepted(1),
            Verdict::Accepted(1),
            Verdict::Accepted(2)
        ]
    );
    assert_eq!(
        judged_parts("generic_application_exceeds_callee", &relay(10)),
        [
            Verdict::Accepted(1),
            Verdict::Accepted(1),
            Verdict::Rejected
        ]
    );
    assert_eq!(
        judged_parts(
            "generic_application_of_itself",
            "machine again<T>(item: T, limit: u32) -> u32 \
             ensures result <= 10 { again<T>(item, limit) }"
        ),
        [
            Verdict::Accepted(1),
            Verdict::FallsBack("generic machine `again` on a call cycle".to_owned())
        ]
    );
    assert_eq!(
        judged_parts(
            "generic_application_of_itself_with_no_guarantee",
            "machine forever<T>(item: T, limit: u32) -> u32              terminates { forever<T>(item, limit) }"
        ),
        [
            Verdict::Accepted(1),
            Verdict::FallsBack("generic machine `forever` on a call cycle".to_owned())
        ]
    );
    // Two generic machines that reach each other are one cycle; each is
    // refused.
    assert_eq!(
        judged_parts(
            "generic_application_cycle_of_two",
            "machine ping<T>(item: T, limit: u32) -> u32 { pong<T>(item, limit) }
             machine pong<T>(item: T, limit: u32) -> u32 { ping<T>(item, limit) }"
        ),
        [
            Verdict::Accepted(1),
            Verdict::FallsBack("generic machine `ping` on a call cycle".to_owned()),
            Verdict::FallsBack("generic machine `pong` on a call cycle".to_owned())
        ]
    );
}

/// Every kind of unused subject declaration is in the check. What the route
/// can judge it judges: a valid machine and plain data are accepted with the
/// machine in the module, and a defective machine is rejected. Every kind it
/// cannot judge hands the compilation back with a refusal naming that kind,
/// so none is skipped.
#[test]
fn each_kind_of_unused_subject_declaration_is_judged_or_handed_back() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let falls_back = |detail: &str| Verdict::FallsBack(detail.to_owned());
    let expected = [
        Verdict::Accepted(2),
        Verdict::Rejected,
        Verdict::Accepted(1),
        Verdict::Accepted(1),
        Verdict::Accepted(1),
        falls_back("machine generic machines not judged as their own check forms"),
        falls_back("machine generic machines not judged as their own check forms"),
        falls_back("declaration const"),
        // A domain's predicates are contracts of the places declared in it,
        // so a declaration no type names owes nothing once its clauses are
        // read.
        Verdict::Accepted(1),
        // A requirement has no body: nothing is owed until something
        // implements or calls it.
        Verdict::Accepted(1),
        falls_back("declaration measure"),
        falls_back("declaration proposition"),
        falls_back("contract named proposition"),
        Verdict::Accepted(2),
        Verdict::Accepted(2),
        falls_back("type reference"),
    ];
    assert_eq!(expected.len(), UNUSED_DECLARATIONS.len());
    let mut wrong = Vec::new();
    for ((name, declaration), expected) in UNUSED_DECLARATIONS.iter().zip(expected) {
        let found = verdict_on_unused(name, declaration);
        if found != expected {
            wrong.push(format!("{name}: expected {expected:?}, found {found:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A requirement that contradicts a crash route's guard rules the route out,
/// so a machine that writes `crashes none` keeps it. In the pending program
/// `requires denominator != 0u64` makes both `denominator == 0u64` routes
/// unreachable, the machine's own arm and a callee's published route, and the
/// route accepts the whole program. The twin requires `denominator != 1u64`,
/// which admits zero, and is rejected.
#[test]
fn a_requirement_rules_out_the_crash_routes_it_contradicts() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let ruled_out = rejected_frontier(
        "tests/fixtures/pending/contracts/requires_disequality_rules_out_crash_route_exit",
    );
    let verdict = route_verdict(&ruled_out).1;
    assert!(
        matches!(verdict, Ok(RouteVerdict::Accepted(_))),
        "every route is ruled out: {verdict:?}"
    );
    let left_open = rejected_frontier(
        "tests/fixtures/fail/contracts/requires_disequality_leaves_crash_route_open",
    );
    let verdict = route_verdict(&left_open).1;
    assert!(
        matches!(verdict, Ok(RouteVerdict::Rejected { .. })),
        "`denominator != 1u64` admits zero: {verdict:?}"
    );
}

/// A call into a dependency's private machine. The typed trees hold the call
/// resolved. The route took every resolved call as permitted and accepted
/// the program whole, with two obligations discharged. It now refuses the
/// compilation at the call.
#[test]
fn a_call_into_another_packages_private_machine_is_never_accepted() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // The edge the fixture's Build registers, under the alias it gives.
    let frontier = unbuilt_frontier::bound_fixture_frontier(
        "tests/fixtures/fail/build/executed_dependency_private_call",
        None,
        &[(
            "selected",
            "tests/fixtures/pass/build/executed_dependency_source_library",
        )],
    );
    let verdict = route_verdict(&frontier).1;
    assert!(
        matches!(verdict, Ok(RouteVerdict::FallBack(_))),
        "{verdict:?}"
    );
}

/// A statement that computes a value and drops it is a defect: "A non-Unit
/// call result cannot be silently discarded" (`wiki/spec/language/effects.md`).
/// `invalid` evaluates `identity(false) || identity(false);` and goes on. The
/// route read the statement for its effects and accepted the machine; it now
/// hands the machine back.
#[test]
fn a_statement_that_drops_its_value_is_never_accepted() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let wrong = wrong_verdicts(&[(
        "tests/fixtures/fail/calls/non_unit_compound_expression_requires_use",
        Expected::FallsBack,
    )]);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// One program per thing the route decides about a formal the body writes.
/// Each is the declaration followed by `machine main() {}`.
const WRITTEN_FORMALS: &[(&str, &str)] = &[
    // The add after the assignment reads the assigned value, at most 11.
    (
        "mut_scalar_assigned",
        "machine spare(mut value: u32) -> u32 requires value <= 10 {\n    \
         value = value + 1;\n    value + 4294967284\n}",
    ),
    // `value <= 10` was true of the argument; `value` now holds `other`.
    (
        "mut_scalar_stale_fact",
        "machine spare(mut value: u32, other: u32) -> u32 requires value <= 10 {\n    \
         value = other;\n    value + 4294967285\n}",
    ),
    // The same add with no assignment before it.
    (
        "mut_scalar_entry_fact",
        "machine spare(mut value: u32, other: u32) -> u32 requires value <= 10 {\n    \
         value + 4294967285\n}",
    ),
    (
        "immutable_scalar_assigned",
        "machine spare(value: u32) -> u32 { value = 1; value }",
    ),
    // The read after the store is of 5.
    (
        "mut_record_stored",
        "data Pair [copy] { a: u32; b: u32; }\n\
         machine spare(mut pair: Pair) -> u32 requires pair.a <= 10 {\n    \
         pair.a = 5;\n    pair.a + 4294967285\n}",
    ),
    // `pair.a <= 10` was true of what the field held before the store.
    (
        "mut_record_stale_fact",
        "data Pair [copy] { a: u32; b: u32; }\n\
         machine spare(mut pair: Pair) -> u32 requires pair.a <= 10 {\n    \
         pair.a = 4294967295;\n    pair.a + 4294967285\n}",
    ),
    // A whole assignment replaces every field the entry fact spoke of.
    (
        "mut_record_replaced",
        "data Pair [copy] { a: u32; b: u32; }\n\
         machine spare(mut pair: Pair) -> u32 requires pair.a <= 10 {\n    \
         pair = Pair { a: 4294967295, b: 0 };\n    pair.a + 4294967285\n}",
    ),
    (
        "immutable_record_stored",
        "data Pair [copy] { a: u32; b: u32; }\n\
         machine spare(pair: Pair) -> u32 { pair.a = 5; pair.a }",
    ),
    // The callee stores into its own copy, so the caller's field holds 1.
    (
        "mut_record_callers_argument",
        "data Pair [copy] { a: u32; b: u32; }\n\
         machine set(mut pair: Pair) { pair.a = 4294967295; }\n\
         machine spare() -> u32 {\n    \
         let q: Pair = Pair { a: 1, b: 2 };\n    set(q);\n    q.a + 4294967294\n}",
    ),
    // Lent exclusively, the formal holds whatever the callee stored.
    (
        "mut_scalar_lent",
        "machine clear(value: &mut u32) { value = 0; }\n\
         machine spare(mut value: u32) -> u32 requires value <= 10 {\n    \
         clear(&mut value);\n    value + 4294967285\n}",
    ),
    (
        "mut_named_in_ensures",
        "machine spare(mut value: u32) -> u32 ensures result == value { value }",
    ),
    (
        "mut_affine_data",
        "data Token { value: u32; }\n\
         machine spare(mut token: Token) -> u32 { token.value = 1; token.value }",
    ),
];

/// A `mut` formal that holds its own value is the callee's own storage and
/// is written as a `mut` local is; the route used to refuse every machine
/// that declared one (`parameter mutable parameter`). A fact from `requires`
/// is about the argument: it does not survive an assignment to a scalar
/// formal, a store into that field of a record formal, a whole assignment or
/// an exclusive loan, and each such program is rejected for the operation
/// that needed it. A formal that is not `mut` is still not written, and the
/// case the route does not serve is refused by name.
#[test]
fn a_mut_formal_is_written_as_a_mut_local_is_and_no_other_formal_is_written() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let falls_back = |detail: &str| Verdict::FallsBack(detail.to_owned());
    let expected = [
        Verdict::Accepted(2),
        Verdict::Rejected,
        Verdict::Accepted(2),
        falls_back("statement assignment to an immutable binding"),
        Verdict::Accepted(2),
        Verdict::Rejected,
        Verdict::Rejected,
        falls_back("statement store into an immutable binding"),
        Verdict::Accepted(3),
        Verdict::Rejected,
        Verdict::Accepted(2),
        // Plain data without `[copy]` is served since affine moves landed:
        // the callee owns the value, stores into it and drops it.
        Verdict::Accepted(2),
    ];
    assert_eq!(expected.len(), WRITTEN_FORMALS.len());
    let mut wrong = Vec::new();
    for ((name, declaration), expected) in WRITTEN_FORMALS.iter().zip(expected) {
        let found = verdict_on_unused(name, declaration);
        if found != expected {
            wrong.push(format!("{name}: expected {expected:?}, found {found:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Contracts retain invocation inputs while executable bindings are assigned
/// or lent. The same identities survive publication, and the actual working
/// values execute; a guarantee equating the result with the final binding
/// instead of the input is rejected.
#[test]
fn mutable_contract_inputs_publish_and_execute_without_runtime_snapshots() {
    use terminal_interpreter::{
        TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
        TerminalExecutionStatus, TerminalStructuralInputs,
    };

    struct NoEffects;
    impl TerminalEffectHandler for NoEffects {
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            Err(TerminalEffectRejection::new(
                "the counter performs no effect",
            ))
        }
    }

    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier =
        unbuilt_frontier::fixture_frontier("tests/fixtures/pass/contracts/mutable_contract_input");
    let counter = unbuilt_frontier::program_machine(&frontier.typed, "counter").unwrap();
    let closure = match route_verdict(&frontier).1 {
        Ok(RouteVerdict::Accepted(closure)) => closure,
        other => panic!("the original-input contracts must check: {other:?}"),
    };
    let counter_id = closure
        .elaboration
        .machines
        .iter()
        .find_map(|(id, symbol)| (*symbol == counter).then_some(id))
        .unwrap();
    let counter = closure
        .elaboration
        .module
        .machines
        .iter()
        .find(|machine| machine.id == counter_id)
        .unwrap();
    assert_eq!(counter.parameters.len(), 2);
    assert!(
        counter.structural_places.is_empty(),
        "logical scalar inputs require no snapshot storage"
    );
    let publication = closure.publish().expect("counter publication");
    assert_eq!(
        publication.artifact.semantic_module(),
        &closure.elaboration.module
    );
    let input = terminal_psi_to_abstract_operations::lower_module_with_evidence(
        publication.artifact.semantic_module(),
        publication.seals.clone(),
    )
    .expect("Omega 00 retains the original-input guarantees")
    .into_optimization_artifact()
    .into_optimization_input();
    abstract_operations_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_work::WorkSchedule::CURRENT.identity(),
    )
    .expect("Omega 01 independently checks the published operations");
    let bytes = terminal_codec::encode_module(publication.artifact.semantic_module())
        .expect("the published module encodes");
    let mut execution =
        TerminalExecution::start_artifact(&bytes, &[], TerminalStructuralInputs::default())
            .expect("the published module loads");
    let status = execution
        .resume(
            &mut psi_work::WorkMeter::with_budget(10_000_000),
            &mut NoEffects,
        )
        .expect("direct, lent, terminal and affine counter calls execute");
    assert!(
        matches!(status, TerminalExecutionStatus::Complete(_)),
        "{status:?}"
    );

    let wrong =
        unbuilt_frontier::fixture_frontier("tests/fixtures/fail/contracts/mutable_contract_input");
    let verdict = route_verdict(&wrong).1;
    let Ok(RouteVerdict::Rejected { unproved, .. }) = verdict else {
        panic!("the final-value interpretation must reject: {verdict:?}");
    };
    assert!(
        unproved.iter().any(|row| matches!(
            row.kind,
            terminal_check_tables::obligations::ObligationKind::NormalGuarantee { .. }
        )),
        "the authored return guarantee must be unproved: {unproved:?}"
    );
}

#[test]
fn mutable_contract_inputs_keep_crash_guards_and_arrivals_at_their_own_inputs() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for (name, guard, accepted) in [
        ("original_input_crash", 3, true),
        ("assigned_value_crash", 0, false),
    ] {
        let source = format!(
            "machine spare(mut position: u32) requires position == 3u32 \
             crashes Trap position == {guard}u32 {{ position = 0; crash Trap; }}"
        );
        let frontier = staged_unused(name, &source);
        let verdict = route_verdict(&frontier).1;
        if accepted {
            assert!(
                matches!(verdict, Ok(RouteVerdict::Accepted(_))),
                "{name}: {verdict:?}"
            );
        } else {
            let Ok(RouteVerdict::Rejected { unproved, .. }) = verdict else {
                panic!("{name}: the assigned value cannot cover an input-based route: {verdict:?}");
            };
            assert!(
                unproved.iter().any(|row| matches!(
                    row.kind,
                    terminal_check_tables::obligations::ObligationKind::CrashCoverage { .. }
                )),
                "{name}: {unproved:?}"
            );
        }
    }
    assert_eq!(
        verdict_on_unused(
            "fresh_recursive_input",
            "machine spare(mut position: u32) requires position <= 10u32 { \
             position = position + 1; transition position <= 10u32 { \
             true -> spare(position) false -> done() } } machine done() {}"
        ),
        Verdict::Accepted(3)
    );
    assert_eq!(
        verdict_on_unused(
            "recursive_input_not_original",
            "machine spare(mut position: u32) requires position <= 10u32 { \
             position = position + 1; spare(position) }"
        ),
        Verdict::Rejected
    );
    assert_eq!(
        verdict_on_unused(
            "borrowed_completion_not_input_snapshot",
            "machine spare(position: &mut u32) -> u32 ensures result == position { \
             position = 0; position }"
        ),
        Verdict::FallsBack(
            "contract clause that names a borrowed formal outside requires".to_owned()
        )
    );
}

/// One program per thing the route decides about an integer whose type
/// carries an arithmetic policy. Each is the declaration followed by
/// `machine main() {}`.
const POLICY_INTEGERS: &[(&str, &str)] = &[
    // A Wrapping add is total: nothing is owed.
    (
        "wrapping_add",
        "machine spare(left: u32 in Wrapping, right: u32 in Wrapping) -> u32 in Wrapping {\n    \
         left + right\n}",
    ),
    // The literal lands in the carrier and the operator is still Wrapping's.
    (
        "wrapping_literal_operand",
        "machine spare(left: u32 in Wrapping) -> u32 in Wrapping { left * 31 + 1 }",
    ),
    (
        "saturating_add",
        "machine spare(left: i32 in Saturating, right: i32 in Saturating) -> i32 in Saturating {\n    \
         left + right\n}",
    ),
    // Stage 04 rewrites `+%` to the casts and the Wrapping add.
    (
        "suffixed_operator",
        "machine spare(left: u32, right: u32) -> u32 { left +% right }",
    ),
    // A policy supplies no divisor.
    (
        "wrapping_divide_unproved",
        "machine spare(left: u32 in Wrapping, right: u32 in Wrapping) -> u32 in Wrapping {\n    \
         left / right\n}",
    ),
    (
        "wrapping_divide_proved",
        "machine spare(left: u32 in Wrapping, right: u32 in Wrapping) -> u32 in Wrapping\n\
         requires right > 0\n{\n    left / right\n}",
    ),
    // A Wrapping operation beside it excuses no Exact one.
    (
        "exact_add_beside_a_wrapping_one",
        "machine spare(left: u32, right: u32) -> u32 {\n    \
         let wrapped: u32 = left +% right;\n    left + right\n}",
    ),
    (
        "wrapping_field",
        "data Counter [copy] { ticks: u32 in Wrapping; }\n\
         machine spare(mut counter: Counter) -> u32 in Wrapping {\n    \
         counter.ticks = counter.ticks + 1;\n    counter.ticks\n}",
    ),
    (
        "wrapping_loop",
        "machine spare(count: u32 in Wrapping, left: u32) -> u32 in Wrapping {\n    \
         transition left > 0 {\n        true -> spare(count + 1, left - 1)\n        \
         false -> count\n    }\n}",
    ),
    (
        "wrapping_call",
        "machine twice(value: u32 in Wrapping) -> u32 in Wrapping { value + value }\n\
         machine spare(value: u32 in Wrapping) -> u32 in Wrapping { twice(twice(value)) }",
    ),
    // The literal lands in the carrier the cast names.
    (
        "wrapping_constant",
        "machine spare() -> u32 in Wrapping { (5 as u32 in Wrapping) + 1 }",
    ),
    // A cast into a policy converts the carrier exactly first: no policy
    // makes `as` truncate.
    (
        "narrowing_cast_into_wrapping",
        "machine spare(value: u16) -> u8 in Wrapping { value as u8 in Wrapping }",
    ),
    (
        "bounded_narrowing_cast_into_wrapping",
        "machine spare(value: u16) -> u8 in Wrapping requires value <= 255 {\n    \
         value as u8 in Wrapping\n}",
    ),
    (
        "wrapping_and_plain_binding",
        "machine spare(left: u32 in Wrapping, right: u32) -> u32 in Wrapping { left + right }",
    ),
    (
        "two_policies",
        "machine spare(left: u8 in Wrapping, right: u8 in Saturating) -> u8 in Wrapping {\n    \
         left + right\n}",
    ),
    (
        "wrapping_value_as_plain_result",
        "machine spare(left: u32 in Wrapping) -> u32 { left }",
    ),
    (
        "literal_into_wrapping_local",
        "machine spare() -> u32 in Wrapping { let start: u32 in Wrapping = 5; start }",
    ),
    (
        "trapping",
        "machine spare(left: u32 in Trapping, right: u32 in Trapping) -> u32 in Trapping {\n    \
         left + right\n}",
    ),
];

/// An integer of type `T in Wrapping` or `T in Saturating` is judged with
/// its policy's operations; the route used to refuse every machine that
/// named such a type (`type constrained`) and every suffixed operator's
/// rewrite. A policy operation owes what Terminal semantics says it owes: an
/// add nothing, a divide its divisor. It is never read as the Exact
/// operation: an Exact add beside a Wrapping one still owes its range. That
/// a wrapped sum is not taken for the mathematical sum is pinned in
/// `terminal-psi-to-terminal-psi` (`a_wrapped_sum_is_not_the_mathematical_sum`): its
/// verdict is "no rule for this goal", which this table has no row for. A cast into a policy converts the carrier exactly and owes
/// that the value fits. What the route does not serve is refused by name.
#[test]
fn a_policy_qualified_integer_is_judged_with_its_policys_operations() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let falls_back = |detail: &str| Verdict::FallsBack(detail.to_owned());
    let expected = [
        Verdict::Accepted(2),
        Verdict::Accepted(2),
        Verdict::Accepted(2),
        Verdict::Accepted(2),
        Verdict::Rejected,
        Verdict::Accepted(2),
        Verdict::Rejected,
        Verdict::Accepted(2),
        Verdict::Accepted(2),
        Verdict::Accepted(3),
        Verdict::Accepted(2),
        Verdict::Rejected,
        Verdict::Accepted(2),
        falls_back(
            "expression operator between a policy-qualified operand and a plain operand that is not a literal",
        ),
        falls_back("typing operands under two arithmetic policies"),
        // A policy is a domain: a Wrapping value is not a plain result,
        // and a literal lands in the carrier and gains no policy. Both are
        // errors of the program, not forms the route does not serve.
        Verdict::Rejected,
        Verdict::Rejected,
        falls_back("type constrained by Trapping"),
    ];
    assert_eq!(expected.len(), POLICY_INTEGERS.len());
    let mut wrong = Vec::new();
    for ((name, declaration), expected) in POLICY_INTEGERS.iter().zip(expected) {
        let found = verdict_on_unused(name, declaration);
        if found != expected {
            wrong.push(format!("{name}: expected {expected:?}, found {found:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// One program per thing the route decides about data with no field and no
/// case. Each is the declarations followed by `machine main() {}`.
const EMPTY_RECORDS: &[(&str, &str)] = &[
    (
        "minted",
        "data Token {}\nmachine spare() -> u32 { let token: Token = Token {}; 0 }",
    ),
    (
        "declared",
        "data Token {}\nmachine spare() -> u32 { let token: Token; 0 }",
    ),
    // Not `[copy]`: the second argument names a local whose value moved.
    (
        "affine_passed_twice",
        "data Token {}\nmachine take(token: Token) -> u32 { 0 }\n\
         machine spare(token: Token) -> u32 { take(token) + take(token) }",
    ),
    (
        "copy_passed_twice",
        "data Token [copy] {}\nmachine take(token: Token) -> u32 { 0 }\n\
         machine spare(token: Token) -> u32 { take(token) + take(token) }",
    ),
    (
        "lent",
        "data Token {}\nmachine touch(token: &mut Token) {}\n\
         machine spare() -> u32 {\n    let mut token: Token = Token {};\n    \
         touch(&mut token);\n    0\n}",
    ),
    (
        "field",
        "data Token {}\ndata Holder { token: Token; count: u32; }\n\
         machine spare() -> u32 {\n    \
         let holder: Holder = Holder { token: Token {}, count: 3 };\n    holder.count\n}",
    ),
];

/// Data with no field and no case is a record whose roster is empty; the
/// route used to refuse every value of one (`type data without fields`). A
/// value is minted, declared, lent and held in a field like any record, and
/// one that is not `[copy]` still moves only once although it has no storage
/// to find empty.
#[test]
fn an_empty_record_is_a_record_with_no_field() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let expected = [
        Verdict::Accepted(2),
        Verdict::Accepted(2),
        Verdict::Rejected,
        Verdict::Accepted(3),
        Verdict::Accepted(3),
        Verdict::Accepted(2),
    ];
    assert_eq!(expected.len(), EMPTY_RECORDS.len());
    let mut wrong = Vec::new();
    for ((name, declaration), expected) in EMPTY_RECORDS.iter().zip(expected) {
        let found = verdict_on_unused(name, declaration);
        if found != expected {
            wrong.push(format!("{name}: expected {expected:?}, found {found:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
