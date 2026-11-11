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
        Expected::Incomplete,
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
    // Data that is not `[copy]` has custody, and so cleanup.
    ("data_cleanup", "data Owned { value: u64; }"),
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
    // An uncalled machine that depends on a library declaration.
    (
        "dependency_call",
        "use omega::language::core::wire;\n\
         machine spare(word: u64) -> i64 { CompactBinary::signed_from_word(word) }",
    ),
];

/// Writes `declaration` and an empty entry as a package under the
/// repository's ignored `build` directory (the fixture loader takes only
/// paths inside the repository, and `target` may be a link to a cache
/// outside it, as `mbx` makes it), checks it, and returns the route's
/// verdict.
/// The Build binds `main` for the four hosted targets, as the corpus
/// fixtures' Builds do.
fn verdict_on_unused(name: &str, declaration: &str) -> Verdict {
    let root = repo_root()
        .join("build")
        .join("lowered_route_subjects")
        .join(name);
    std::fs::create_dir_all(&root).expect("staging directory");
    let targets = [
        "macos_arm64",
        "linux_x86_64",
        "linux_arm64",
        "windows_x86_64",
    ];
    let binds: String = targets
        .iter()
        .map(|target| format!("    Build::bind(builder, {target}::ProgramEntry, main);\n"))
        .collect();
    std::fs::write(
        root.join("build.omg"),
        format!(
            "machine build(builder: &mut Build) {{\n    \
             Build::application(builder, \"{name}\");\n{binds}}}\n"
        ),
    )
    .expect("staged Build");
    let main = root.join("main.omg");
    std::fs::write(&main, format!("{declaration}\nmachine main() {{}}\n")).expect("staged source");
    let checked = std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            check_fixture_keeping_typed_frontier(&main, "linux_x86_64")
        })
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    });
    let _ = std::fs::remove_dir_all(&root);
    let frontier = match checked {
        FixtureFrontierCheck::Accepted { frontier, .. }
        | FixtureFrontierCheck::Rejected { frontier, .. }
        | FixtureFrontierCheck::ReviewRefused { frontier, .. } => frontier,
        FixtureFrontierCheck::StoppedBeforeTypedFrontier(diagnostics) => {
            panic!("{name}: the staged program must reach Psi 04: {diagnostics:?}")
        }
    };
    match route_verdict(&frontier).1 {
        Ok(RouteVerdict::Accepted(accepted)) => {
            Verdict::Accepted(accepted.elaboration.machines.len())
        }
        Ok(RouteVerdict::Rejected { .. }) => Verdict::Rejected,
        Ok(RouteVerdict::FallBack(refusal)) => Verdict::FallsBack(refusal.detail),
        other => panic!("{name}: neither a verdict nor a fallback: {other:?}"),
    }
}

/// Every kind of unused subject declaration is in the check. What the route
/// can judge it judges: a valid machine and plain data are accepted with the
/// machine in the module, and a defective machine is rejected. Every kind it
/// cannot judge hands the compilation back with a refusal naming that kind,
/// so none is skipped.
#[test]
fn each_kind_of_unused_subject_declaration_is_judged_or_handed_back() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let falls_back = |detail: &str| Verdict::FallsBack(detail.to_owned());
    let expected = [
        Verdict::Accepted(2),
        Verdict::Rejected,
        Verdict::Accepted(1),
        Verdict::Accepted(1),
        falls_back("type data with custody"),
        falls_back("machine generic parameters"),
        falls_back("machine generic parameters"),
        falls_back("declaration const"),
        falls_back("declaration domain"),
        falls_back("declaration trait"),
        falls_back("declaration measure"),
        falls_back("declaration proposition"),
        falls_back("contract named proposition"),
        falls_back("call receiver, requirement or coercion call"),
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
/// which admits zero, and is rejected. Both are refused by the current
/// checker, which is why the positive program waits in `pending/`.
#[test]
fn a_requirement_rules_out_the_crash_routes_it_contradicts() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let ruled_out = rejected_frontier(
        "tests/omega/pending/contracts/requires_disequality_rules_out_crash_route_exit",
    );
    let verdict = route_verdict(&ruled_out).1;
    assert!(
        matches!(verdict, Ok(RouteVerdict::Accepted(_))),
        "every route is ruled out: {verdict:?}"
    );
    let left_open = rejected_frontier(
        "tests/omega/fail/contracts/requires_disequality_leaves_crash_route_open",
    );
    let verdict = route_verdict(&left_open).1;
    assert!(
        matches!(verdict, Ok(RouteVerdict::Rejected { .. })),
        "`denominator != 1u64` admits zero: {verdict:?}"
    );
}
