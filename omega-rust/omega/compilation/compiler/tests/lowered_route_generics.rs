//! One generic body goes the whole way on the lowered form, from source:
//! it is checked once with its parameters rigid, and its certificates are
//! decided again by the proof kernel, with no search, in each machine stage
//! 04 generated from it. `wiki/drafts/designs/check_on_lowered_form.md`,
//! condition 3 of the gate, asks for that and for the negative twin to be
//! rejected.
//!
//! Each fixture is typed without running its Build
//! (`support/unbuilt_frontier.rs`); the route's three steps then run on
//! those trees (`support/generic_certificate_carry.rs`). Whether the whole
//! program is accepted or rejected is pinned from source in
//! `route_probe.rs`; what is pinned here is the carry.
#[path = "support/generic_certificate_carry.rs"]
mod generic_certificate_carry;
#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

use unbuilt_frontier::repository;

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The program line's outcome and every per-obligation line, tab-split
/// after the fixture path: `[label, specialization, obligation, kind,
/// outcome, detail]`.
fn carried(fixture: &str) -> (String, Vec<Vec<String>>) {
    let _guard = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let main = repository(fixture).join("main.omg");
    let mut lines = Vec::new();
    let outcome = generic_certificate_carry::probe(&main, &mut lines);
    let rows = lines
        .iter()
        .map(|line| {
            let mut fields: Vec<String> = line.split('\t').map(str::to_owned).collect();
            fields.remove(1);
            fields.truncate(6);
            fields
        })
        .collect();
    (outcome, rows)
}

fn count(rows: &[Vec<String>], label: &str, outcome: &str) -> usize {
    rows.iter()
        .filter(|row| row[0] == label && row[4] == outcome)
        .count()
}

/// An implementation parameter: `advance<Selected satisfies Step>` is
/// proved once against `Step::apply`'s contract and carried into
/// `advance<Hold>` and `advance<Shift>`.
#[test]
fn an_implementation_parameter_body_is_checked_once_and_carried() {
    let (outcome, rows) = carried("tests/fixtures/run/generics/implementation_parameter_contract");
    assert_eq!(outcome, "route=carried-accepted");
    assert_eq!(count(&rows, "generic", "proved"), 3);
    assert_eq!(count(&rows, "generic", "refused"), 0);
    assert_eq!(count(&rows, "carried", "accepted"), 6);
    assert_eq!(count(&rows, "carried", "refused"), 0);
}

/// Its twin promises `result <= 18`, beyond the public requirement's bound.
/// Selecting a stronger implementation exposes no private result facts, so
/// both concrete callers and the generic check refuse that guarantee. No
/// certificate can then be carried for it.
#[test]
fn a_bound_only_the_selected_implementations_give_is_refused_generically() {
    let (outcome, rows) = carried(
        "tests/fixtures/fail/generics/implementation_parameter_contract_only_holds_for_selection",
    );
    assert_eq!(outcome, "route=carried-refused");
    let refusals = |label: &str| {
        rows.iter()
            .filter(|row| row[0] == label && row[4] == "refused")
            .map(|row| (row[3].as_str(), row[5].as_str()))
            .collect::<Vec<_>>()
    };
    let no_bound = (
        "guarantee",
        "Unproved(NoDerivation): the bounds do not decide the comparison",
    );
    assert_eq!(refusals("concrete"), [no_bound, no_bound]);
    assert_eq!(refusals("generic"), [no_bound]);
    assert_eq!(
        refusals("carried"),
        [(
            "guarantee",
            "Unproved(NoDerivation): no proof of this was found to carry"
        ); 2]
    );
}

/// Conditional caller proofs do not admit a selected implementation. Source
/// preparation must retain the actual member's witness, and the full driver
/// must refuse that witness even if a public caller's arithmetic is provable.
#[test]
fn a_failed_actual_refinement_is_kept_by_the_whole_source_driver() {
    use terminal_check_tables::UnprovedCause;
    use terminal_check_tables::compilation::TerminalCompilation;
    use terminal_check_tables::obligations::ObligationKind;
    use terminal_psi_to_terminal_psi::RouteVerdict;

    let _guard = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let original = std::fs::read_to_string(repository(
        "tests/fixtures/run/generics/implementation_parameter_contract/main.omg",
    ))
    .unwrap();
    // The unchanged +5 body can satisfy the public <=20 result. Its
    // explicitly weaker <=25 promise still cannot establish that result
    // through the separate conformance witness's ordinary call.
    let mut source = original.clone();
    for (from, to) in [
        ("requires value <= 12", "requires value <= 10"),
        ("ensures result <= 17", "ensures result <= 25"),
    ] {
        assert_eq!(source.matches(from).count(), 1, "exact Shift declaration");
        source = source.replace(from, to);
    }
    assert_ne!(source, original);
    assert_eq!(
        source.matches("value + 5").count(),
        1,
        "unchanged Shift body"
    );
    let frontier = unbuilt_frontier::staged_frontier("failed_actual_refinement", &source);
    let typed = std::sync::Arc::new(frontier.typed);
    let subjects = [frontier.subject];
    let compilation = || typed_trees_to_terminal_psi::Compilation {
        calling: frontier
            .normalization
            .calling
            .borrow(&frontier.typed)
            .selection(),
        providers: frontier.provider_selections(),

        declaration_metadata: &[],
        entries: &frontier.entries,
        subjects: &subjects,
        purpose: source::DependencyScope::Product,
        selected: &frontier.selected,
        bindings: &frontier.bindings,
        supplies: &[],
    };
    let prepared = match typed_trees_to_terminal_psi::prepare_compilation(
        &typed,
        compilation(),
        terminal_codec::TerminalPublication::Portable,
    )
    .program
    {
        TerminalCompilation::Prepared(prepared) => prepared,
        stopped => panic!("the actual source must reach full compilation preparation: {stopped:?}"),
    };
    assert_eq!(prepared.forms.len(), 1);
    assert_eq!(prepared.witnesses.len(), 2);
    assert_eq!(prepared.selection_witnesses.len(), 2);
    let closure = prepared.closure.as_ref().unwrap();
    let first = semantic_vocabulary::MachineId::new(1).unwrap();
    let second = semantic_vocabulary::MachineId::new(2).unwrap();
    for (selection, witness) in &prepared.selection_witnesses {
        let selected = &closure.selections[*selection];
        let witness = prepared.witnesses[*witness].as_ref().unwrap();
        assert_eq!(witness.machines.get(first), Some(&selected.requirement));
        assert_eq!(
            witness.value_arguments.get(first),
            Some(&selected.value_arguments)
        );
        let implementation = closure
            .machines
            .get(selected.implementation)
            .expect("the selection has an actual source implementation");
        assert_eq!(
            witness
                .machines
                .get(second)
                .expect("the witness has an actual source body"),
            implementation,
            "the actual selected member, never another matching requirement witness"
        );
    }

    let verdicts =
        terminal_psi_to_terminal_psi::judge_compilation(TerminalCompilation::Prepared(prepared));
    assert_eq!(
        verdicts.len(),
        4,
        "closure, generic form and both actual witnesses"
    );
    assert!(
        matches!(verdicts.first(), Some(Ok(RouteVerdict::Accepted(_)))),
        "the first closure must be accepted before the separate failed witness: {:?}",
        verdicts.first()
    );
    let mut accepted = 0;
    let mut rejected = 0;
    let mut failed_witness_diagnostics = Vec::new();
    for verdict in &verdicts[2..] {
        match verdict {
            Ok(RouteVerdict::Accepted(_)) => accepted += 1,
            Ok(RouteVerdict::Rejected {
                formations,
                violations,
                unproved,
                unestablished,
                conflicts,
                exceeded,
            }) => {
                assert!(
                    formations.is_empty()
                        && violations.is_empty()
                        && unestablished.is_empty()
                        && conflicts.is_empty()
                        && exceeded.is_empty()
                );
                assert!(unproved.iter().any(|refusal| {
                    matches!(refusal.kind, ObligationKind::NormalGuarantee { .. })
                        && refusal.cause == UnprovedCause::NoDerivation
                }));
                assert!(unproved.iter().all(|refusal| matches!(
                    (refusal.kind, refusal.cause),
                    (
                        ObligationKind::NormalGuarantee { .. },
                        UnprovedCause::NoDerivation
                    ) | (
                        ObligationKind::PublishedGuarantee { .. },
                        UnprovedCause::ReturnDoesNotEstablish
                    )
                )));
                failed_witness_diagnostics.extend(
                    unproved
                        .iter()
                        .map(|refusal| refusal.reject_diagnostic().message),
                );
                rejected += 1;
            }
            other => panic!("the actual witness must be judged, not omitted: {other:?}"),
        }
    }
    assert_eq!((accepted, rejected), (1, 1));

    // Publication must refuse the same source even if its first closure
    // passed: an actual selected member's failed witness remains decisive.
    let refused = psi_compiler::AcceptedTerminalSource::judge(
        typed,
        compilation(),
        terminal_codec::TerminalPublication::Portable,
    )
    .expect_err("normal admission must not publish a closure with a failed actual witness");
    for message in failed_witness_diagnostics {
        assert!(
            refused
                .iter()
                .any(|diagnostic| diagnostic.message == message),
            "normal admission must retain the actual witness refusal: {message}; got {refused:?}"
        );
    }
}

/// A value parameter, with an implementation parameter beside it:
/// `Selected::value<const Units: u64>` applied at `7`, and
/// `selected<Implementation satisfies Bounded>` applied at `Selected`.
///
/// Typed preparation generates `selected` at `Selected` and, for the call
/// that machine makes, `Selected::value` at `7`. Three generic obligations
/// are proved (the call's requirement and guarantee in `selected`, the
/// guarantee in `value`) and each is carried into the generated machine.
#[test]
fn a_value_parameter_body_is_checked_once_and_carried() {
    let (outcome, rows) =
        carried("tests/fixtures/pass/facts/lexical_generic_bound_through_local_exit");
    assert_eq!(outcome, "route=carried-accepted");
    assert_eq!(count(&rows, "generic", "proved"), 3);
    assert_eq!(count(&rows, "generic", "refused"), 0);
    assert_eq!(count(&rows, "carried", "accepted"), 3);
    assert_eq!(count(&rows, "carried", "refused"), 0);
}

/// Its twin's requirement states `requires true` where the member requires
/// `input <= Units`. The member does not refine the requirement, so the
/// refinement witness for the call's requirement is refused and nothing is
/// carried for it; the guarantees are carried as before.
#[test]
fn a_value_parameter_member_that_asks_more_than_its_requirement_is_refused() {
    let (outcome, rows) =
        carried("tests/fixtures/fail/facts/lexical_generic_bound_through_local_rejected");
    assert_eq!(outcome, "route=carried-refused");
    let refused = |label: &str| {
        rows.iter()
            .filter(|row| row[0] == label && row[4] == "refused")
            .map(|row| row[3].as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(refused("witness"), ["requirement"]);
    assert_eq!(refused("carried"), ["requirement"]);
    assert_eq!(count(&rows, "generic", "refused"), 0);
    assert_eq!(count(&rows, "carried", "accepted"), 2);
}

/// An opaque type parameter: `relay<T>` and `forward<T>` are each proved
/// once with their `T` values rigid, `relay` against `forward`'s contract,
/// and carried into the machines generated for `u8` and for `bool`.
#[test]
fn a_type_parameter_body_is_checked_once_and_carried() {
    let (outcome, rows) = carried("tests/fixtures/run/generics/type_parameter_contract");
    assert_eq!(outcome, "route=carried-accepted");
    // `relay`: the call's requirement, the add and the guarantee.
    // `forward`: its guarantee.
    assert_eq!(count(&rows, "generic", "proved"), 4);
    assert_eq!(count(&rows, "generic", "refused"), 0);
    // Two types, each with both machines.
    assert_eq!(count(&rows, "carried", "accepted"), 8);
    assert_eq!(count(&rows, "carried", "refused"), 0);
}

/// Its twin differs only in the bound `relay` promises, `result <= 10`,
/// which `forward`'s contract does not give after the add: refused once, as
/// a generic body, and then in each generated machine with nothing to
/// carry.
#[test]
fn a_type_parameter_body_that_exceeds_its_callee_is_refused() {
    let (outcome, rows) =
        carried("tests/fixtures/fail/generics/type_parameter_contract_exceeds_callee");
    assert_eq!(outcome, "route=carried-refused");
    let refused: Vec<_> = rows
        .iter()
        .filter(|row| row[4] == "refused" && row[0] != "concrete")
        .map(|row| (row[0].as_str(), row[3].as_str()))
        .collect();
    assert_eq!(
        refused,
        [
            ("generic", "guarantee"),
            ("carried", "guarantee"),
            ("carried", "guarantee")
        ]
    );
}

/// The design's witness for an opaque `T`: `same<T>` and `compare<T>` own no
/// contract clause, so nothing is proved or carried, and what is shown is
/// that both generic forms lower with their `T` values rigid and that each
/// generated machine is that body at `u64`. Its `main` is outside the
/// subset, so each generated machine is the root of its own closure.
#[test]
fn the_comparison_witness_lowers_and_corresponds() {
    let (outcome, rows) =
        carried("tests/fixtures/pass/facts/generic_comparison_body_through_match_join_exit");
    assert_eq!(outcome, "route=carried-accepted");
    assert_eq!(count(&rows, "generic", "refused"), 0);
    assert_eq!(count(&rows, "carried", "refused"), 0);
}

/// The route's verdict on each machine stage 04 generated from the machine
/// named `template`, each as the root of its own closure.
fn generated_verdicts(fixture: &str, template: &str) -> Vec<String> {
    let _guard = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier = unbuilt_frontier::fixture_frontier(fixture);
    let typed = &frontier.typed;
    let declared = unbuilt_frontier::program_machine(typed, template)
        .unwrap_or_else(|| panic!("{fixture}: no machine `{template}`"));
    typed
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.template == declared)
        .map(|specialization| {
            let closure =
                typed_trees_to_terminal_psi::elaborate_closure(typed, specialization.instance);
            match terminal_psi_to_terminal_psi::judge(closure) {
                Ok(terminal_psi_to_terminal_psi::RouteVerdict::Accepted { .. }) => {
                    "accepted".into()
                }
                Ok(terminal_psi_to_terminal_psi::RouteVerdict::Rejected { .. }) => {
                    "rejected".into()
                }
                Ok(terminal_psi_to_terminal_psi::RouteVerdict::Incomplete { .. }) => {
                    "incomplete".into()
                }
                Ok(terminal_psi_to_terminal_psi::RouteVerdict::FallBack(refusal)) => {
                    format!("falls back: {}", refusal.detail)
                }
                Err(refusal) => format!("defect: {}", refusal.detail),
            }
        })
        .collect()
}

/// A finite type family: `threshold<T>` requires `T == u32 || T == u64` and
/// branches on `T == u32`. Stage 04 does not fold the branch, so the machine
/// generated for `u32` still holds `u32 == u32`, both arms, and an arm that
/// returns a `u64`. Closed type equations are decided statically: the
/// `requires` states nothing, the true arm is taken, and the arm of the
/// other type is left unread. Both generated machines are then judged.
#[test]
fn a_generated_machine_of_a_finite_family_is_read_under_its_own_type() {
    assert_eq!(
        generated_verdicts(
            "tests/fixtures/pass/contracts/generic_effective_result_comparison_exit",
            "threshold",
        ),
        ["accepted", "accepted"]
    );
    assert_eq!(
        generated_verdicts(
            "tests/fixtures/fail/contracts/specialized_normal_call_wrong_type_contract",
            "choose",
        ),
        ["accepted", "accepted"]
    );
}

/// A finite type family checked once per alternative: `pick<T>` admits `u8`
/// and `u16`, and its `ensures` states what it returns at each, a conjunct
/// after `T == u8` typed with `T` fixed to `u8` (generics.md, "Static type
/// equality"). The body is checked with `T` fixed to each listed type, and
/// each generated machine takes the certificate of the alternative whose
/// body it is, decided again by the kernel.
#[test]
fn a_finite_family_is_checked_once_per_alternative_and_carried() {
    let (outcome, rows) = carried("tests/fixtures/run/generics/finite_family_contract");
    assert_eq!(outcome, "route=carried-accepted");
    // One guarantee per alternative.
    assert_eq!(count(&rows, "generic", "proved"), 2);
    assert_eq!(count(&rows, "generic", "refused"), 0);
    // `pick<u8>` and `pick<u16>`, each its own alternative's guarantee.
    assert_eq!(count(&rows, "carried", "accepted"), 2);
    assert_eq!(count(&rows, "carried", "refused"), 0);
}

/// Its twin differs only in what `pick` returns at `u16`: the `u8`
/// alternative is proved and carried, the `u16` alternative is refused, and
/// `pick<u16>` has nothing to carry.
#[test]
fn a_finite_family_alternative_that_breaks_its_contract_is_refused() {
    let (outcome, rows) =
        carried("tests/fixtures/fail/generics/finite_family_contract_wrong_alternative");
    assert_eq!(outcome, "route=carried-refused");
    assert_eq!(count(&rows, "generic", "proved"), 1);
    assert_eq!(count(&rows, "generic", "refused"), 1);
    assert_eq!(count(&rows, "carried", "accepted"), 1);
    assert_eq!(count(&rows, "carried", "refused"), 1);
}

/// The design's witness for a finite family: `threshold<T>` owns no
/// contract clause beyond its family, so each alternative's check proves
/// nothing and what is shown is that both alternatives lower and that each
/// generated machine corresponds to one of them.
#[test]
fn the_threshold_family_lowers_per_alternative_and_corresponds() {
    let (outcome, rows) =
        carried("tests/fixtures/pass/contracts/generic_effective_result_comparison_exit");
    assert_eq!(outcome, "route=carried-accepted");
    assert!(rows.iter().all(|row| row[4] != "refused"), "{rows:?}");
}
