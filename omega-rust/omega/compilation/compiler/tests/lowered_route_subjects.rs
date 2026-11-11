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
//! program must be rejected by the route or handed back to the current
//! checker, and none may be accepted.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use fixture_package_inputs::{
    FixtureFrontierCheck, check_fixture_keeping_typed_frontier, repo_root,
};
use lowered_discharge::{RouteVerdict, judge};
use semantic_vocabulary::{IdTable, MachineId};
use structure_elaboration::{Compilation, elaborate};
use symbols::SymbolHandle;

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
    /// current checker decides the whole compilation.
    FallsBack,
}

/// Thirteen `fail/` programs with an empty entry and a defect in an uncalled
/// machine, all of which the route accepted when it lowered only the entry's
/// calls. They are the five the route now rejects and one program for each of
/// eight different reasons it hands a compilation back. `#2317` counted 13
/// such programs without recording which, so this list is re-derived: it
/// includes the four that PR names.
const UNCALLED_DEFECTS: &[(&str, Expected)] = &[
    (
        "tests/omega/fail/arithmetic/unformed_contradiction_cannot_hide_overflow",
        Expected::Rejected,
    ),
    (
        "tests/omega/fail/constants/target_layout_derived_ground_guarantee_reject",
        Expected::Rejected,
    ),
    (
        "tests/omega/fail/contracts/overwritten_requirement_cannot_rule_out_branch",
        Expected::Rejected,
    ),
    (
        "tests/omega/fail/contracts/ruled_out_caller_does_not_bound_callee_body",
        Expected::Rejected,
    ),
    (
        "tests/omega/fail/contracts/unproved_branch_is_not_contradiction",
        Expected::Rejected,
    ),
    (
        "tests/omega/fail/arithmetic/integer_multiple_remainder_stale_capture",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/contracts/adapter_input_membership_is_not_result",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/contracts/ruled_out_branch_requires_resolved_names",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/dyn_requirement_float_nonreflexive",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/normal_return_expression_statement_is_not_return",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/wrapping_zero_product_does_not_cancel_factor",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/termination/computed_rank_no_decrease",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/termination/native_layout_family_circular",
        Expected::FallsBack,
    ),
];

/// The other programs a sweep found with the same shape and the same wrong
/// acceptance: `fail/` fixtures of `arithmetic`, `calls`, `contracts`,
/// `proofs`, `ranges` and `termination`, and screened fixtures elsewhere,
/// whose entry is `machine main() {}`.
const OTHER_SWEPT_DEFECTS: &[(&str, Expected)] = &[
    (
        "tests/omega/fail/arithmetic/integer_multiple_remainder_exact_overflow",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/arithmetic/integer_multiple_remainder_saturation",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/arithmetic/integer_multiple_remainder_signed_minimum",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/arithmetic/integer_multiple_remainder_wrapping",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/arithmetic/integer_multiple_remainder_zero_divisor",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/contracts/match_contract_partial_operand_rejected",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/contracts/match_contract_replaced_subject_rejected",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/contracts/match_contract_unknown_arm_rejected",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/contracts/match_contract_wrong_case_rejected",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/contracts/recursive_result_bound_changed_actual",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/contracts/ruled_out_arm_value_does_not_reach_join",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/contracts/written_element_cannot_rule_out_branch",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/core/content_projection_foreign_package",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/core/scoped_integer_call_wrapping_actual_requires_formation",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/applied_callable_unproven_law",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/dyn_requirement_float_alias_inequality",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/dyn_requirement_float_alias_negated",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/dyn_requirement_float_alias_strict",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/mathematical_negated_custom_order",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/mathematical_negated_ieee_order",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/quotient_runtime_affine_carrier",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/quotient_runtime_false_relation",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/quotient_runtime_false_result",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/quotient_runtime_routed_carrier",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/quotient_theorem_private_crash_rejected",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/referenced_law_slot_ambiguous",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/result_index_authored_operator",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/result_index_lexical_operator",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/result_index_shadowed_result",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/proofs/structural_application_premise_unknown_rejected",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/targets/target_local_precondition_rejected",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/termination/computed_rank_guard_write",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/termination/computed_rank_operand_write",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/termination/computed_rank_static_arity",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/termination/native_layout_family_missing_premise",
        Expected::FallsBack,
    ),
    (
        "tests/omega/fail/termination/native_layout_family_unpromised",
        Expected::FallsBack,
    ),
];

/// The typed trees the current checker judged, with the request's entries
/// and subject. Every fixture here is one the current checker rejects after
/// Psi 04.
fn rejected_frontier(fixture: &str) -> Box<compiler::TypedFrontier> {
    let main = repo_root().join(fixture).join("main.omg");
    // Discovery and checking recurse deeply; give them the compiler worker's
    // stack rather than the small one libtest gives each test thread.
    let checked = std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            check_fixture_keeping_typed_frontier(&main, "linux_x86_64")
        })
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    });
    match checked {
        FixtureFrontierCheck::Rejected { frontier, .. } => frontier,
        _ => panic!("{fixture}: the current checker must reject this program after Psi 04"),
    }
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
            subject: frontier.subject,
            selected: &frontier.selected,
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
            // Accepted is the defect these tests pin. `Incomplete` and a
            // refusal that is not a fallback are not expected of any of
            // these programs either.
            Ok(RouteVerdict::Accepted(_) | RouteVerdict::Incomplete(_)) | Err(_) => None,
        };
        if found != Some(*expected) {
            wrong.push(format!("{fixture}: expected {expected:?}, found {found:?}"));
        }
    }
    wrong
}

#[test]
fn a_defect_in_a_machine_the_entry_never_calls_is_never_accepted() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let wrong = wrong_verdicts(UNCALLED_DEFECTS);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
#[ignore = "36 more fixture compilations, several minutes; run with --ignored"]
fn no_other_swept_program_with_an_uncalled_defect_is_accepted() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
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
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let frontier =
        rejected_frontier("tests/omega/fail/contracts/unproved_branch_is_not_contradiction");
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
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let frontier =
        rejected_frontier("tests/omega/fail/contracts/unproved_branch_is_not_contradiction");
    let another = another_package(&frontier);
    // Checked as a dependency of some other package, the fixture's machines
    // are judged only when an entry reaches them.
    let as_dependency = judge(elaborate(
        &frontier.typed,
        Compilation {
            entries: &frontier.entries,
            subject: Some(another),
            selected: &frontier.selected,
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
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let frontier =
        rejected_frontier("tests/omega/fail/contracts/unproved_branch_is_not_contradiction");
    let entry = *frontier.entries.first().expect("the Build binds an entry");
    let another = another_package(&frontier);
    let typed = &frontier.typed;
    let verdict_selecting = |selected: SymbolHandle| {
        judge(elaborate(
            typed,
            Compilation {
                entries: &frontier.entries,
                subject: Some(another),
                selected: &[selected],
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
